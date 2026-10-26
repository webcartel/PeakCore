use peakcore_core::{HistoryPoint, ProcSample, ResourceMetrics, SysSnapshot};
use peakcore_sys::{ProcessSampler, ResourceSampler};
use serde::Serialize;
#[cfg(windows)]
use std::ffi::c_void;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Emitter, Manager, State};

#[derive(Default)]
struct SamplingController {
    stop: Mutex<Option<Arc<AtomicBool>>>,
    worker: Mutex<Option<JoinHandle<()>>>,
    history_range: Arc<Mutex<String>>,
    process_sampler: Arc<Mutex<ProcessSampler>>,
    resource_sampler: Arc<Mutex<ResourceSampler>>,
}

#[derive(Clone, Debug, Default, Serialize)]
struct DashboardUpdate {
    ts_ms: u64,
    snapshot: Option<SysSnapshot>,
    history: Vec<HistoryPoint>,
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
                                    update.history = reply
                                        .data
                                        .and_then(|data| serde_json::from_value(data).ok())
                                        .unwrap_or_default();
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
            thread::sleep(Duration::from_secs(1));
        }
    }));
}

#[tauri::command]
fn set_history_range(
    range: String,
    state: State<'_, SamplingController>,
) -> Result<Vec<HistoryPoint>, String> {
    if !matches!(range.as_str(), "5m" | "15m" | "1h") {
        return Err("History range must be 5m, 15m, or 1h".into());
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
    reply
        .data
        .ok_or_else(|| "History response did not contain data".to_owned())
        .and_then(|data| serde_json::from_value(data).map_err(|error| error.to_string()))
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
            set_history_range,
            get_process_path
        ])
        .run(tauri::generate_context!())
        .expect("error while running PeakCore UI");
}
