//! Shared models and history logic for PeakCore.

use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct SysSnapshot {
    pub ts_ms: u64,
    pub cpu_total: f32,
    pub cpu_cores: Vec<f32>,
    pub mem: MemInfo,
    /// Bumped by the daemon whenever it applies a new configuration.
    #[serde(default)]
    pub config_revision: u64,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct MemInfo {
    pub total: u64,
    pub used: u64,
    pub available: u64,
    pub cached: u64,
    pub commit_used: u64,
    pub commit_limit: u64,
    pub pagefile_used: u64,
    pub pagefile_total: u64,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct HistoryPoint {
    pub ts_ms: u64,
    pub cpu: f32,
    pub mem_pct: f32,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ResourceMetrics {
    pub ts_ms: u64,
    pub disk_available: bool,
    pub disk_busy_available: bool,
    pub disk_read_bps: u64,
    pub disk_write_bps: u64,
    pub disk_busy_pct: f32,
    pub network_rx_bps: u64,
    pub network_tx_bps: u64,
    pub network_available: bool,
    pub interfaces: Vec<NetworkInterfaceMetrics>,
    pub gpu_available: bool,
    pub gpu_utilization_pct: f32,
    pub gpu_memory_used: u64,
    pub gpu_engines: Vec<GpuEngineMetrics>,
    pub gpu_processes: Vec<GpuProcessMetrics>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct GpuEngineMetrics {
    pub name: String,
    pub utilization_pct: f32,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct GpuProcessMetrics {
    pub pid: u32,
    pub utilization_pct: f32,
    pub memory: u64,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct NetworkInterfaceMetrics {
    pub id: u64,
    pub index: u32,
    pub name: String,
    pub rx_bps: u64,
    pub tx_bps: u64,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct MinuteAggregate {
    pub minute_ts_ms: u64,
    pub cpu_avg: f32,
    pub cpu_max: f32,
    pub mem_avg: f32,
    pub mem_max: f32,
    /// How many samples the minute actually contains; a suspended machine
    /// leaves minutes with far fewer than a full interval's worth.
    pub samples: u32,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ProcSample {
    pub pid: u32,
    pub ppid: u32,
    pub name: String,
    pub exe_path: Option<String>,
    pub cpu: f32,
    pub private_bytes: u64,
    pub working_set: u64,
    pub io_read_bps: u64,
    pub io_write_bps: u64,
    pub threads: u32,
    pub handles: u32,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct AppGroup {
    pub name: String,
    pub pids: Vec<u32>,
    pub cpu: f32,
    pub private_bytes: u64,
    pub working_set: u64,
    pub io_read_bps: u64,
    pub io_write_bps: u64,
}

/// A bounded FIFO that retains the newest `capacity` values.
#[derive(Clone, Debug)]
pub struct RingBuffer<T> {
    capacity: usize,
    values: VecDeque<T>,
}

impl<T> RingBuffer<T> {
    pub fn new(capacity: usize) -> Self {
        assert!(capacity > 0, "ring buffer capacity must be nonzero");
        Self {
            capacity,
            values: VecDeque::with_capacity(capacity),
        }
    }

    pub fn push(&mut self, value: T) {
        if self.values.len() == self.capacity {
            self.values.pop_front();
        }
        self.values.push_back(value);
    }

    pub fn len(&self) -> usize {
        self.values.len()
    }

    pub fn capacity(&self) -> usize {
        self.capacity
    }

    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    pub fn front(&self) -> Option<&T> {
        self.values.front()
    }

    pub fn last(&self) -> Option<&T> {
        self.values.back()
    }

    pub fn pop_front(&mut self) -> Option<T> {
        self.values.pop_front()
    }

    pub fn iter(&self) -> impl DoubleEndedIterator<Item = &T> + ExactSizeIterator {
        self.values.iter()
    }
}

/// The daemon samples at 500 ms at the fastest, so a window sized in time
/// bounds the detailed buffer even when the interval changes.
pub const MIN_SAMPLE_INTERVAL_MS: u64 = 500;
pub const DETAIL_WINDOW_MS: u64 = 60 * 60_000;
pub const AGGREGATE_WINDOW_MS: u64 = 24 * 60 * 60_000;
pub const MAX_IPC_FRAME_SIZE: usize = 1_048_576;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct IpcRequest {
    pub v: u32,
    pub op: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub range: Option<String>,
    /// Operation payload, used by `config_set`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
}

impl IpcRequest {
    pub fn new(op: impl Into<String>) -> Self {
        Self {
            v: 1,
            op: op.into(),
            range: None,
            data: None,
        }
    }

    pub fn with_range(mut self, range: impl Into<String>) -> Self {
        self.range = Some(range.into());
        self
    }

    pub fn with_data(mut self, data: serde_json::Value) -> Self {
        self.data = Some(data);
        self
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct IpcReply {
    pub v: u32,
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl IpcReply {
    pub fn success(data: serde_json::Value) -> Self {
        Self {
            v: 1,
            ok: true,
            data: Some(data),
            error: None,
        }
    }

    pub fn failure(error: impl Into<String>) -> Self {
        Self {
            v: 1,
            ok: false,
            data: None,
            error: Some(error.into()),
        }
    }
}

pub fn encode_frame<T: Serialize>(value: &T) -> std::io::Result<Vec<u8>> {
    let payload = serde_json::to_vec(value).map_err(std::io::Error::other)?;
    if payload.len() > MAX_IPC_FRAME_SIZE {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "IPC frame exceeds 1 MiB",
        ));
    }
    let mut frame = Vec::with_capacity(payload.len() + 4);
    frame.extend_from_slice(&(payload.len() as u32).to_le_bytes());
    frame.extend_from_slice(&payload);
    Ok(frame)
}

pub fn decode_payload<T: for<'de> Deserialize<'de>>(payload: &[u8]) -> std::io::Result<T> {
    if payload.len() > MAX_IPC_FRAME_SIZE {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "IPC frame exceeds 1 MiB",
        ));
    }
    serde_json::from_slice(payload).map_err(std::io::Error::other)
}

pub mod alerts;
pub mod config;
pub mod ipc_client;

#[derive(Clone, Debug)]
pub struct History {
    detailed: RingBuffer<HistoryPoint>,
    minutes: RingBuffer<MinuteAggregate>,
    current_minute: Option<MinuteBuilder>,
    detail_window_ms: u64,
    aggregate_window_ms: u64,
}

impl Default for History {
    fn default() -> Self {
        Self::new()
    }
}

impl History {
    /// One hour of detail plus 24 hours of minute aggregates, all in memory.
    pub fn new() -> Self {
        Self::with_windows(DETAIL_WINDOW_MS, AGGREGATE_WINDOW_MS)
    }

    /// Builds a history bounded by two time windows; nothing is ever written to
    /// disk, so these windows are the whole memory budget.
    pub fn with_windows(detail_window_ms: u64, aggregate_window_ms: u64) -> Self {
        let detail_window_ms = detail_window_ms.max(MIN_SAMPLE_INTERVAL_MS);
        let aggregate_window_ms = aggregate_window_ms.max(60_000);
        Self {
            detailed: RingBuffer::new(detail_capacity(detail_window_ms)),
            minutes: RingBuffer::new(minute_capacity(aggregate_window_ms)),
            current_minute: None,
            detail_window_ms,
            aggregate_window_ms,
        }
    }

    pub fn detail_capacity(&self) -> usize {
        self.detailed.capacity()
    }

    pub fn minute_capacity(&self) -> usize {
        self.minutes.capacity()
    }

    pub fn push(&mut self, point: HistoryPoint) {
        self.detailed.push(point);
        while self
            .detailed
            .front()
            .is_some_and(|oldest| point.ts_ms.saturating_sub(oldest.ts_ms) > self.detail_window_ms)
        {
            self.detailed.pop_front();
        }
        let minute_ts_ms = point.ts_ms / 60_000 * 60_000;
        match self.current_minute {
            Some(mut current) if current.minute_ts_ms == minute_ts_ms => {
                current.add(point);
                self.current_minute = Some(current);
            }
            Some(current) => {
                self.minutes.push(current.finish());
                self.current_minute = Some(MinuteBuilder::from_point(minute_ts_ms, point));
            }
            None => self.current_minute = Some(MinuteBuilder::from_point(minute_ts_ms, point)),
        }
        while self.minutes.last().is_some_and(|latest| {
            self.minutes.front().is_some_and(|oldest| {
                latest.minute_ts_ms.saturating_sub(oldest.minute_ts_ms) > self.aggregate_window_ms
            })
        }) {
            self.minutes.pop_front();
        }
    }

    pub fn detailed(&self) -> impl DoubleEndedIterator<Item = &HistoryPoint> + ExactSizeIterator {
        self.detailed.iter()
    }

    pub fn minutes(&self) -> impl DoubleEndedIterator<Item = &MinuteAggregate> + ExactSizeIterator {
        self.minutes.iter()
    }
}

fn detail_capacity(window_ms: u64) -> usize {
    (window_ms / MIN_SAMPLE_INTERVAL_MS).max(1) as usize
}

fn minute_capacity(window_ms: u64) -> usize {
    (window_ms / 60_000 + 1).max(1) as usize
}

#[derive(Clone, Copy, Debug)]
struct MinuteBuilder {
    minute_ts_ms: u64,
    count: u64,
    cpu_sum: f64,
    cpu_max: f32,
    mem_sum: f64,
    mem_max: f32,
}

impl MinuteBuilder {
    fn from_point(minute_ts_ms: u64, point: HistoryPoint) -> Self {
        Self {
            minute_ts_ms,
            count: 1,
            cpu_sum: f64::from(point.cpu),
            cpu_max: point.cpu,
            mem_sum: f64::from(point.mem_pct),
            mem_max: point.mem_pct,
        }
    }

    fn add(&mut self, point: HistoryPoint) {
        self.count += 1;
        self.cpu_sum += f64::from(point.cpu);
        self.cpu_max = self.cpu_max.max(point.cpu);
        self.mem_sum += f64::from(point.mem_pct);
        self.mem_max = self.mem_max.max(point.mem_pct);
    }

    fn finish(self) -> MinuteAggregate {
        MinuteAggregate {
            minute_ts_ms: self.minute_ts_ms,
            cpu_avg: (self.cpu_sum / self.count as f64) as f32,
            cpu_max: self.cpu_max,
            mem_avg: (self.mem_sum / self.count as f64) as f32,
            mem_max: self.mem_max,
            samples: self.count.min(u64::from(u32::MAX)) as u32,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ring_buffer_discards_oldest_and_preserves_order() {
        let mut ring = RingBuffer::new(3);
        for value in 1..=4 {
            ring.push(value);
        }
        assert_eq!(ring.iter().copied().collect::<Vec<_>>(), [2, 3, 4]);
    }

    #[test]
    fn detailed_history_keeps_only_the_configured_window() {
        let mut history = History::with_windows(60_000, 3_600_000);
        for ts_ms in [0, 30_000, 60_001] {
            history.push(HistoryPoint {
                ts_ms,
                cpu: 1.0,
                mem_pct: 2.0,
            });
        }
        let kept: Vec<u64> = history.detailed().map(|point| point.ts_ms).collect();
        assert_eq!(kept, [30_000, 60_001]);
    }

    #[test]
    fn minute_aggregates_keep_only_the_configured_window() {
        let mut history = History::with_windows(3_600_000, 120_000);
        for minute in 0..5u64 {
            let start = minute * 60_000;
            history.push(HistoryPoint {
                ts_ms: start,
                cpu: 10.0,
                mem_pct: 20.0,
            });
            history.push(HistoryPoint {
                ts_ms: start + 30_000,
                cpu: 30.0,
                mem_pct: 40.0,
            });
        }
        let kept: Vec<u64> = history
            .minutes()
            .map(|aggregate| aggregate.minute_ts_ms)
            .collect();
        // The last minute (4) is still open, so the newest aggregate is 3 (180s).
        assert_eq!(kept, [60_000, 120_000, 180_000]);
    }

    #[test]
    fn history_capacities_cover_the_windows_at_the_fastest_interval() {
        let history = History::with_windows(60 * 60_000, 24 * 60 * 60_000);
        assert_eq!(history.detail_capacity(), 7_200);
        assert_eq!(history.minute_capacity(), 1_441);
    }

    #[test]
    fn history_emits_minute_aggregates_on_minute_change() {
        let mut history = History::new();
        history.push(HistoryPoint {
            ts_ms: 1_000,
            cpu: 10.0,
            mem_pct: 20.0,
        });
        history.push(HistoryPoint {
            ts_ms: 30_000,
            cpu: 30.0,
            mem_pct: 40.0,
        });
        assert_eq!(history.minutes().count(), 0);
        history.push(HistoryPoint {
            ts_ms: 60_000,
            cpu: 50.0,
            mem_pct: 60.0,
        });
        let aggregate = history.minutes().next().unwrap();
        assert_eq!(aggregate.minute_ts_ms, 0);
        assert_eq!(aggregate.cpu_avg, 20.0);
        assert_eq!(aggregate.cpu_max, 30.0);
        assert_eq!(aggregate.mem_avg, 30.0);
        assert_eq!(aggregate.mem_max, 40.0);
        assert_eq!(aggregate.samples, 2);
    }

    #[test]
    fn minute_aggregate_counts_samples_even_for_a_single_point() {
        let mut history = History::new();
        // A suspended machine produced one point in this minute, not sixty.
        history.push(HistoryPoint {
            ts_ms: 1_000,
            cpu: 5.0,
            mem_pct: 50.0,
        });
        history.push(HistoryPoint {
            ts_ms: 120_000,
            cpu: 5.0,
            mem_pct: 50.0,
        });
        let aggregate = history.minutes().next().unwrap();
        assert_eq!(aggregate.minute_ts_ms, 0);
        assert_eq!(aggregate.samples, 1);
    }

    #[test]
    fn a_full_day_of_history_fits_into_one_ipc_frame() {
        // 24 hours at the fastest interval fills both buffers to their limits.
        let mut history = History::with_windows(60 * 60_000, 24 * 60 * 60_000);
        let steps = 24 * 60 * 60 * 2u64;
        for step in 0..steps {
            history.push(HistoryPoint {
                ts_ms: step * 500,
                cpu: 42.125,
                mem_pct: 61.937_5,
            });
        }
        assert_eq!(history.detailed().count(), 7_200);
        assert!(
            history.minutes().count() >= 1_439,
            "only {} aggregates kept",
            history.minutes().count()
        );

        let detailed: Vec<HistoryPoint> = history.detailed().copied().collect();
        let minutes: Vec<MinuteAggregate> = history.minutes().copied().collect();
        let detailed_size = serde_json::to_vec(&detailed).expect("detail json").len();
        let minutes_size = serde_json::to_vec(&minutes).expect("aggregate json").len();
        assert!(
            detailed_size < MAX_IPC_FRAME_SIZE,
            "detail payload is {detailed_size} bytes"
        );
        assert!(
            minutes_size < MAX_IPC_FRAME_SIZE,
            "aggregate payload is {minutes_size} bytes"
        );
        // The server writes these exact payloads into one frame each.
        assert!(encode_frame(&detailed).is_ok());
        assert!(encode_frame(&minutes).is_ok());
        println!(
            "history payloads: {detailed_size} B detailed, {minutes_size} B aggregates (limit {MAX_IPC_FRAME_SIZE} B)"
        );
    }

    #[test]
    fn ipc_frame_uses_little_endian_length_and_json_payload() {
        let request = IpcRequest::new("ping");
        let frame = encode_frame(&request).unwrap();
        let length = u32::from_le_bytes(frame[..4].try_into().unwrap()) as usize;
        assert_eq!(length, frame.len() - 4);
        let decoded: IpcRequest = decode_payload(&frame[4..]).unwrap();
        assert_eq!(decoded, request);
    }

    #[test]
    fn ipc_request_payload_round_trips_and_omits_missing_fields() {
        let request = IpcRequest::new("config_set")
            .with_data(serde_json::json!({ "ui": { "theme": "dark" } }));
        let payload = serde_json::to_vec(&request).unwrap();
        let text = String::from_utf8(payload.clone()).unwrap();
        assert!(!text.contains("range"), "unused fields stay out of frames");
        let decoded: IpcRequest = serde_json::from_slice(&payload).unwrap();
        assert_eq!(decoded, request);

        let legacy: IpcRequest = serde_json::from_str(r#"{"v":1,"op":"ping"}"#).unwrap();
        assert_eq!(legacy.data, None);
        assert_eq!(legacy.range, None);
    }

    #[test]
    fn ipc_reply_round_trips_success_and_failure() {
        let success = IpcReply::success(serde_json::json!({ "pong": true }));
        let payload = serde_json::to_vec(&success).unwrap();
        assert_eq!(
            serde_json::from_slice::<IpcReply>(&payload).unwrap(),
            success
        );
        let failure = IpcReply::failure("unknown operation");
        let payload = serde_json::to_vec(&failure).unwrap();
        assert_eq!(
            serde_json::from_slice::<IpcReply>(&payload).unwrap(),
            failure
        );
    }

    #[test]
    fn ipc_rejects_frames_larger_than_one_megabyte() {
        let oversized = "x".repeat(MAX_IPC_FRAME_SIZE);
        let error = encode_frame(&oversized).unwrap_err();
        assert_eq!(error.kind(), std::io::ErrorKind::InvalidData);
        assert_eq!(
            decode_payload::<String>(&vec![b'x'; MAX_IPC_FRAME_SIZE + 1])
                .unwrap_err()
                .kind(),
            std::io::ErrorKind::InvalidData
        );
    }
}
