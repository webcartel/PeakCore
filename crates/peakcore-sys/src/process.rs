//! Process-list sampling and ranking shared by CLI and UI sampling paths.

use peakcore_core::{AppGroup, ProcSample};
use std::io;

#[derive(Debug, Default)]
pub struct ProcessSampler {
    inner: platform::Sampler,
}

impl ProcessSampler {
    /// Samples the process table. The first sample establishes counter baselines
    /// and reports zero CPU and I/O rates.
    pub fn sample(&mut self) -> io::Result<Vec<ProcSample>> {
        self.inner.sample()
    }

    /// Lazily resolves and caches the executable image path for a PID sampled earlier.
    pub fn executable_path(&mut self, pid: u32) -> Option<String> {
        self.inner.executable_path(pid)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProcessSort {
    Cpu,
    Memory,
}

pub fn top_processes(
    mut processes: Vec<ProcSample>,
    by: ProcessSort,
    count: usize,
) -> Vec<ProcSample> {
    match by {
        ProcessSort::Cpu => {
            processes.sort_by(|a, b| b.cpu.total_cmp(&a.cpu).then_with(|| a.pid.cmp(&b.pid)))
        }
        ProcessSort::Memory => processes.sort_by(|a, b| {
            b.private_bytes
                .cmp(&a.private_bytes)
                .then_with(|| a.pid.cmp(&b.pid))
        }),
    }
    processes.truncate(count);
    processes
}

pub fn group_processes(processes: &[ProcSample]) -> Vec<AppGroup> {
    let mut groups: std::collections::HashMap<String, AppGroup> = std::collections::HashMap::new();
    for process in processes {
        let key = process.name.to_lowercase();
        let group = groups.entry(key).or_insert_with(|| AppGroup {
            name: process.name.clone(),
            ..AppGroup::default()
        });
        group.pids.push(process.pid);
        group.cpu += process.cpu;
        group.private_bytes = group.private_bytes.saturating_add(process.private_bytes);
        group.working_set = group.working_set.saturating_add(process.working_set);
        group.io_read_bps = group.io_read_bps.saturating_add(process.io_read_bps);
        group.io_write_bps = group.io_write_bps.saturating_add(process.io_write_bps);
    }
    let mut result: Vec<_> = groups.into_values().collect();
    for group in &mut result {
        group.pids.sort_unstable();
    }
    result.sort_by_key(|group| group.name.to_lowercase());
    result
}

#[cfg(windows)]
mod platform {
    use super::*;
    use std::collections::HashMap;
    use std::ffi::c_void;
    use std::mem::size_of;
    use std::time::Instant;

    const SYSTEM_PROCESS_INFORMATION: u32 = 5;
    const STATUS_INFO_LENGTH_MISMATCH: i32 = 0xC000_0004u32 as i32;
    const PROCESS_QUERY_LIMITED_INFORMATION: u32 = 0x1000;

    #[repr(C)]
    #[derive(Clone, Copy)]
    struct UnicodeString {
        length: u16,
        maximum_length: u16,
        buffer: *const u16,
    }

    #[repr(C)]
    #[allow(dead_code)]
    struct SystemProcessInformation {
        next_entry_offset: u32,
        number_of_threads: u32,
        working_set_private_size: i64,
        hard_fault_count: u32,
        number_of_threads_high_watermark: u32,
        cycle_time: u64,
        create_time: i64,
        user_time: i64,
        kernel_time: i64,
        image_name: UnicodeString,
        base_priority: i32,
        process_id: *mut c_void,
        parent_process_id: *mut c_void,
        handle_count: u32,
        session_id: u32,
        unique_process_key: usize,
        peak_virtual_size: usize,
        virtual_size: usize,
        page_fault_count: u32,
        peak_working_set_size: usize,
        working_set_size: usize,
        quota_peak_paged_pool_usage: usize,
        quota_paged_pool_usage: usize,
        quota_peak_nonpaged_pool_usage: usize,
        quota_nonpaged_pool_usage: usize,
        pagefile_usage: usize,
        peak_pagefile_usage: usize,
        private_page_count: usize,
        read_operation_count: i64,
        write_operation_count: i64,
        other_operation_count: i64,
        read_transfer_count: i64,
        write_transfer_count: i64,
        other_transfer_count: i64,
    }

    #[link(name = "ntdll")]
    unsafe extern "system" {
        fn NtQuerySystemInformation(
            info_class: u32,
            info: *mut c_void,
            info_length: u32,
            return_length: *mut u32,
        ) -> i32;
    }

    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn OpenProcess(access: u32, inherit: i32, process_id: u32) -> *mut c_void;
        fn QueryFullProcessImageNameW(
            process: *mut c_void,
            flags: u32,
            name: *mut u16,
            size: *mut u32,
        ) -> i32;
        fn CloseHandle(handle: *mut c_void) -> i32;
    }

    #[derive(Clone, Copy, Debug)]
    struct Counters {
        create_time: i64,
        user_time: i64,
        kernel_time: i64,
        read_bytes: i64,
        write_bytes: i64,
    }

    #[derive(Clone, Debug)]
    struct CachedPath {
        create_time: i64,
        path: Option<String>,
    }

    #[derive(Debug, Default)]
    pub(super) struct Sampler {
        previous: HashMap<u32, Counters>,
        paths: HashMap<u32, CachedPath>,
        previous_at: Option<Instant>,
        buffer: Vec<u64>,
    }

    impl Sampler {
        pub(super) fn sample(&mut self) -> io::Result<Vec<ProcSample>> {
            let now = Instant::now();
            let elapsed = self
                .previous_at
                .map(|at| now.duration_since(at).as_secs_f64())
                .unwrap_or(0.0);
            let core_count = std::thread::available_parallelism()
                .map(usize::from)
                .unwrap_or(1)
                .max(1);
            let used = query_process_table(&mut self.buffer)?;
            let buffer_ptr = self.buffer.as_ptr();
            let start = buffer_ptr as usize;
            let end = start + used;
            let mut offset = 0usize;
            let mut result = Vec::new();
            let mut current = HashMap::new();
            let mut live = Vec::new();

            loop {
                if offset + size_of::<SystemProcessInformation>() > used {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "truncated process information entry",
                    ));
                }
                // SAFETY: offset is aligned within the kernel-filled, u64-aligned buffer.
                let entry = unsafe {
                    &*buffer_ptr
                        .cast::<u8>()
                        .add(offset)
                        .cast::<SystemProcessInformation>()
                };
                let pid = entry.process_id as usize as u32;
                let ppid = entry.parent_process_id as usize as u32;
                let counters = Counters {
                    create_time: entry.create_time,
                    user_time: entry.user_time,
                    kernel_time: entry.kernel_time,
                    read_bytes: entry.read_transfer_count,
                    write_bytes: entry.write_transfer_count,
                };
                current.insert(pid, counters);
                live.push(pid);

                let (cpu, io_read_bps, io_write_bps) = self
                    .previous
                    .get(&pid)
                    .filter(|previous| previous.create_time == counters.create_time)
                    .map(|previous| rates(*previous, counters, elapsed, core_count))
                    .unwrap_or((0.0, 0, 0));
                let name = read_image_name(entry.image_name, start, end, pid)?;
                let exe_path = self
                    .paths
                    .get(&pid)
                    .filter(|entry| entry.create_time == counters.create_time)
                    .and_then(|entry| entry.path.clone());
                result.push(ProcSample {
                    pid,
                    ppid,
                    name,
                    exe_path,
                    cpu,
                    private_bytes: entry.private_page_count as u64,
                    working_set: entry.working_set_size as u64,
                    io_read_bps,
                    io_write_bps,
                    threads: entry.number_of_threads,
                    handles: entry.handle_count,
                });

                if entry.next_entry_offset == 0 {
                    break;
                }
                if (entry.next_entry_offset as usize) < size_of::<SystemProcessInformation>() {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "invalid next process information offset",
                    ));
                }
                offset = offset
                    .checked_add(entry.next_entry_offset as usize)
                    .filter(|next| *next <= used)
                    .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, format!(
                        "process information offset out of bounds (pid={pid}, offset={offset}, next={}, used={used})",
                        entry.next_entry_offset
                    )))?;
            }

            self.previous = current;
            self.previous_at = Some(now);
            self.paths.retain(|pid, _| live.contains(pid));
            // PID 0 is the idle accounting process, not an application consuming CPU.
            result.retain(|process| process.pid != 0);
            Ok(result)
        }

        fn process_path(&mut self, pid: u32, create_time: i64) -> Option<String> {
            if let Some(cached) = self
                .paths
                .get(&pid)
                .filter(|entry| entry.create_time == create_time)
            {
                return cached.path.clone();
            }
            let path = query_process_path(pid);
            self.paths.insert(
                pid,
                CachedPath {
                    create_time,
                    path: path.clone(),
                },
            );
            path
        }

        pub(super) fn executable_path(&mut self, pid: u32) -> Option<String> {
            let create_time = self.previous.get(&pid)?.create_time;
            self.process_path(pid, create_time)
        }
    }

    fn query_process_table(buffer: &mut Vec<u64>) -> io::Result<usize> {
        if buffer.is_empty() {
            buffer.resize(8_192, 0);
        }
        loop {
            let capacity = buffer.len() * size_of::<u64>();
            let mut returned = 0u32;
            // SAFETY: buffer is writable, u64-aligned, and the length matches its allocation.
            let status = unsafe {
                NtQuerySystemInformation(
                    SYSTEM_PROCESS_INFORMATION,
                    buffer.as_mut_ptr().cast(),
                    capacity.min(u32::MAX as usize) as u32,
                    &mut returned,
                )
            };
            if status >= 0 {
                let used = if returned == 0 {
                    capacity
                } else {
                    returned as usize
                };
                return Ok(used.min(capacity));
            }
            if status != STATUS_INFO_LENGTH_MISMATCH {
                return Err(io::Error::other(format!(
                    "NtQuerySystemInformation failed: 0x{:08X}",
                    status as u32
                )));
            }
            let requested = (returned as usize)
                .max(capacity.saturating_mul(2))
                .saturating_add(4_096);
            if requested > 64 * 1024 * 1024 {
                return Err(io::Error::new(
                    io::ErrorKind::OutOfMemory,
                    "process information buffer exceeds 64 MiB",
                ));
            }
            buffer.resize(requested.div_ceil(size_of::<u64>()), 0);
        }
    }

    fn read_image_name(
        value: UnicodeString,
        start: usize,
        end: usize,
        pid: u32,
    ) -> io::Result<String> {
        if value.length == 0 || value.buffer.is_null() {
            return Ok(if pid == 0 {
                "System Idle Process".into()
            } else {
                format!("Process {pid}")
            });
        }
        if value.length % 2 != 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "odd process image-name byte length",
            ));
        }
        let name_start = value.buffer as usize;
        let name_end = name_start
            .checked_add(value.length as usize)
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    "process image-name pointer overflow",
                )
            })?;
        if name_start < start || name_end > end {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "process image-name pointer is outside the system buffer",
            ));
        }
        // SAFETY: pointer bounds and UTF-16 byte length were validated against the returned buffer.
        let units = unsafe { std::slice::from_raw_parts(value.buffer, value.length as usize / 2) };
        Ok(String::from_utf16_lossy(units))
    }

    fn rates(
        previous: Counters,
        current: Counters,
        elapsed: f64,
        core_count: usize,
    ) -> (f32, u64, u64) {
        if elapsed <= 0.0 || previous.create_time != current.create_time {
            return (0.0, 0, 0);
        }
        let cpu_delta = current
            .user_time
            .saturating_sub(previous.user_time)
            .saturating_add(current.kernel_time.saturating_sub(previous.kernel_time))
            as f64;
        let wall_ticks = elapsed * 10_000_000.0 * core_count as f64;
        let cpu = (cpu_delta * 100.0 / wall_ticks).clamp(0.0, 100.0) as f32;
        let read = current
            .read_bytes
            .saturating_sub(previous.read_bytes)
            .max(0) as f64
            / elapsed;
        let write = current
            .write_bytes
            .saturating_sub(previous.write_bytes)
            .max(0) as f64
            / elapsed;
        (
            cpu,
            read.min(u64::MAX as f64) as u64,
            write.min(u64::MAX as f64) as u64,
        )
    }

    fn query_process_path(pid: u32) -> Option<String> {
        // SAFETY: OpenProcess accepts a PID and returns null if access is denied or it exited.
        let process = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid) };
        if process.is_null() {
            return None;
        }
        let mut path = vec![0u16; 32_768];
        let mut length = path.len() as u32;
        // SAFETY: the buffer is writable, and length specifies its capacity in UTF-16 units.
        let success =
            unsafe { QueryFullProcessImageNameW(process, 0, path.as_mut_ptr(), &mut length) } != 0;
        unsafe {
            CloseHandle(process);
        }
        if !success {
            return None;
        }
        path.truncate(length as usize);
        Some(String::from_utf16_lossy(&path))
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn cpu_and_io_rates_use_elapsed_time_and_core_count() {
            let previous = Counters {
                create_time: 1,
                user_time: 0,
                kernel_time: 0,
                read_bytes: 50,
                write_bytes: 10,
            };
            let current = Counters {
                create_time: 1,
                user_time: 20_000_000,
                kernel_time: 0,
                read_bytes: 550,
                write_bytes: 1_010,
            };
            let (cpu, read, write) = rates(previous, current, 1.0, 4);
            assert!((cpu - 50.0).abs() < 0.001);
            assert_eq!(read, 500);
            assert_eq!(write, 1_000);
        }

        #[test]
        fn create_time_mismatch_resets_cpu_delta() {
            let previous = Counters {
                create_time: 1,
                user_time: 20_000_000,
                kernel_time: 0,
                read_bytes: 1_000,
                write_bytes: 1_000,
            };
            let current = Counters {
                create_time: 2,
                user_time: 30_000_000,
                kernel_time: 0,
                read_bytes: 2_000,
                write_bytes: 2_000,
            };
            assert_eq!(rates(previous, current, 1.0, 4), (0.0, 0, 0));
        }
    }
}

#[cfg(not(windows))]
mod platform {
    use super::*;

    #[derive(Debug, Default)]
    pub(super) struct Sampler;

    impl Sampler {
        pub(super) fn sample(&mut self) -> io::Result<Vec<ProcSample>> {
            Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "Process sampling is Windows-only",
            ))
        }

        pub(super) fn executable_path(&mut self, _pid: u32) -> Option<String> {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn process(pid: u32, name: &str, cpu: f32, private_bytes: u64) -> ProcSample {
        ProcSample {
            pid,
            name: name.into(),
            cpu,
            private_bytes,
            ..ProcSample::default()
        }
    }

    #[test]
    fn top_processes_sorts_by_cpu_or_private_memory_and_limits_count() {
        let processes = vec![
            process(1, "a.exe", 10.0, 300),
            process(2, "b.exe", 30.0, 100),
            process(3, "c.exe", 20.0, 200),
        ];
        let top_cpu = top_processes(processes.clone(), ProcessSort::Cpu, 2);
        assert_eq!(top_cpu.iter().map(|p| p.pid).collect::<Vec<_>>(), [2, 3]);
        let top_memory = top_processes(processes, ProcessSort::Memory, 1);
        assert_eq!(top_memory[0].pid, 1);
    }

    #[test]
    fn group_processes_is_case_insensitive_and_sums_counters() {
        let mut first = process(4, "App.exe", 2.0, 10);
        first.working_set = 20;
        first.io_read_bps = 3;
        let mut second = process(3, "app.EXE", 1.0, 30);
        second.working_set = 40;
        second.io_read_bps = 5;
        let groups = group_processes(&[first, second]);
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].pids, [3, 4]);
        assert_eq!(groups[0].cpu, 3.0);
        assert_eq!(groups[0].private_bytes, 40);
        assert_eq!(groups[0].working_set, 60);
        assert_eq!(groups[0].io_read_bps, 8);
    }
}
