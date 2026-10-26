#![cfg_attr(windows, windows_subsystem = "windows")]

use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

fn main() {
    let mut args = std::env::args().skip(1);
    #[cfg(windows)]
    let first_arg = {
        let argument = args.next();
        if argument.as_deref().is_some_and(|value| value != "--tray") {
            attach_cli_console();
        }
        argument
    };
    #[cfg(not(windows))]
    let first_arg = args.next();

    match first_arg.as_deref() {
        Some("--measure-idle") => measure_idle(),
        Some("--print") => print_snapshots(),
        Some("--tray") | None => tray::run(),
        Some("stats") => stats(args.collect()),
        Some("history") => history(args.collect()),
        Some("top") => top(args.collect()),
        Some("show") => simple_request("show_window"),
        Some("shutdown") => simple_request("shutdown"),
        Some("--help") | Some("-h") | Some("help") => print_help(),
        Some(command) => {
            eprintln!("Unknown argument: {command}");
            print_help();
            std::process::exit(2);
        }
    }
}

#[cfg(windows)]
fn attach_cli_console() {
    const ATTACH_PARENT_PROCESS: u32 = u32::MAX;

    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn AttachConsole(process_id: u32) -> i32;
        fn AllocConsole() -> i32;
        fn GetStdHandle(std_handle: i32) -> *mut std::ffi::c_void;
        fn SetStdHandle(std_handle: i32, handle: *mut std::ffi::c_void) -> i32;
    }

    // GUI-subsystem builds have no console by default. CLI invocations attach
    // to their launching terminal, or create a console when started elsewhere.
    let attached = unsafe { AttachConsole(ATTACH_PARENT_PROCESS) };
    if attached == 0 {
        unsafe {
            AllocConsole();
        }
    }
    // GUI-subsystem executables can start with null standard handles. Refresh
    // Rust's process handles after attaching/allocating the CLI console.
    for standard_handle in [-10, -11, -12] {
        let handle = unsafe { GetStdHandle(standard_handle) };
        if !handle.is_null() {
            unsafe {
                SetStdHandle(standard_handle, handle);
            }
        }
    }
}

mod ipc;
mod state;
mod tray;

fn print_help() {
    println!(
        "PeakCore resource monitor\n\nUsage: peakcore [--tray]\n       peakcore stats [--json]\n       peakcore history --minutes N --csv\n       peakcore top --by cpu|mem -n 10\n       peakcore show | shutdown\n       peakcore --print\n       peakcore --help"
    );
}

fn simple_request(op: &str) {
    match peakcore_core::ipc_client::request(op, None) {
        Ok(reply) if reply.ok => println!("OK"),
        Ok(reply) => report_reply_error(reply.error.as_deref().unwrap_or("IPC request failed")),
        Err(error) => report_connection_error(&error),
    }
}

fn stats(args: Vec<String>) {
    let json = args.iter().any(|arg| arg == "--json");
    match peakcore_core::ipc_client::request("snapshot", None) {
        Ok(reply) if reply.ok => {
            let data = reply.data.unwrap_or(serde_json::Value::Null);
            if json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&data).unwrap_or_default()
                );
            } else {
                let cpu = data["cpu_total"].as_f64().unwrap_or(0.0);
                let used = data["mem"]["used"].as_u64().unwrap_or(0);
                let total = data["mem"]["total"].as_u64().unwrap_or(0);
                let percent = if total == 0 {
                    0.0
                } else {
                    used as f64 * 100.0 / total as f64
                };
                println!(
                    "CPU {cpu:.1}% | RAM {:.1}/{:.1} GiB ({percent:.1}%)",
                    used as f64 / 1_073_741_824.0,
                    total as f64 / 1_073_741_824.0
                );
            }
        }
        Ok(reply) => report_reply_error(reply.error.as_deref().unwrap_or("IPC request failed")),
        Err(error) => report_connection_error(&error),
    }
}

fn history(args: Vec<String>) {
    let mut minutes = None;
    let mut csv = false;
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--minutes" => {
                minutes = iter.next().and_then(|value| value.parse::<u32>().ok());
            }
            "--csv" => csv = true,
            other => {
                eprintln!("Unknown history option: {other}");
                std::process::exit(2);
            }
        }
    }
    let Some(minutes) = minutes.filter(|value| (1..=1_440).contains(value)) else {
        eprintln!("--minutes must be between 1 and 1440");
        std::process::exit(2);
    };
    let range = match minutes {
        1..=5 => "5m",
        6..=15 => "15m",
        16..=60 => "1h",
        _ => "24h",
    };
    match peakcore_core::ipc_client::request("history", Some(range)) {
        Ok(reply) if reply.ok => {
            let data = reply.data.unwrap_or(serde_json::Value::Null);
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64;
            let cutoff = now.saturating_sub(u64::from(minutes) * 60_000);
            let time_key = if minutes <= 60 {
                "ts_ms"
            } else {
                "minute_ts_ms"
            };
            let points: Vec<_> = data
                .as_array()
                .into_iter()
                .flatten()
                .filter(|point| point[time_key].as_u64().is_some_and(|ts| ts >= cutoff))
                .collect();
            if !csv {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&points).unwrap_or_default()
                );
            } else if minutes <= 60 {
                println!("ts_ms,cpu,mem_pct");
                for point in points {
                    println!("{},{},{}", point["ts_ms"], point["cpu"], point["mem_pct"]);
                }
            } else {
                println!("minute_ts_ms,cpu_avg,cpu_max,mem_avg,mem_max");
                for point in points {
                    println!(
                        "{},{},{},{},{}",
                        point["minute_ts_ms"],
                        point["cpu_avg"],
                        point["cpu_max"],
                        point["mem_avg"],
                        point["mem_max"]
                    );
                }
            }
        }
        Ok(reply) => report_reply_error(reply.error.as_deref().unwrap_or("IPC request failed")),
        Err(error) => report_connection_error(&error),
    }
}

fn report_reply_error(message: &str) {
    eprintln!("PeakCore: {message}");
    std::process::exit(1);
}

fn report_connection_error(error: &std::io::Error) {
    if error.kind() == std::io::ErrorKind::NotFound
        || error.kind() == std::io::ErrorKind::TimedOut
        || error.raw_os_error() == Some(2)
        || error.raw_os_error() == Some(121)
    {
        eprintln!("PeakCore daemon is not running. Start peakcore first.");
    } else {
        eprintln!("PeakCore IPC error: {error}");
    }
    std::process::exit(1);
}

fn top(args: Vec<String>) {
    let mut by = None;
    let mut count = 10usize;
    let mut show_timing = false;
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--by" => by = iter.next().map(String::as_str),
            "-n" | "--count" => {
                count = iter
                    .next()
                    .and_then(|value| value.parse().ok())
                    .unwrap_or(0);
            }
            "--timing" => show_timing = true,
            other => {
                eprintln!("Unknown top option: {other}");
                std::process::exit(2);
            }
        }
    }
    if count == 0 || count > 10_000 {
        eprintln!("-n must be between 1 and 10000");
        std::process::exit(2);
    }
    let sort = match by.unwrap_or("cpu") {
        "cpu" => peakcore_sys::ProcessSort::Cpu,
        "mem" => peakcore_sys::ProcessSort::Memory,
        other => {
            eprintln!("--by must be cpu or mem, got {other}");
            std::process::exit(2);
        }
    };
    let mut sampler = peakcore_sys::ProcessSampler::default();
    let first_started = Instant::now();
    if let Err(error) = sampler.sample() {
        eprintln!("Could not sample processes: {error}");
        std::process::exit(1);
    }
    let first_sample_ms = first_started.elapsed().as_secs_f64() * 1_000.0;
    std::thread::sleep(Duration::from_secs(1));
    let second_started = Instant::now();
    let samples = match sampler.sample() {
        Ok(samples) => samples,
        Err(error) => {
            eprintln!("Could not sample processes: {error}");
            std::process::exit(1);
        }
    };
    let second_sample_ms = second_started.elapsed().as_secs_f64() * 1_000.0;
    let processes = peakcore_sys::top_processes(samples, sort, count);
    if show_timing {
        eprintln!(
            "Process sampler: first {first_sample_ms:.2} ms, subsequent {second_sample_ms:.2} ms"
        );
    }
    println!("PID     CPU%   Private MiB  Working MiB  Name");
    for process in processes {
        println!(
            "{:<7} {:>5.1} {:>12.1} {:>12.1}  {}",
            process.pid,
            process.cpu,
            process.private_bytes as f64 / 1_048_576.0,
            process.working_set as f64 / 1_048_576.0,
            process.name
        );
    }
}

fn print_snapshots() {
    let mut cpu = peakcore_sys::CpuSampler::default();
    loop {
        let cpu_total = match cpu.sample() {
            Ok(Some(value)) => value,
            Ok(None) => 0.0,
            Err(error) => {
                eprintln!("Unable to sample CPU: {error}");
                std::process::exit(1);
            }
        };
        let mem = match peakcore_sys::mem_info() {
            Ok(value) => value,
            Err(error) => {
                eprintln!("Unable to sample memory: {error}");
                std::process::exit(1);
            }
        };
        let ts_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        println!(
            "{ts_ms} CPU {cpu_total:.1}% | RAM {:.1}/{:.1} GiB ({:.1}%) | cache {:.1} GiB | commit {:.1}/{:.1} GiB",
            mem.used as f64 / 1_073_741_824.0,
            mem.total as f64 / 1_073_741_824.0,
            if mem.total == 0 {
                0.0
            } else {
                mem.used as f64 * 100.0 / mem.total as f64
            },
            mem.cached as f64 / 1_073_741_824.0,
            mem.commit_used as f64 / 1_073_741_824.0,
            mem.commit_limit as f64 / 1_073_741_824.0
        );
        std::thread::sleep(Duration::from_secs(1));
    }
}

fn measure_idle() {
    let until = Instant::now() + Duration::from_secs(75);
    while Instant::now() < until {
        std::thread::sleep(Duration::from_millis(250));
    }
}
