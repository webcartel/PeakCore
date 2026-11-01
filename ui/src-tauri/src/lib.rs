use peakcore_core::{
    HistoryPoint, IpcReply, MinuteAggregate, ProcSample, ResourceMetrics, SysSnapshot,
};
use peakcore_sys::{ProcessSampler, ResourceSampler};
use serde::Serialize;
#[cfg(windows)]
use std::ffi::c_void;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Emitter, Manager, State};

mod icons;

struct SamplingController {
    stop: Mutex<Option<Arc<AtomicBool>>>,
    worker: Mutex<Option<JoinHandle<()>>>,
    history_range: Arc<Mutex<String>>,
    process_sampler: Arc<Mutex<ProcessSampler>>,
    resource_sampler: Arc<Mutex<ResourceSampler>>,
    interval_ms: Arc<AtomicU64>,
    icons: Mutex<icons::IconCache>,
}

impl Default for SamplingController {
    fn default() -> Self {
        Self {
            stop: Mutex::new(None),
            worker: Mutex::new(None),
            history_range: Arc::new(Mutex::new("5m".into())),
            process_sampler: Arc::new(Mutex::new(ProcessSampler::default())),
            resource_sampler: Arc::new(Mutex::new(ResourceSampler::default())),
            interval_ms: Arc::new(AtomicU64::new(1_000)),
            icons: Mutex::new(icons::IconCache::default()),
        }
    }
}

#[derive(Clone, Debug, Default, Serialize)]
struct DashboardUpdate {
    ts_ms: u64,
    snapshot: Option<SysSnapshot>,
    history: Vec<HistoryPoint>,
    aggregates: Vec<MinuteAggregate>,
    processes: Vec<ProcSample>,
    resources: Option<ResourceMetrics>,
    daemon_connected: bool,
    error: Option<String>,
}

#[tauri::command]
fn start_sampling(app: AppHandle, state: State<'_, SamplingController>) {
    let mut worker = state
        .worker
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    if worker.is_some() {
        return;
    }

    let stop = Arc::new(AtomicBool::new(false));
    let history_range = Arc::clone(&state.history_range);
    let process_sampler = Arc::clone(&state.process_sampler);
    let resource_sampler = Arc::clone(&state.resource_sampler);
    let interval_ms = Arc::clone(&state.interval_ms);
    *state
        .stop
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(Arc::clone(&stop));
    *worker = Some(thread::spawn(move || {
        let mut next_daemon_poll = Instant::now();
        let mut retry_delay = Duration::from_secs(1);
        while !stop.load(Ordering::Relaxed) {
            let mut update = DashboardUpdate {
                ts_ms: SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis() as u64,
                ..DashboardUpdate::default()
            };

            match process_sampler
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .sample()
            {
                Ok(processes) => update.processes = processes,
                Err(error) => update.error = Some(error.to_string()),
            }
            match resource_sampler
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .sample()
            {
                Ok(resources) => update.resources = Some(resources),
                Err(error) => update.error = Some(error.to_string()),
            }
            if Instant::now() >= next_daemon_poll {
                match peakcore_core::ipc_client::request("snapshot", None) {
                    Ok(reply) if reply.ok => {
                        update.snapshot = reply
                            .data
                            .and_then(|data| serde_json::from_value(data).ok());
                        update.daemon_connected = update.snapshot.is_some();
                        if update.daemon_connected {
                            retry_delay = Duration::from_secs(1);
                            let range = history_range
                                .lock()
                                .unwrap_or_else(std::sync::PoisonError::into_inner)
                                .clone();
                            if let Ok(reply) =
                                peakcore_core::ipc_client::request("history", Some(&range))
                            {
                                if reply.ok {
                                    if let Some(data) = reply.data {
                                        if let Ok((points, aggregates)) =
                                            history_payload(&range, data)
                                        {
                                            update.history = points;
                                            update.aggregates = aggregates;
                                        }
                                    }
                                }
                            }
                        }
                    }
                    Ok(reply) => update.error = reply.error,
                    Err(error) => {
                        update.error = Some(error.to_string());
                        retry_delay = Duration::from_secs(5);
                    }
                }
                next_daemon_poll = Instant::now() + retry_delay;
            }
            let _ = app.emit("dashboard-update", update);
            let interval = interval_ms.load(Ordering::Relaxed).clamp(250, 60_000);
            thread::sleep(Duration::from_millis(interval));
        }
    }));
}

#[tauri::command]
fn set_history_range(
    range: String,
    state: State<'_, SamplingController>,
) -> Result<Vec<HistoryPoint>, String> {
    if !matches!(range.as_str(), "5m" | "15m" | "1h" | "24h") {
        return Err("History range must be 5m, 15m, 1h, or 24h".into());
    }
    *state
        .history_range
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner) = range.clone();
    let reply = peakcore_core::ipc_client::request("history", Some(&range))
        .map_err(|error| error.to_string())?;
    if !reply.ok {
        return Err(reply
            .error
            .unwrap_or_else(|| "History request failed".into()));
    }
    let data = reply
        .data
        .ok_or_else(|| "History response did not contain data".to_owned())?;
    history_payload(&range, data).map(|(points, _)| points)
}

/// A day is answered with minute aggregates; the chart plots their averages but
/// keeps the raw aggregates so the page can switch to maximums and show coverage.
fn history_payload(
    range: &str,
    data: serde_json::Value,
) -> Result<(Vec<HistoryPoint>, Vec<MinuteAggregate>), String> {
    if range == "24h" {
        let minutes: Vec<MinuteAggregate> =
            serde_json::from_value(data).map_err(|error| error.to_string())?;
        let points = minutes
            .iter()
            .map(|minute| HistoryPoint {
                ts_ms: minute.minute_ts_ms,
                cpu: minute.cpu_avg,
                mem_pct: minute.mem_avg,
            })
            .collect();
        return Ok((points, minutes));
    }
    let points: Vec<HistoryPoint> =
        serde_json::from_value(data).map_err(|error| error.to_string())?;
    Ok((points, Vec::new()))
}

#[tauri::command]
fn get_process_path(
    pid: u32,
    state: State<'_, SamplingController>,
) -> Result<Option<String>, String> {
    Ok(state
        .process_sampler
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .executable_path(pid))
}

#[tauri::command]
fn set_sampling_interval(interval_ms: u64, state: State<'_, SamplingController>) {
    state
        .interval_ms
        .store(interval_ms.clamp(250, 60_000), Ordering::Relaxed);
}

fn ipc_data(reply: IpcReply, operation: &str) -> Result<serde_json::Value, String> {
    if !reply.ok {
        return Err(reply.error.unwrap_or_else(|| format!("{operation} failed")));
    }
    reply
        .data
        .ok_or_else(|| format!("{operation} response did not contain data"))
}

#[tauri::command]
fn get_config() -> Result<serde_json::Value, String> {
    let reply = peakcore_core::ipc_client::request("config", None)
        .map_err(|error| format!("daemon is not reachable: {error}"))?;
    ipc_data(reply, "config")
}

#[tauri::command]
fn save_config(config: serde_json::Value) -> Result<serde_json::Value, String> {
    let reply = peakcore_core::ipc_client::request_data("config_set", config)
        .map_err(|error| format!("daemon is not reachable: {error}"))?;
    ipc_data(reply, "config_set")
}

#[tauri::command]
fn get_alerts() -> Result<serde_json::Value, String> {
    let reply = peakcore_core::ipc_client::request("alerts", None)
        .map_err(|error| format!("daemon is not reachable: {error}"))?;
    ipc_data(reply, "alerts")
}

#[tauri::command]
fn get_process_icon(pid: u32, state: State<'_, SamplingController>) -> Option<String> {
    let path = state
        .process_sampler
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .executable_path(pid)?;
    state
        .icons
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .data_url(&path)
}

#[tauri::command]
fn stop_sampling(state: State<'_, SamplingController>) {
    if let Some(stop) = state
        .stop
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .take()
    {
        stop.store(true, Ordering::Relaxed);
    }
    if let Some(worker) = state
        .worker
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .take()
    {
        let _ = worker.join();
    }
}

#[cfg(windows)]
fn wait_for_dashboard_exit() {
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn OpenEventW(access: u32, inherit: i32, name: *const u16) -> *mut c_void;
        fn WaitForSingleObject(handle: *mut c_void, milliseconds: u32) -> u32;
        fn CloseHandle(handle: *mut c_void) -> i32;
    }

    let name: Vec<u16> = "Local\\PeakCoreUiExit\0".encode_utf16().collect();
    loop {
        // The daemon creates the event before launching the UI. Keep the opened
        // handle alive while waiting so it survives the daemon's own exit.
        let event = unsafe { OpenEventW(0x0010_0000, 0, name.as_ptr()) };
        if !event.is_null() {
            unsafe {
                WaitForSingleObject(event, u32::MAX);
                CloseHandle(event);
            }
            return;
        }
        thread::sleep(Duration::from_millis(250));
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.show();
                let _ = window.set_focus();
            }
        }))
        .setup(|app| {
            #[cfg(windows)]
            {
                let app = app.handle().clone();
                thread::spawn(move || {
                    wait_for_dashboard_exit();
                    app.exit(0);
                });
            }
            Ok(())
        })
        .manage(SamplingController::default())
        .invoke_handler(tauri::generate_handler![
            start_sampling,
            stop_sampling,
            set_sampling_interval,
            set_history_range,
            get_process_path,
            get_process_icon,
            get_config,
            save_config,
            get_alerts
        ])
        .run(tauri::generate_context!())
        .expect("error while running PeakCore UI");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn day_history_is_mapped_from_minute_aggregates() {
        let data = serde_json::json!([
            {
                "minute_ts_ms": 60_000,
                "cpu_avg": 10.0,
                "cpu_max": 20.0,
                "mem_avg": 30.0,
                "mem_max": 40.0,
                "samples": 60
            }
        ]);
        let (points, aggregates) = history_payload("24h", data).expect("day payload");
        assert_eq!(
            points,
            vec![HistoryPoint {
                ts_ms: 60_000,
                cpu: 10.0,
                mem_pct: 30.0,
            }]
        );
        assert_eq!(aggregates.len(), 1);
        assert_eq!(aggregates[0].samples, 60);
        assert_eq!(aggregates[0].cpu_max, 20.0);
    }

    #[test]
    fn short_history_keeps_detailed_points() {
        let data = serde_json::json!([{ "ts_ms": 1_000, "cpu": 5.0, "mem_pct": 6.0 }]);
        let (points, aggregates) = history_payload("15m", data).expect("detailed payload");
        assert_eq!(
            points,
            vec![HistoryPoint {
                ts_ms: 1_000,
                cpu: 5.0,
                mem_pct: 6.0,
            }]
        );
        assert!(aggregates.is_empty());
    }

    #[test]
    fn a_payload_with_the_wrong_shape_is_rejected() {
        let data = serde_json::json!([{ "ts_ms": 1, "cpu": 1.0, "mem_pct": 1.0 }]);
        assert!(history_payload("24h", data).is_err());
    }
}
