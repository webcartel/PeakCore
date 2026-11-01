use peakcore_core::{
    HistoryPoint, IpcReply, IpcRequest, MAX_IPC_FRAME_SIZE, MinuteAggregate, decode_payload,
    encode_frame,
};
use std::io;
#[cfg(windows)]
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

#[cfg(windows)]
mod windows_pipe {
    use super::*;
    use crate::state::SharedMetrics;
    use peakcore_core::config::Config;
    use std::ffi::c_void;
    use std::ptr::null_mut;

    type Handle = *mut c_void;

    const TOKEN_QUERY: u32 = 0x0008;
    const TOKEN_USER: u32 = 1;
    const PIPE_ACCESS_DUPLEX: u32 = 0x00000003;
    const PIPE_TYPE_BYTE: u32 = 0;
    const PIPE_READMODE_BYTE: u32 = 0;
    const PIPE_WAIT: u32 = 0;
    const ERROR_PIPE_CONNECTED: u32 = 535;
    const ERROR_MORE_DATA: u32 = 234;
    const INVALID_HANDLE_VALUE: Handle = -1isize as Handle;
    const MAX_INSTANCES: u32 = 1;
    const READ_TIMEOUT: Duration = Duration::from_secs(2);
    const WM_APP_IPC: u32 = 0x8000 + 9;

    #[repr(C)]
    struct SecurityAttributes {
        length: u32,
        descriptor: *mut c_void,
        inherit: i32,
    }

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
        fn CreateNamedPipeW(
            name: *const u16,
            open_mode: u32,
            pipe_mode: u32,
            max_instances: u32,
            out_buffer: u32,
            in_buffer: u32,
            timeout: u32,
            attributes: *mut SecurityAttributes,
        ) -> Handle;
        fn ConnectNamedPipe(pipe: Handle, overlapped: *mut c_void) -> i32;
        fn DisconnectNamedPipe(pipe: Handle) -> i32;
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
        fn PostMessageW(hwnd: Handle, message: u32, w_param: usize, l_param: isize) -> i32;
    }

    #[link(name = "advapi32")]
    unsafe extern "system" {
        fn ConvertSidToStringSidW(sid: *mut c_void, string_sid: *mut *mut u16) -> i32;
        fn ConvertStringSecurityDescriptorToSecurityDescriptorW(
            sddl: *const u16,
            revision: u32,
            descriptor: *mut *mut c_void,
            size: *mut u32,
        ) -> i32;
    }

    fn wide(value: &str) -> Vec<u16> {
        value.encode_utf16().chain(std::iter::once(0)).collect()
    }

    fn last_error() -> io::Error {
        // SAFETY: GetLastError has no preconditions.
        io::Error::from_raw_os_error(unsafe { GetLastError() } as i32)
    }

    fn current_user_sid() -> io::Result<String> {
        let mut token = null_mut();
        // SAFETY: requests the current process token for read-only identity lookup.
        if unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) } == 0 {
            return Err(last_error());
        }
        let mut needed = 0;
        // SAFETY: null buffer with zero size is the documented size-query call.
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
        // SAFETY: the aligned buffer is sized using the preceding query.
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
        // SAFETY: token_user is a valid TOKEN_USER returned by GetTokenInformation.
        let converted = unsafe { ConvertSidToStringSidW((*token_user).user.sid, &mut string_sid) };
        unsafe {
            CloseHandle(token);
        }
        if converted == 0 {
            return Err(last_error());
        }
        let mut sid_len = 0;
        unsafe {
            while *string_sid.add(sid_len) != 0 {
                sid_len += 1;
            }
        }
        let sid =
            unsafe { String::from_utf16_lossy(std::slice::from_raw_parts(string_sid, sid_len)) };
        unsafe {
            LocalFree(string_sid.cast());
        }
        Ok(sid)
    }

    fn pipe_security(sid: &str) -> io::Result<(SecurityAttributes, Handle)> {
        let sddl = wide(&format!("D:P(A;;GA;;;{sid})"));
        let mut descriptor = null_mut();
        // SAFETY: SDDL is NUL-terminated and descriptor is a writable output pointer.
        if unsafe {
            ConvertStringSecurityDescriptorToSecurityDescriptorW(
                sddl.as_ptr(),
                1,
                &mut descriptor,
                null_mut(),
            )
        } == 0
        {
            return Err(last_error());
        }
        let attributes = SecurityAttributes {
            length: std::mem::size_of::<SecurityAttributes>() as u32,
            descriptor,
            inherit: 0,
        };
        Ok((attributes, descriptor))
    }

    pub fn serve(shared: SharedMetrics, hwnd: usize) -> io::Result<()> {
        serve_on(&peakcore_core::ipc_client::pipe_path()?, shared, hwnd)
    }

    /// Serves requests on an explicitly named pipe. Tests use a private name so
    /// a running daemon cannot answer their requests.
    pub fn serve_on(name: &str, shared: SharedMetrics, hwnd: usize) -> io::Result<()> {
        let name = wide(name);
        let sid = current_user_sid()?;
        let (mut attributes, descriptor) = pipe_security(&sid)?;
        loop {
            // SAFETY: arguments describe a local byte-mode pipe; attributes hold a current-user-only DACL.
            let pipe = unsafe {
                CreateNamedPipeW(
                    name.as_ptr(),
                    PIPE_ACCESS_DUPLEX,
                    PIPE_TYPE_BYTE | PIPE_READMODE_BYTE | PIPE_WAIT,
                    MAX_INSTANCES,
                    MAX_IPC_FRAME_SIZE as u32,
                    MAX_IPC_FRAME_SIZE as u32,
                    0,
                    &mut attributes,
                )
            };
            if pipe == INVALID_HANDLE_VALUE {
                unsafe {
                    LocalFree(descriptor);
                }
                return Err(last_error());
            }
            // SAFETY: pipe is a listening named-pipe instance.
            let connected = unsafe { ConnectNamedPipe(pipe, null_mut()) } != 0
                || unsafe { GetLastError() } == ERROR_PIPE_CONNECTED;
            if connected {
                let mut stop_server = false;
                while let Ok(Some(payload)) = read_frame(pipe) {
                    let mut request_shutdown = false;
                    let reply = match decode_payload::<IpcRequest>(&payload) {
                        Ok(request) => {
                            stop_server = cfg!(test) && request.op == "__test_stop";
                            request_shutdown = request.op == "shutdown";
                            handle_request(request, &shared, hwnd)
                        }
                        Err(error) => IpcReply::failure(error.to_string()),
                    };
                    if write_frame(pipe, &reply).is_err() {
                        break;
                    }
                    if request_shutdown {
                        // Queue shutdown only after the acknowledgement has been written.
                        if unsafe { PostMessageW(hwnd as Handle, WM_APP_IPC, 1, 0) } == 0 {
                            eprintln!("Could not post daemon shutdown: {}", last_error());
                        }
                    }
                    if stop_server {
                        // Let the test client consume the reply and close its handle first.
                        let _ = read_frame(pipe);
                        break;
                    }
                }
                if stop_server {
                    unsafe {
                        DisconnectNamedPipe(pipe);
                        CloseHandle(pipe);
                        LocalFree(descriptor);
                    }
                    return Ok(());
                }
            }
            unsafe {
                DisconnectNamedPipe(pipe);
                CloseHandle(pipe);
            }
        }
    }

    fn handle_request(request: IpcRequest, shared: &SharedMetrics, hwnd: usize) -> IpcReply {
        if request.v != 1 {
            return IpcReply::failure("unsupported protocol version");
        }
        match request.op.as_str() {
            "ping" => IpcReply::success(serde_json::json!({ "pong": true })),
            "snapshot" => {
                let store = shared
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                IpcReply::success(serde_json::to_value(&store.snapshot).unwrap_or_default())
            }
            "history" => history_reply(request.range.as_deref(), shared),
            "config" => config_reply(),
            "config_set" => config_set_reply(&request, &Config::path()),
            "alerts" => alerts_reply(shared),
            "show_window" => {
                // SAFETY: hwnd is the daemon's hidden tray window; WM_APP_IPC requests the dashboard.
                if unsafe { PostMessageW(hwnd as Handle, WM_APP_IPC, 0, 0) } == 0 {
                    IpcReply::failure(last_error().to_string())
                } else {
                    IpcReply::success(serde_json::json!({ "requested": true }))
                }
            }
            "shutdown" => IpcReply::success(serde_json::json!({ "requested": true })),
            _ => IpcReply::failure(format!("unknown operation: {}", request.op)),
        }
    }

    pub(super) fn config_reply() -> IpcReply {
        let path = Config::path();
        match Config::load_or_create(&path) {
            Ok(config) => IpcReply::success(serde_json::json!({
                "config": config,
                "path": path.display().to_string(),
            })),
            Err(error) => IpcReply::failure(format!("could not read config: {error}")),
        }
    }

    pub(super) fn config_set_reply(request: &IpcRequest, path: &std::path::Path) -> IpcReply {
        let Some(data) = request.data.clone() else {
            return IpcReply::failure("config_set requires a config payload");
        };
        let config: Config = match serde_json::from_value(data) {
            Ok(config) => config,
            Err(error) => return IpcReply::failure(format!("invalid config: {error}")),
        };
        if let Err(error) = config.validate() {
            return IpcReply::failure(error);
        }
        if let Err(error) = crate::tray::apply_config(&config) {
            return IpcReply::failure(error);
        }
        match config.save(path) {
            Ok(()) => IpcReply::success(serde_json::json!(config)),
            Err(error) => IpcReply::failure(format!("could not save config: {error}")),
        }
    }

    pub(super) fn alerts_reply(shared: &SharedMetrics) -> IpcReply {
        let store = shared
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let mem = &store.snapshot.mem;
        let mem_pct = if mem.total == 0 {
            0.0
        } else {
            (mem.used as f64 * 100.0 / mem.total as f64) as f32
        };
        let statuses = store.alerts.status(store.snapshot.cpu_total, mem_pct);
        let history: Vec<_> = store.alerts.history().copied().collect();
        IpcReply::success(serde_json::json!({
            "statuses": statuses,
            "history": history,
        }))
    }

    fn history_reply(range: Option<&str>, shared: &SharedMetrics) -> IpcReply {
        let range = range.unwrap_or("1h");
        let duration_ms = match range {
            "5m" => 5 * 60_000,
            "15m" => 15 * 60_000,
            "1h" => 60 * 60_000,
            "24h" => 24 * 60 * 60_000,
            _ => return IpcReply::failure("range must be 5m, 15m, 1h, or 24h"),
        };
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        let store = shared
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if range == "24h" {
            let points: Vec<MinuteAggregate> = store
                .history
                .minutes()
                .filter(|point| now.saturating_sub(point.minute_ts_ms) <= duration_ms)
                .copied()
                .collect();
            IpcReply::success(serde_json::to_value(points).unwrap_or_default())
        } else {
            let points: Vec<HistoryPoint> = store
                .history
                .detailed()
                .filter(|point| now.saturating_sub(point.ts_ms) <= duration_ms)
                .copied()
                .collect();
            IpcReply::success(serde_json::to_value(points).unwrap_or_default())
        }
    }

    fn write_frame<T: serde::Serialize>(handle: Handle, value: &T) -> io::Result<()> {
        let frame = encode_frame(value)?;
        let mut offset = 0;
        while offset < frame.len() {
            let mut written = 0;
            // SAFETY: the frame slice is valid for the requested number of bytes.
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

    fn read_frame(handle: Handle) -> io::Result<Option<Vec<u8>>> {
        let mut prefix = [0u8; 4];
        match read_exact_timeout(handle, &mut prefix) {
            Ok(()) => {}
            Err(error) if error.kind() == io::ErrorKind::UnexpectedEof => return Ok(None),
            Err(error) => return Err(error),
        }
        let length = u32::from_le_bytes(prefix) as usize;
        if length > MAX_IPC_FRAME_SIZE {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "IPC frame exceeds 1 MiB",
            ));
        }
        let mut payload = vec![0; length];
        read_exact_timeout(handle, &mut payload)?;
        Ok(Some(payload))
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
            // SAFETY: null output buffer checks the number of bytes currently available.
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
            // SAFETY: the destination slice has `requested` writable bytes.
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

#[cfg(windows)]
pub fn start_server(shared: crate::state::SharedMetrics, hwnd: usize) {
    #[cfg(windows)]
    {
        std::thread::spawn(move || {
            if let Err(error) = windows_pipe::serve(shared, hwnd) {
                eprintln!("PeakCore IPC server stopped: {error}");
            }
        });
    }
    #[cfg(not(windows))]
    let _ = (shared, hwnd);
}

#[cfg(all(test, windows))]
mod integration_tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    /// A private pipe per test instance, so the exchange never touches the
    /// per-user pipe a running daemon may already own.
    fn test_pipe_path() -> String {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        format!(
            r"\\.\pipe\peakcore-test-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        )
    }

    #[test]
    fn test_pipe_path_is_isolated_from_the_daemon_pipe() {
        let daemon_pipe = peakcore_core::ipc_client::pipe_path().expect("daemon pipe path");
        assert_ne!(test_pipe_path(), daemon_pipe);
        assert_ne!(test_pipe_path(), test_pipe_path());
    }

    #[test]
    fn named_pipe_client_and_server_exchange_requests() {
        let pipe = test_pipe_path();
        let server_pipe = pipe.clone();
        let shared = crate::state::new_shared_metrics();
        let server = std::thread::spawn(move || windows_pipe::serve_on(&server_pipe, shared, 0));

        let ping =
            peakcore_core::ipc_client::request_on(&pipe, "ping", None).expect("ping response");
        assert!(ping.ok);

        let unknown =
            peakcore_core::ipc_client::request_on(&pipe, "unsupported_test_operation", None)
                .expect("error response");
        assert!(!unknown.ok);
        assert!(unknown.error.unwrap().contains("unknown operation"));
        let ping = peakcore_core::ipc_client::request_on(&pipe, "ping", None).expect("second ping");
        assert!(ping.ok);

        // A private test operation stops the listener after the response is written.
        let stop = peakcore_core::ipc_client::request_on(&pipe, "__test_stop", None)
            .expect("stop response");
        assert!(!stop.ok);
        assert!(server.join().unwrap().is_ok());
    }

    #[test]
    fn concurrent_clients_are_served_without_pipe_busy_errors() {
        let pipe = test_pipe_path();
        let server_pipe = pipe.clone();
        let shared = crate::state::new_shared_metrics();
        let server = std::thread::spawn(move || windows_pipe::serve_on(&server_pipe, shared, 0));

        let workers: Vec<_> = (0..8)
            .map(|_| {
                let pipe = pipe.clone();
                std::thread::spawn(move || {
                    for _ in 0..4 {
                        let reply = peakcore_core::ipc_client::request_on(&pipe, "ping", None)
                            .expect("concurrent ping");
                        assert!(reply.ok);
                    }
                })
            })
            .collect();
        for worker in workers {
            worker.join().expect("concurrent worker");
        }

        let stop = peakcore_core::ipc_client::request_on(&pipe, "__test_stop", None)
            .expect("stop response");
        assert!(!stop.ok);
        assert!(server.join().unwrap().is_ok());
    }
}

#[cfg(all(test, windows))]
mod config_ipc_tests {
    use super::windows_pipe::{alerts_reply, config_set_reply};
    use peakcore_core::IpcRequest;
    use peakcore_core::alerts::Alerts;
    use peakcore_core::config::{AlertMetric, AlertRule, Config, Theme};

    fn temp_config_path(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("peakcore-ipc-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir.join("config.toml")
    }

    fn config_request(config: &Config) -> IpcRequest {
        IpcRequest::new("config_set").with_data(serde_json::to_value(config).expect("config json"))
    }

    #[test]
    fn config_set_saves_valid_settings_and_replies_with_them() {
        let path = temp_config_path("save");
        let mut config = Config::default();
        config.ui.theme = Theme::Light;

        let reply = config_set_reply(&config_request(&config), &path);
        assert!(reply.ok, "unexpected error: {:?}", reply.error);
        assert_eq!(Config::load(&path).expect("saved config"), config);

        let saved: Config =
            serde_json::from_value(reply.data.expect("reply data")).expect("config");
        assert_eq!(saved, config);
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn config_set_rejects_invalid_settings_without_writing_the_file() {
        let path = temp_config_path("invalid");
        let mut config = Config::default();
        config.tray.warn_pct = 95;
        config.tray.crit_pct = 90;

        let reply = config_set_reply(&config_request(&config), &path);
        assert!(!reply.ok);
        assert!(reply.error.unwrap().contains("warn_pct"));
        assert!(!path.exists(), "invalid settings must not reach the disk");
    }

    #[test]
    fn config_set_requires_a_payload() {
        let path = std::path::Path::new("unused-config.toml");
        let reply = config_set_reply(&IpcRequest::new("config_set"), path);
        assert!(!reply.ok);
        assert!(reply.error.unwrap().contains("payload"));
        assert!(!path.exists());
    }

    #[test]
    fn alerts_reply_reports_rules_with_the_current_values() {
        let shared = crate::state::new_shared_metrics();
        {
            let mut store = shared
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            store.snapshot.cpu_total = 91.0;
            store.snapshot.mem.total = 100;
            store.snapshot.mem.used = 50;
            store.alerts = Alerts::new(vec![AlertRule {
                metric: AlertMetric::Cpu,
                threshold_pct: 90,
                duration_s: 30,
                cooldown_min: 10,
            }]);
        }

        let reply = alerts_reply(&shared);
        assert!(reply.ok, "unexpected error: {:?}", reply.error);
        let data = reply.data.expect("reply data");
        let statuses = data["statuses"].as_array().expect("statuses");
        assert_eq!(statuses.len(), 1);
        assert_eq!(statuses[0]["threshold_pct"], 90);
        assert_eq!(statuses[0]["value_pct"], 91.0);
        assert_eq!(statuses[0]["triggered"], false);
        assert!(data["history"].as_array().expect("history").is_empty());
    }
}
