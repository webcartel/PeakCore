//! Windows system metric collection for PeakCore.

use std::io;

#[derive(Clone, Copy, Debug, Default)]
struct CpuTimes {
    idle: u64,
    kernel: u64,
    user: u64,
}

/// Utilization measured between two sampling calls.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct CpuUsage {
    /// Total utilization; absent on the first sample or when counters did not advance.
    pub total: Option<f32>,
    /// Utilization per logical processor; empty on the first sample.
    pub cores: Vec<f32>,
}

/// Utilization percentage between two counter snapshots for one processor.
///
/// `kernel` includes idle time, so busy ticks are `kernel + user - idle`.
fn core_percent(previous: CpuTimes, current: CpuTimes) -> Option<f32> {
    let total =
        current.kernel.saturating_sub(previous.kernel) + current.user.saturating_sub(previous.user);
    let idle = current.idle.saturating_sub(previous.idle);
    (total > 0).then(|| {
        (total.saturating_sub(idle) as f64 * 100.0 / total as f64).clamp(0.0, 100.0) as f32
    })
}

/// Per-processor percentages aligned with `current`. A processor without a
/// baseline, or whose counters did not advance, reports zero.
fn cores_usage(previous: &[CpuTimes], current: &[CpuTimes]) -> Vec<f32> {
    current
        .iter()
        .enumerate()
        .map(|(index, current)| {
            previous
                .get(index)
                .and_then(|previous| core_percent(*previous, *current))
                .unwrap_or(0.0)
        })
        .collect()
}

/// Samples total CPU utilization from `GetSystemTimes` and per-processor
/// utilization from `NtQuerySystemInformation`.
#[derive(Debug, Default)]
pub struct CpuSampler {
    previous: Option<CpuTimes>,
    previous_cores: Option<Vec<CpuTimes>>,
}

impl CpuSampler {
    /// Returns `None` for the initial sample or when counters did not advance.
    pub fn sample(&mut self) -> io::Result<Option<f32>> {
        Ok(self.sample_usage()?.total)
    }

    /// Samples total and per-processor utilization from the same counters.
    pub fn sample_usage(&mut self) -> io::Result<CpuUsage> {
        let current = platform::system_times()?;
        let current_cores = platform::processor_times()?;
        let total = self
            .previous
            .replace(current)
            .and_then(|previous| core_percent(previous, current));
        let cores = match self.previous_cores.take() {
            Some(previous) => cores_usage(&previous, &current_cores),
            None => Vec::new(),
        };
        self.previous_cores = Some(current_cores);
        Ok(CpuUsage { total, cores })
    }
}

pub fn mem_info() -> io::Result<peakcore_core::MemInfo> {
    platform::mem_info()
}

/// Samples physical memory and pagefile counters without querying the heavier
/// commit, cache, and kernel counters from `GetPerformanceInfo`.
pub fn basic_mem_info() -> io::Result<peakcore_core::MemInfo> {
    platform::basic_mem_info()
}

mod process;
mod resources;

pub use process::{ProcessSampler, ProcessSort, group_processes, top_processes};
pub use resources::ResourceSampler;

#[cfg(windows)]
mod platform {
    use super::*;
    use std::ffi::c_void;
    use std::mem::size_of;

    #[repr(C)]
    #[derive(Default)]
    struct FileTime {
        low: u32,
        high: u32,
    }

    impl FileTime {
        fn value(&self) -> u64 {
            (u64::from(self.high) << 32) | u64::from(self.low)
        }
    }

    #[repr(C)]
    struct MemoryStatusEx {
        length: u32,
        memory_load: u32,
        total_phys: u64,
        avail_phys: u64,
        total_pagefile: u64,
        avail_pagefile: u64,
        total_virtual: u64,
        avail_virtual: u64,
        avail_extended_virtual: u64,
    }

    impl Default for MemoryStatusEx {
        fn default() -> Self {
            Self {
                length: size_of::<Self>() as u32,
                memory_load: 0,
                total_phys: 0,
                avail_phys: 0,
                total_pagefile: 0,
                avail_pagefile: 0,
                total_virtual: 0,
                avail_virtual: 0,
                avail_extended_virtual: 0,
            }
        }
    }

    #[repr(C)]
    struct PerformanceInformation {
        cb: u32,
        commit_total: usize,
        commit_limit: usize,
        commit_peak: usize,
        physical_total: usize,
        physical_available: usize,
        system_cache: usize,
        kernel_total: usize,
        kernel_paged: usize,
        kernel_nonpaged: usize,
        page_size: usize,
        handle_count: u32,
        process_count: u32,
        thread_count: u32,
    }

    impl Default for PerformanceInformation {
        fn default() -> Self {
            Self {
                cb: size_of::<Self>() as u32,
                commit_total: 0,
                commit_limit: 0,
                commit_peak: 0,
                physical_total: 0,
                physical_available: 0,
                system_cache: 0,
                kernel_total: 0,
                kernel_paged: 0,
                kernel_nonpaged: 0,
                page_size: 0,
                handle_count: 0,
                process_count: 0,
                thread_count: 0,
            }
        }
    }

    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetSystemTimes(idle: *mut FileTime, kernel: *mut FileTime, user: *mut FileTime) -> i32;
        fn GlobalMemoryStatusEx(status: *mut MemoryStatusEx) -> i32;
        fn GetLastError() -> u32;
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

    /// `SYSTEM_PROCESSOR_PERFORMANCE_INFORMATION` from `winternl.h`.
    #[repr(C)]
    #[derive(Clone, Copy, Default)]
    struct ProcessorPerformanceInformation {
        idle_time: i64,
        kernel_time: i64,
        user_time: i64,
        dpc_time: i64,
        interrupt_time: i64,
        interrupt_count: u32,
    }

    const SYSTEM_PROCESSOR_PERFORMANCE_INFORMATION: u32 = 8;
    const STATUS_INFO_LENGTH_MISMATCH: i32 = 0xC000_0004u32 as i32;
    const MAX_PROCESSOR_COUNT: usize = 4096;

    #[link(name = "psapi")]
    unsafe extern "system" {
        fn GetPerformanceInfo(info: *mut PerformanceInformation, size: u32) -> i32;
    }

    fn last_error() -> io::Error {
        // SAFETY: GetLastError has no preconditions.
        io::Error::from_raw_os_error(unsafe { GetLastError() } as i32)
    }

    pub(super) fn system_times() -> io::Result<CpuTimes> {
        let (mut idle, mut kernel, mut user) = (
            FileTime::default(),
            FileTime::default(),
            FileTime::default(),
        );
        // SAFETY: all pointers refer to writable FILETIME values for the duration of the call.
        if unsafe { GetSystemTimes(&mut idle, &mut kernel, &mut user) } == 0 {
            return Err(last_error());
        }
        Ok(CpuTimes {
            idle: idle.value(),
            kernel: kernel.value(),
            user: user.value(),
        })
    }

    pub(super) fn processor_times() -> io::Result<Vec<CpuTimes>> {
        // `SystemProcessorPerformanceInformation` reports the caller's processor
        // group only (up to 64 logical CPUs); machines with more need a per-group
        // query, which is not implemented yet.
        let entry_size = size_of::<ProcessorPerformanceInformation>();
        let mut capacity = 64usize;
        loop {
            let mut buffer = vec![ProcessorPerformanceInformation::default(); capacity];
            let mut returned = 0u32;
            let length = (capacity * entry_size) as u32;
            // SAFETY: the buffer is writable for `length` bytes and the output length is writable.
            let status = unsafe {
                NtQuerySystemInformation(
                    SYSTEM_PROCESSOR_PERFORMANCE_INFORMATION,
                    buffer.as_mut_ptr().cast(),
                    length,
                    &mut returned,
                )
            };
            if status == STATUS_INFO_LENGTH_MISMATCH {
                capacity = ((returned as usize).div_ceil(entry_size)).max(capacity * 2);
                if capacity > MAX_PROCESSOR_COUNT {
                    return Err(io::Error::other(
                        "NtQuerySystemInformation reported an implausible processor count",
                    ));
                }
                continue;
            }
            if status != 0 {
                return Err(io::Error::other(format!(
                    "NtQuerySystemInformation failed: 0x{status:08X}"
                )));
            }
            let count = (returned as usize / entry_size).min(capacity);
            buffer.truncate(count);
            return Ok(buffer
                .into_iter()
                .map(|info| CpuTimes {
                    idle: info.idle_time.max(0) as u64,
                    kernel: info.kernel_time.max(0) as u64,
                    user: info.user_time.max(0) as u64,
                })
                .collect());
        }
    }

    pub(super) fn mem_info() -> io::Result<peakcore_core::MemInfo> {
        let mut memory = MemoryStatusEx::default();
        // SAFETY: the structure is initialized with its required size and remains writable.
        if unsafe { GlobalMemoryStatusEx(&mut memory) } == 0 {
            return Err(last_error());
        }
        let mut performance = PerformanceInformation::default();
        // SAFETY: the structure is initialized with its required size and remains writable.
        if unsafe {
            GetPerformanceInfo(&mut performance, size_of::<PerformanceInformation>() as u32)
        } == 0
        {
            return Err(last_error());
        }
        let page_size = performance.page_size as u64;
        let total = memory.total_phys;
        let available = memory.avail_phys.min(total);
        Ok(peakcore_core::MemInfo {
            total,
            used: total - available,
            available,
            cached: (performance.system_cache as u64).saturating_mul(page_size),
            commit_used: (performance.commit_total as u64).saturating_mul(page_size),
            commit_limit: (performance.commit_limit as u64).saturating_mul(page_size),
            pagefile_used: memory.total_pagefile.saturating_sub(memory.avail_pagefile),
            pagefile_total: memory.total_pagefile,
        })
    }

    pub(super) fn basic_mem_info() -> io::Result<peakcore_core::MemInfo> {
        let mut memory = MemoryStatusEx::default();
        // SAFETY: the structure is initialized with its required size and remains writable.
        if unsafe { GlobalMemoryStatusEx(&mut memory) } == 0 {
            return Err(last_error());
        }
        let total = memory.total_phys;
        let available = memory.avail_phys.min(total);
        Ok(peakcore_core::MemInfo {
            total,
            used: total - available,
            available,
            cached: 0,
            commit_used: 0,
            commit_limit: 0,
            pagefile_used: memory.total_pagefile.saturating_sub(memory.avail_pagefile),
            pagefile_total: memory.total_pagefile,
        })
    }
}

#[cfg(not(windows))]
mod platform {
    use super::*;

    pub(super) fn system_times() -> io::Result<CpuTimes> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "PeakCore system sampling is Windows-only",
        ))
    }

    pub(super) fn processor_times() -> io::Result<Vec<CpuTimes>> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "PeakCore system sampling is Windows-only",
        ))
    }

    pub(super) fn mem_info() -> io::Result<peakcore_core::MemInfo> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "PeakCore system sampling is Windows-only",
        ))
    }

    pub(super) fn basic_mem_info() -> io::Result<peakcore_core::MemInfo> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "PeakCore system sampling is Windows-only",
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cpu_delta_excludes_idle_time() {
        let previous = CpuTimes {
            idle: 100,
            kernel: 200,
            user: 100,
        };
        let current = CpuTimes {
            idle: 150,
            kernel: 300,
            user: 200,
        };
        assert_eq!(core_percent(previous, current), Some(75.0));
    }

    #[test]
    fn zero_cpu_delta_has_no_percentage() {
        let times = CpuTimes {
            idle: 100,
            kernel: 200,
            user: 100,
        };
        assert_eq!(core_percent(times, times), None);
    }

    #[test]
    fn per_core_usage_reports_every_logical_processor() {
        let previous = [
            CpuTimes {
                idle: 100,
                kernel: 200,
                user: 100,
            },
            CpuTimes {
                idle: 100,
                kernel: 100,
                user: 100,
            },
            CpuTimes {
                idle: 0,
                kernel: 0,
                user: 0,
            },
        ];
        let current = [
            // 200 busy of 200 tracked ticks.
            CpuTimes {
                idle: 150,
                kernel: 300,
                user: 200,
            },
            // Counters did not advance: no measurement for this processor.
            CpuTimes {
                idle: 200,
                kernel: 100,
                user: 100,
            },
            // Every tick idle: the processor was doing nothing.
            CpuTimes {
                idle: 20,
                kernel: 10,
                user: 10,
            },
        ];

        assert_eq!(cores_usage(&previous, &current), vec![75.0, 0.0, 0.0]);
    }

    #[test]
    fn per_core_usage_defaults_processors_without_a_baseline_to_zero() {
        let current = [CpuTimes {
            idle: 50,
            kernel: 100,
            user: 50,
        }];

        assert_eq!(cores_usage(&[], &current), vec![0.0]);
        assert_eq!(cores_usage(&current, &[]), Vec::<f32>::new());
    }

    #[cfg(windows)]
    #[test]
    fn processor_times_reports_the_machines_logical_processors() {
        let times = platform::processor_times().expect("processor times");
        assert!(!times.is_empty(), "no logical processors reported");
    }
}
