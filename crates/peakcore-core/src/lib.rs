//! Shared models and history logic for PeakCore.

use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct SysSnapshot {
    pub ts_ms: u64,
    pub cpu_total: f32,
    pub cpu_cores: Vec<f32>,
    pub mem: MemInfo,
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

    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    pub fn iter(&self) -> impl DoubleEndedIterator<Item = &T> + ExactSizeIterator {
        self.values.iter()
    }
}

pub const DETAIL_HISTORY_CAPACITY: usize = 3_600;
pub const MINUTE_HISTORY_CAPACITY: usize = 1_440;
pub const MAX_IPC_FRAME_SIZE: usize = 1_048_576;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct IpcRequest {
    pub v: u32,
    pub op: String,
    #[serde(default)]
    pub range: Option<String>,
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

pub mod ipc_client;

#[derive(Clone, Debug)]
pub struct History {
    detailed: RingBuffer<HistoryPoint>,
    minutes: RingBuffer<MinuteAggregate>,
    current_minute: Option<MinuteBuilder>,
}

impl Default for History {
    fn default() -> Self {
        Self::new()
    }
}

impl History {
    pub fn new() -> Self {
        Self {
            detailed: RingBuffer::new(DETAIL_HISTORY_CAPACITY),
            minutes: RingBuffer::new(MINUTE_HISTORY_CAPACITY),
            current_minute: None,
        }
    }

    pub fn push(&mut self, point: HistoryPoint) {
        self.detailed.push(point);
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
    }

    pub fn detailed(&self) -> impl DoubleEndedIterator<Item = &HistoryPoint> + ExactSizeIterator {
        self.detailed.iter()
    }

    pub fn minutes(&self) -> impl DoubleEndedIterator<Item = &MinuteAggregate> + ExactSizeIterator {
        self.minutes.iter()
    }
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
    }

    #[test]
    fn ipc_frame_uses_little_endian_length_and_json_payload() {
        let request = IpcRequest {
            v: 1,
            op: "ping".into(),
            range: None,
        };
        let frame = encode_frame(&request).unwrap();
        let length = u32::from_le_bytes(frame[..4].try_into().unwrap()) as usize;
        assert_eq!(length, frame.len() - 4);
        let decoded: IpcRequest = decode_payload(&frame[4..]).unwrap();
        assert_eq!(decoded, request);
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
