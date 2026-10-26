# PeakCore

PeakCore is a lightweight Windows 10/11 resource monitor. The tray daemon is
designed to stay within a 5–8 MB working set, use less than 0.1% CPU on average,
and start in under 100 ms. The dashboard is a separate process and runs only
while it is open.

Milestones M0 and M1 are implemented. The tray and IPC implementations are in
place; M4 process sampling is available and the first M5 dashboard shell is
scaffolded. The overview now consumes daemon snapshots/history and live process
samples; clearly marked demo values are shown until the first update arrives.
The Processes page supports application grouping, search, sorting, virtualized
rows, and a context menu for copying a PID or executable path. Disk read/write
rates and busy time plus per-interface network throughput are sampled only while
the dashboard is open and shown on the Disk and Network pages. Network rows
exclude NDIS filter layers and interfaces Windows marks disconnected at the IP
layer. The Disk page reports
unavailable PDH counters explicitly instead of displaying false zeroes.
Alerts and settings pages remain placeholders. Executable icons,
manual DPI checks, and review in both themes remain.
`peakcore-core`
contains the system snapshot models, bounded history buffers, minute aggregates,
and IPC client. On Windows,
`peakcore-sys` samples total CPU with `GetSystemTimes` and memory with
`GlobalMemoryStatusEx` / `GetPerformanceInfo`. Run `peakcore --print` to print
CPU and RAM values once per second. Process rankings are available through
`peakcore top --by cpu|mem -n 10`; the overview consumes live snapshots and
process samples, and the Processes page shows live process and application
rankings. The table virtualizes visible rows; executable icons are still pending. The
default `peakcore` launch starts the
tray process; its menu supports icon mode, refresh interval,
per-user startup, and opening the configuration file. Use
`./scripts/measure.ps1 -Tray` to measure the live tray process.

The daemon exposes a per-user named pipe using versioned JSON frames. While it
is running, `peakcore stats [--json]`, `peakcore history --minutes N --csv`,
`peakcore show`, and `peakcore shutdown` communicate with it. History ranges
through IPC are bounded to 1 MiB per frame and read time out after two seconds.

## Development

Install the stable Rust toolchain with the `rustfmt` and `clippy` components.
Then run:

```powershell
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo build --workspace --release
```

On Windows, measure the idle daemon process with:

```powershell
./scripts/measure.ps1
```

Build the daemon and dashboard as a paired portable release with:

```powershell
./scripts/build-release.ps1
```

The script places `peakcore-ui.exe` beside `peakcore.exe` in `target/release`,
so the tray can start the dashboard from the packaged folder.

The measurement currently exercises a minimal idle-process harness; it is not
yet representative of the finished tray daemon. See [the project specification](docs/peakcore-spec.md)
for architecture, resource budgets, and milestone acceptance criteria.

The UI project is under `ui/` and uses Tauri v2, Vue 3, TypeScript and Vite.
To install and run it, use `npm install` followed by `npm run tauri dev` from
that directory. The first dashboard pass provides the frameless window shell,
single-instance behavior, navigation, theme toggle, and live overview updates.
Manual visual and DPI checks are still outstanding.
