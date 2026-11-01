//! Named-pipe client shared by the daemon CLI and the dashboard process.

use crate::{IpcReply, IpcRequest, decode_payload, encode_frame};
use std::io;
use std::time::{Duration, Instant};

#[cfg(windows)]
mod windows {
    use super::*;
    use std::ffi::c_void;
    use std::ptr::null_mut;

    type Handle = *mut c_void;

    const TOKEN_QUERY: u32 = 0x0008;
    const TOKEN_USER: u32 = 1;
    const GENERIC_READ: u32 = 0x80000000;
    const GENERIC_WRITE: u32 = 0x40000000;
    const OPEN_EXISTING: u32 = 3;
    const ERROR_MORE_DATA: u32 = 234;
    const INVALID_HANDLE_VALUE: Handle = -1isize as Handle;
    const READ_TIMEOUT: Duration = Duration::from_secs(2);

    /// `WaitNamedPipeW` gives up when another client grabs the only free
    /// instance first, and so does `CreateFileW`; both are retried until this
    /// budget runs out.
    const CONNECT_TIMEOUT: Duration = Duration::from_secs(2);
    const CONNECT_SLICE: Duration = Duration::from_millis(75);
    const ERROR_FILE_NOT_FOUND: i32 = 2;
    const ERROR_SEM_TIMEOUT: i32 = 121;
    const ERROR_PIPE_BUSY: i32 = 231;

    #[repr(C)]
    struct SidAndAttributes {
        sid: *mut c_void,
        attributes: u32,
    }

    #[repr(C)]
    struct TokenUser {
        user: SidAndAttributes,
    }

    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetCurrentProcess() -> Handle;
        fn GetLastError() -> u32;
        fn OpenProcessToken(process: Handle, access: u32, token: *mut Handle) -> i32;
        fn GetTokenInformation(
            token: Handle,
            class: u32,
            buffer: *mut c_void,
            length: u32,
            needed: *mut u32,
        ) -> i32;
        fn WaitNamedPipeW(name: *const u16, timeout: u32) -> i32;
        fn CreateFileW(
            name: *const u16,
            access: u32,
            share: u32,
            attributes: *mut c_void,
            creation: u32,
            flags: u32,
            template: Handle,
        ) -> Handle;
        fn ReadFile(
            file: Handle,
            buffer: *mut u8,
            bytes: u32,
            read: *mut u32,
            overlapped: *mut c_void,
        ) -> i32;
        fn WriteFile(
            file: Handle,
            buffer: *const u8,
            bytes: u32,
            written: *mut u32,
            overlapped: *mut c_void,
        ) -> i32;
        fn PeekNamedPipe(
            pipe: Handle,
            buffer: *mut u8,
            buffer_size: u32,
            read: *mut u32,
            available: *mut u32,
            left: *mut u32,
        ) -> i32;
        fn CloseHandle(handle: Handle) -> i32;
        fn LocalFree(memory: Handle) -> Handle;
    }

    #[link(name = "advapi32")]
    unsafe extern "system" {
        fn ConvertSidToStringSidW(sid: *mut c_void, string_sid: *mut *mut u16) -> i32;
    }

    fn wide(value: &str) -> Vec<u16> {
        value.encode_utf16().chain(std::iter::once(0)).collect()
    }

    fn last_error() -> io::Error {
        // SAFETY: GetLastError has no preconditions.
        io::Error::from_raw_os_error(unsafe { GetLastError() } as i32)
    }

    pub(super) fn pipe_path() -> io::Result<String> {
        let mut token = null_mut();
        // SAFETY: requests the current process token for a read-only identity lookup.
        if unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) } == 0 {
            return Err(last_error());
        }
        let mut needed = 0;
        // SAFETY: null buffer and zero size is the documented size query.
        unsafe {
            GetTokenInformation(token, TOKEN_USER, null_mut(), 0, &mut needed);
        }
        if needed < std::mem::size_of::<TokenUser>() as u32 {
            unsafe {
                CloseHandle(token);
            }
            return Err(last_error());
        }
        let mut buffer = vec![0u64; needed.div_ceil(8) as usize];
        // SAFETY: buffer is aligned and sized to the result of the preceding query.
        if unsafe {
            GetTokenInformation(
                token,
                TOKEN_USER,
                buffer.as_mut_ptr().cast(),
                needed,
                &mut needed,
            )
        } == 0
        {
            unsafe {
                CloseHandle(token);
            }
            return Err(last_error());
        }
        let token_user = buffer.as_ptr().cast::<TokenUser>();
        let mut string_sid = null_mut();
        // SAFETY: token_user came from GetTokenInformation(TokenUser).
        let converted = unsafe { ConvertSidToStringSidW((*token_user).user.sid, &mut string_sid) };
        unsafe {
            CloseHandle(token);
        }
        if converted == 0 {
            return Err(last_error());
        }
        let mut length = 0;
        unsafe {
            while *string_sid.add(length) != 0 {
                length += 1;
            }
        }
        let sid =
            unsafe { String::from_utf16_lossy(std::slice::from_raw_parts(string_sid, length)) };
        unsafe {
            LocalFree(string_sid.cast());
        }
        Ok(format!(r"\\.\pipe\peakcore-{sid}"))
    }

    pub(super) fn request_on(pipe: &str, request: &IpcRequest) -> io::Result<IpcReply> {
        let name = wide(pipe);
        let handle = connect(&name)?;
        let result = (|| {
            write_frame(handle, request)?;
            read_reply(handle)
        })();
        unsafe {
            CloseHandle(handle);
        }
        result
    }

    fn connect(name: &[u16]) -> io::Result<Handle> {
        let deadline = Instant::now() + CONNECT_TIMEOUT;
        loop {
            let remaining = deadline.saturating_duration_since(Instant::now());
            let slice = remaining.min(CONNECT_SLICE).max(Duration::from_millis(1));
            // SAFETY: the pipe name is NUL-terminated; the wait is bounded by `slice`.
            let available = unsafe { WaitNamedPipeW(name.as_ptr(), slice.as_millis() as u32) } != 0;
            if available {
                // SAFETY: opens an existing local pipe for synchronous duplex I/O.
                let handle = unsafe {
                    CreateFileW(
                        name.as_ptr(),
                        GENERIC_READ | GENERIC_WRITE,
                        0,
                        null_mut(),
                        OPEN_EXISTING,
                        0,
                        null_mut(),
                    )
                };
                if handle != INVALID_HANDLE_VALUE {
                    return Ok(handle);
                }
            }
            let error = last_error();
            if !is_retryable(&error) {
                return Err(error);
            }
            if Instant::now() >= deadline {
                return Err(error);
            }
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    /// Errors that simply mean "the daemon is between connections" or
    /// "another client won the race for the only free instance".
    fn is_retryable(error: &io::Error) -> bool {
        matches!(
            error.raw_os_error(),
            Some(ERROR_FILE_NOT_FOUND | ERROR_SEM_TIMEOUT | ERROR_PIPE_BUSY)
        )
    }

    fn write_frame<T: serde::Serialize>(handle: Handle, value: &T) -> io::Result<()> {
        let frame = encode_frame(value)?;
        let mut offset = 0;
        while offset < frame.len() {
            let mut written = 0;
            // SAFETY: frame[offset..] is a valid readable source buffer.
            if unsafe {
                WriteFile(
                    handle,
                    frame[offset..].as_ptr(),
                    (frame.len() - offset) as u32,
                    &mut written,
                    null_mut(),
                )
            } == 0
            {
                return Err(last_error());
            }
            if written == 0 {
                return Err(io::Error::new(
                    io::ErrorKind::WriteZero,
                    "named pipe write returned zero",
                ));
            }
            offset += written as usize;
        }
        Ok(())
    }

    fn read_reply(handle: Handle) -> io::Result<IpcReply> {
        let mut prefix = [0u8; 4];
        read_exact_timeout(handle, &mut prefix)?;
        let length = u32::from_le_bytes(prefix) as usize;
        if length > crate::MAX_IPC_FRAME_SIZE {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "IPC frame exceeds 1 MiB",
            ));
        }
        let mut payload = vec![0; length];
        read_exact_timeout(handle, &mut payload)?;
        decode_payload(&payload)
    }

    fn read_exact_timeout(handle: Handle, buffer: &mut [u8]) -> io::Result<()> {
        let deadline = Instant::now() + READ_TIMEOUT;
        let mut offset = 0;
        while offset < buffer.len() {
            if Instant::now() >= deadline {
                return Err(io::Error::new(
                    io::ErrorKind::TimedOut,
                    "named pipe read timed out",
                ));
            }
            let mut available = 0;
            // SAFETY: null output buffer requests the current number of readable bytes.
            if unsafe {
                PeekNamedPipe(
                    handle,
                    null_mut(),
                    0,
                    null_mut(),
                    &mut available,
                    null_mut(),
                )
            } == 0
            {
                return Err(last_error());
            }
            if available == 0 {
                std::thread::sleep(Duration::from_millis(10));
                continue;
            }
            let requested = (buffer.len() - offset).min(available as usize) as u32;
            let mut read = 0;
            // SAFETY: the destination slice has requested writable capacity.
            let status = unsafe {
                ReadFile(
                    handle,
                    buffer[offset..].as_mut_ptr(),
                    requested,
                    &mut read,
                    null_mut(),
                )
            };
            if status == 0 && unsafe { GetLastError() } != ERROR_MORE_DATA {
                return Err(last_error());
            }
            if read == 0 {
                return Err(io::Error::new(
                    io::ErrorKind::UnexpectedEof,
                    "named pipe closed",
                ));
            }
            offset += read as usize;
        }
        Ok(())
    }
}

/// Path of the per-user named pipe the daemon listens on.
pub fn pipe_path() -> io::Result<String> {
    #[cfg(windows)]
    {
        windows::pipe_path()
    }
    #[cfg(not(windows))]
    {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "PeakCore IPC is Windows-only",
        ))
    }
}

pub fn request(op: &str, range: Option<&str>) -> io::Result<IpcReply> {
    let mut request = IpcRequest::new(op);
    request.range = range.map(str::to_owned);
    call(&request)
}

/// Sends an operation together with a JSON payload, used by `config_set`.
pub fn request_data(op: &str, data: serde_json::Value) -> io::Result<IpcReply> {
    call(&IpcRequest::new(op).with_data(data))
}

pub fn call(request: &IpcRequest) -> io::Result<IpcReply> {
    send(None, request)
}

/// Sends `op` to an explicitly named pipe instead of the per-user daemon pipe.
///
/// Hidden from the docs: tests use it to talk to a private server instance
/// without a live daemon intercepting their requests.
#[doc(hidden)]
pub fn request_on(pipe: &str, op: &str, range: Option<&str>) -> io::Result<IpcReply> {
    let mut request = IpcRequest::new(op);
    request.range = range.map(str::to_owned);
    send(Some(pipe), &request)
}

fn send(pipe: Option<&str>, request: &IpcRequest) -> io::Result<IpcReply> {
    let path = match pipe {
        Some(pipe) => pipe.to_owned(),
        None => pipe_path()?,
    };
    #[cfg(windows)]
    {
        windows::request_on(&path, request)
    }
    #[cfg(not(windows))]
    {
        let _ = (&path, request);
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "PeakCore IPC is Windows-only",
        ))
    }
}
