//! Disk and network counters collected only by the dashboard process.

use peakcore_core::ResourceMetrics;
use std::io;

#[derive(Debug, Default)]
pub struct ResourceSampler {
    inner: platform::Sampler,
}

impl ResourceSampler {
    pub fn sample(&mut self) -> io::Result<ResourceMetrics> {
        self.inner.sample()
    }
}

#[cfg(windows)]
mod platform {
    use super::*;
    use peakcore_core::NetworkInterfaceMetrics;
    use std::collections::{HashMap, HashSet};
    use std::ptr::{addr_of, null, null_mut};
    use std::slice;
    use std::time::{Instant, SystemTime, UNIX_EPOCH};
    use windows_sys::Win32::NetworkManagement::IpHelper::{
        FreeMibTable, GetIfTable2, GetIpInterfaceTable, MIB_IF_ROW2, MIB_IF_TABLE2,
        MIB_IPINTERFACE_ROW, MIB_IPINTERFACE_TABLE,
    };
    use windows_sys::Win32::Networking::WinSock::AF_UNSPEC;
    use windows_sys::Win32::System::Performance::{
        PDH_FMT_COUNTERVALUE, PDH_FMT_COUNTERVALUE_ITEM_W, PDH_FMT_DOUBLE, PDH_HCOUNTER,
        PDH_HQUERY, PdhAddEnglishCounterW, PdhCloseQuery, PdhCollectQueryData,
        PdhGetFormattedCounterArrayW, PdhGetFormattedCounterValue, PdhOpenQueryW,
    };

    const IF_OPER_STATUS_UP: i32 = 1;
    const IF_TYPE_SOFTWARE_LOOPBACK: u32 = 24;
    const MEDIA_CONNECT_STATE_CONNECTED: i32 = 1;
    const FILTER_INTERFACE_FLAG: u8 = 1 << 1;

    #[derive(Clone, Copy, Debug, Default)]
    struct InterfaceCounters {
        received: u64,
        sent: u64,
    }

    #[derive(Debug, Default)]
    pub(super) struct Sampler {
        disk: DiskCounters,
        gpu: GpuCounters,
        previous_interfaces: HashMap<u64, InterfaceCounters>,
        previous_at: Option<Instant>,
    }

    impl Sampler {
        pub(super) fn sample(&mut self) -> io::Result<ResourceMetrics> {
            let now = Instant::now();
            let elapsed = self
                .previous_at
                .map(|at| now.duration_since(at).as_secs_f64());
            let (interfaces, current, network_available) =
                sample_interfaces(elapsed, &self.previous_interfaces);
            self.previous_interfaces = current;
            self.previous_at = Some(now);
            let mut result = ResourceMetrics {
                ts_ms: SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis() as u64,
                network_available,
                interfaces,
                ..ResourceMetrics::default()
            };
            for interface in &result.interfaces {
                result.network_rx_bps = result.network_rx_bps.saturating_add(interface.rx_bps);
                result.network_tx_bps = result.network_tx_bps.saturating_add(interface.tx_bps);
            }
            let (read, write, busy, disk_available, disk_busy_available) = self.disk.sample();
            result.disk_available = disk_available;
            result.disk_busy_available = disk_busy_available;
            (
                result.disk_read_bps,
                result.disk_write_bps,
                result.disk_busy_pct,
            ) = (read, write, busy);
            let gpu = self.gpu.sample();
            result.gpu_available = gpu.available;
            result.gpu_utilization_pct = gpu.utilization_pct;
            result.gpu_memory_used = gpu.memory_used;
            result.gpu_engines = gpu.engines;
            result.gpu_processes = gpu.processes;
            Ok(result)
        }
    }

    #[derive(Debug, Default)]
    struct DiskCounters {
        query: usize,
        read: usize,
        write: usize,
        busy: usize,
        counters_added: bool,
    }

    impl DiskCounters {
        fn add_counter(query: PDH_HQUERY, path: &str) -> usize {
            let path: Vec<u16> = path.encode_utf16().chain(Some(0)).collect();
            let mut counter = null_mut();
            let status = unsafe { PdhAddEnglishCounterW(query, path.as_ptr(), 0, &mut counter) };
            if status == 0 { counter as usize } else { 0 }
        }

        fn sample(&mut self) -> (u64, u64, f32, bool, bool) {
            if self.query == 0 {
                let mut query = null_mut();
                if unsafe { PdhOpenQueryW(null(), 0, &mut query) } != 0 {
                    return (0, 0, 0.0, false, false);
                }
                self.query = query as usize;
            }
            if !self.counters_added {
                let query = self.query as PDH_HQUERY;
                self.read = Self::add_counter(query, r"\PhysicalDisk(_Total)\Disk Read Bytes/sec");
                self.write =
                    Self::add_counter(query, r"\PhysicalDisk(_Total)\Disk Write Bytes/sec");
                self.busy = Self::add_counter(query, r"\PhysicalDisk(_Total)\% Disk Time");
                self.counters_added = true;
            }
            if unsafe { PdhCollectQueryData(self.query as PDH_HQUERY) } != 0 {
                return (0, 0, 0.0, false, false);
            }
            let read = Self::value(self.read);
            let write = Self::value(self.write);
            let busy = Self::value(self.busy);
            (
                read.unwrap_or_default().max(0.0).round() as u64,
                write.unwrap_or_default().max(0.0).round() as u64,
                busy.unwrap_or_default().clamp(0.0, 100.0) as f32,
                read.is_some() && write.is_some(),
                busy.is_some(),
            )
        }

        fn value(counter: usize) -> Option<f64> {
            if counter == 0 {
                return None;
            }
            let mut value = PDH_FMT_COUNTERVALUE::default();
            if unsafe {
                PdhGetFormattedCounterValue(
                    counter as PDH_HCOUNTER,
                    PDH_FMT_DOUBLE,
                    null_mut(),
                    &mut value,
                )
            } != 0
                || value.CStatus != 0
            {
                return None;
            }
            Some(unsafe { value.Anonymous.doubleValue })
        }
    }

    impl Drop for DiskCounters {
        fn drop(&mut self) {
            if self.query != 0 {
                unsafe { PdhCloseQuery(self.query as PDH_HQUERY) };
            }
        }
    }

    const PDH_MORE_DATA: u32 = 0x8000_07D2;

    #[derive(Debug, Default)]
    struct GpuSample {
        available: bool,
        utilization_pct: f32,
        memory_used: u64,
        engines: Vec<peakcore_core::GpuEngineMetrics>,
        processes: Vec<peakcore_core::GpuProcessMetrics>,
    }

    #[derive(Debug, Default)]
    struct GpuCounters {
        query: usize,
        engines: usize,
        memory: usize,
        process_memory: usize,
        counters_added: bool,
    }

    impl GpuCounters {
        fn sample(&mut self) -> GpuSample {
            if self.query == 0 {
                let mut query = null_mut();
                if unsafe { PdhOpenQueryW(null(), 0, &mut query) } != 0 {
                    return GpuSample::default();
                }
                self.query = query as usize;
            }
            if !self.counters_added {
                let query = self.query as PDH_HQUERY;
                self.engines =
                    DiskCounters::add_counter(query, r"\GPU Engine(*)\Utilization Percentage");
                self.memory =
                    DiskCounters::add_counter(query, r"\GPU Adapter Memory(*)\Dedicated Usage");
                self.process_memory =
                    DiskCounters::add_counter(query, r"\GPU Process Memory(*)\Dedicated Usage");
                self.counters_added = true;
            }
            if self.engines == 0 && self.memory == 0 && self.process_memory == 0 {
                return GpuSample::default();
            }
            if unsafe { PdhCollectQueryData(self.query as PDH_HQUERY) } != 0 {
                return GpuSample::default();
            }

            // Every process reports its own instance per engine; the engine's
            // utilisation is the sum across processes, capped at 100%.
            let mut per_engine: HashMap<String, f64> = HashMap::new();
            for (instance, value) in array_values(self.engines) {
                let Some(engine) = engine_type(&instance) else {
                    continue;
                };
                *per_engine.entry(engine.to_owned()).or_default() += value;
            }
            let mut engines: Vec<peakcore_core::GpuEngineMetrics> = per_engine
                .into_iter()
                .map(|(name, value)| peakcore_core::GpuEngineMetrics {
                    name,
                    utilization_pct: value.clamp(0.0, 100.0) as f32,
                })
                .collect();
            engines.sort_by(|left, right| {
                right
                    .utilization_pct
                    .total_cmp(&left.utilization_pct)
                    .then_with(|| left.name.cmp(&right.name))
            });
            let utilization_pct = engines
                .first()
                .map(|engine| engine.utilization_pct)
                .unwrap_or_default();

            let memory_used: u64 = array_values(self.memory)
                .into_iter()
                .filter(|(instance, _)| !instance.to_lowercase().contains("total"))
                .map(|(_, value)| value.max(0.0) as u64)
                .sum();

            let mut process_utilization: HashMap<u32, f64> = HashMap::new();
            for (instance, value) in array_values(self.engines) {
                if let Some(pid) = instance_pid(&instance) {
                    *process_utilization.entry(pid).or_default() += value;
                }
            }
            let mut process_memory: HashMap<u32, u64> = HashMap::new();
            for (instance, value) in array_values(self.process_memory) {
                if let Some(pid) = instance_pid(&instance) {
                    let memory = process_memory.entry(pid).or_insert(0);
                    *memory = memory.saturating_add(value.max(0.0) as u64);
                }
            }
            let mut pids: Vec<u32> = process_utilization
                .keys()
                .chain(process_memory.keys())
                .copied()
                .collect();
            pids.sort_unstable();
            pids.dedup();
            let processes = pids
                .into_iter()
                .map(|pid| peakcore_core::GpuProcessMetrics {
                    pid,
                    utilization_pct: process_utilization
                        .get(&pid)
                        .copied()
                        .unwrap_or_default()
                        .clamp(0.0, 100.0) as f32,
                    memory: process_memory.get(&pid).copied().unwrap_or_default(),
                })
                .collect();

            GpuSample {
                available: !engines.is_empty() || memory_used > 0,
                utilization_pct,
                memory_used,
                engines,
                processes,
            }
        }
    }

    impl Drop for GpuCounters {
        fn drop(&mut self) {
            if self.query != 0 {
                unsafe { PdhCloseQuery(self.query as PDH_HQUERY) };
            }
        }
    }

    /// Reads every instance of a wildcard counter.
    fn array_values(counter: usize) -> Vec<(String, f64)> {
        if counter == 0 {
            return Vec::new();
        }
        let mut size = 0u32;
        let mut count = 0u32;
        // SAFETY: a null buffer asks PDH for the required size.
        let probe = unsafe {
            PdhGetFormattedCounterArrayW(
                counter as PDH_HCOUNTER,
                PDH_FMT_DOUBLE,
                &mut size,
                &mut count,
                null_mut(),
            )
        };
        if probe != PDH_MORE_DATA || size == 0 || count == 0 {
            return Vec::new();
        }
        // Keep the buffer 8-byte aligned for PDH_FMT_COUNTERVALUE_ITEM_W.
        let mut buffer = vec![0u64; (size as usize).div_ceil(8)];
        // SAFETY: the buffer is sized from the preceding probe.
        let status = unsafe {
            PdhGetFormattedCounterArrayW(
                counter as PDH_HCOUNTER,
                PDH_FMT_DOUBLE,
                &mut size,
                &mut count,
                buffer.as_mut_ptr().cast(),
            )
        };
        if status != 0 || count == 0 {
            return Vec::new();
        }
        // SAFETY: PDH filled `count` items at the start of the buffer.
        let items = unsafe {
            slice::from_raw_parts(
                buffer.as_ptr().cast::<PDH_FMT_COUNTERVALUE_ITEM_W>(),
                count as usize,
            )
        };
        items
            .iter()
            .filter(|item| item.FmtValue.CStatus == 0)
            .map(|item| {
                // SAFETY: szName points inside the buffer PDH just filled.
                let name = utf16_ptr(item.szName);
                let value = unsafe { item.FmtValue.Anonymous.doubleValue };
                (name, value)
            })
            .collect()
    }

    fn engine_type(instance: &str) -> Option<&str> {
        let (_, engine) = instance.split_once("engtype_")?;
        Some(engine.split('_').next().unwrap_or(engine))
    }

    fn instance_pid(instance: &str) -> Option<u32> {
        instance
            .strip_prefix("pid_")?
            .split('_')
            .next()?
            .parse()
            .ok()
    }

    fn utf16_ptr(pointer: *const u16) -> String {
        if pointer.is_null() {
            return String::new();
        }
        let mut length = 0;
        // SAFETY: PDH NUL-terminates instance names.
        unsafe {
            while *pointer.add(length) != 0 {
                length += 1;
            }
            String::from_utf16_lossy(slice::from_raw_parts(pointer, length))
        }
    }

    fn sample_interfaces(
        elapsed: Option<f64>,
        previous_interfaces: &HashMap<u64, InterfaceCounters>,
    ) -> (
        Vec<NetworkInterfaceMetrics>,
        HashMap<u64, InterfaceCounters>,
        bool,
    ) {
        let Some(connected_interfaces) = connected_ip_interfaces() else {
            return (Vec::new(), HashMap::new(), false);
        };
        let mut table: *mut MIB_IF_TABLE2 = null_mut();
        if unsafe { GetIfTable2(&mut table) } != 0 || table.is_null() {
            return (Vec::new(), HashMap::new(), false);
        }
        // MIB_IF_TABLE2 is a count followed by a variable-length row array.
        let count = unsafe { (*table).NumEntries as usize };
        let rows =
            unsafe { slice::from_raw_parts(addr_of!((*table).Table).cast::<MIB_IF_ROW2>(), count) };
        let mut interfaces = Vec::new();
        let mut current = HashMap::new();
        for row in rows {
            if row.OperStatus != IF_OPER_STATUS_UP
                || row.MediaConnectState != MEDIA_CONNECT_STATE_CONNECTED
                || row.InterfaceAndOperStatusFlags._bitfield & FILTER_INTERFACE_FLAG != 0
                || row.Type == IF_TYPE_SOFTWARE_LOOPBACK
                || !connected_interfaces.contains(&row.InterfaceIndex)
            {
                continue;
            }
            let id = unsafe { row.InterfaceLuid.Value };
            let counters = InterfaceCounters {
                received: row.InOctets,
                sent: row.OutOctets,
            };
            let (rx_bps, tx_bps) = elapsed
                .filter(|seconds| *seconds > 0.0)
                .and_then(|seconds| {
                    previous_interfaces
                        .get(&id)
                        .map(|previous| (seconds, previous))
                })
                .map(|(seconds, previous)| {
                    (
                        (counters.received.saturating_sub(previous.received) as f64 / seconds)
                            .round() as u64,
                        (counters.sent.saturating_sub(previous.sent) as f64 / seconds).round()
                            as u64,
                    )
                })
                .unwrap_or_default();
            let name = utf16_string(&row.Alias);
            let name = if name.is_empty() {
                utf16_string(&row.Description)
            } else {
                name
            };
            current.insert(id, counters);
            interfaces.push(NetworkInterfaceMetrics {
                id,
                index: row.InterfaceIndex,
                name,
                rx_bps,
                tx_bps,
            });
        }
        unsafe { FreeMibTable(table.cast()) };
        interfaces.sort_by_key(|interface| interface.name.to_lowercase());
        (interfaces, current, true)
    }

    fn connected_ip_interfaces() -> Option<HashSet<u32>> {
        let mut table: *mut MIB_IPINTERFACE_TABLE = null_mut();
        if unsafe { GetIpInterfaceTable(AF_UNSPEC, &mut table) } != 0 || table.is_null() {
            return None;
        }
        // MIB_IPINTERFACE_TABLE also uses a variable-length row array.
        let count = unsafe { (*table).NumEntries as usize };
        let rows = unsafe {
            slice::from_raw_parts(
                addr_of!((*table).Table).cast::<MIB_IPINTERFACE_ROW>(),
                count,
            )
        };
        let connected = rows
            .iter()
            .filter(|row| row.Connected)
            .map(|row| row.InterfaceIndex)
            .collect();
        unsafe { FreeMibTable(table.cast()) };
        Some(connected)
    }

    fn utf16_string(value: &[u16]) -> String {
        let length = value
            .iter()
            .position(|unit| *unit == 0)
            .unwrap_or(value.len());
        String::from_utf16_lossy(&value[..length])
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn gpu_sample_reports_percentages_within_bounds() {
            let mut sampler = Sampler::default();
            let _ = sampler.sample();
            std::thread::sleep(std::time::Duration::from_millis(250));
            let metrics = sampler.sample().expect("resource sample");
            assert!(
                (0.0..=100.0).contains(&metrics.gpu_utilization_pct),
                "unexpected GPU load {}",
                metrics.gpu_utilization_pct
            );
            if metrics.gpu_available {
                assert!(
                    !metrics.gpu_engines.is_empty() || metrics.gpu_memory_used > 0,
                    "available GPU must expose engines or memory"
                );
            }
            assert!(
                metrics
                    .gpu_engines
                    .iter()
                    .all(|engine| (0.0..=100.0).contains(&engine.utilization_pct)),
                "engine load out of range"
            );
        }

        #[test]
        fn engine_types_are_extracted_from_instance_names() {
            assert_eq!(
                engine_type("pid_1364_luid_0x00000000_0x0000C4B6_phys_0_eng_0_engtype_3D"),
                Some("3D")
            );
            assert_eq!(
                engine_type("pid_9_luid_0x0_0x0_phys_0_eng_0_engtype_VideoDecode"),
                Some("VideoDecode")
            );
            assert_eq!(engine_type("pid_1_luid_0x0"), None);
        }
    }
}

#[cfg(not(windows))]
mod platform {
    use super::*;

    #[derive(Debug, Default)]
    pub(super) struct Sampler;

    impl Sampler {
        pub(super) fn sample(&mut self) -> io::Result<ResourceMetrics> {
            Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "Disk and network sampling is Windows-only",
            ))
        }
    }
}
