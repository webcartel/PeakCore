#[cfg(windows)]
mod platform {
    use peakcore_core::MemInfo;
    use peakcore_core::{HistoryPoint, SysSnapshot};
    use peakcore_sys::CpuSampler;
    use std::ffi::c_void;
    use std::mem::{size_of, zeroed};
    use std::ptr::{null, null_mut};
    use std::sync::{Mutex, OnceLock};
    use std::time::{Instant, SystemTime, UNIX_EPOCH};

    type Hwnd = *mut c_void;
    type Hicon = *mut c_void;
    type Hmenu = *mut c_void;
    type Hinstance = *mut c_void;
    type Hbitmap = *mut c_void;
    type Hkey = *mut c_void;
    type Wparam = usize;
    type Lparam = isize;
    type Lresult = isize;

    const WM_DESTROY: u32 = 0x0002;
    const WM_TIMER: u32 = 0x0113;
    const WM_USER: u32 = 0x0400;
    const WM_LBUTTONUP: isize = 0x0202;
    const WM_RBUTTONUP: isize = 0x0205;
    const WM_CONTEXTMENU: isize = 0x007B;
    const WM_CLOSE: u32 = 0x0010;
    const WM_APP_IPC: u32 = 0x8000 + 9;
    const NIM_ADD: u32 = 0;
    const NIM_MODIFY: u32 = 1;
    const NIM_DELETE: u32 = 2;
    const NIF_MESSAGE: u32 = 1;
    const NIF_ICON: u32 = 2;
    const NIF_TIP: u32 = 4;
    const IDI_APP: usize = 1;
    const TIMER_ID: usize = 1;
    const ID_STATS: u32 = 100;
    const ID_AUTOSTART: u32 = 101;
    const ID_CONFIG: u32 = 102;
    const ID_MODE_CPU_RAM: u32 = 110;
    const ID_MODE_CPU: u32 = 111;
    const ID_MODE_RAM: u32 = 112;
    const ID_INTERVAL_1: u32 = 120;
    const ID_INTERVAL_2: u32 = 121;
    const ID_INTERVAL_5: u32 = 122;
    const ID_EXIT: u32 = 199;
    const MF_STRING: u32 = 0;
    const MF_SEPARATOR: u32 = 0x800;
    const MF_CHECKED: u32 = 8;
    const TPM_RIGHTBUTTON: u32 = 2;
    const TPM_RETURNCMD: u32 = 0x100;
    const TPM_NONOTIFY: u32 = 0x80;
    const ERROR_ALREADY_EXISTS: u32 = 183;
    const EVENT_MODIFY_STATE: u32 = 0x0002;
    const HKEY_CURRENT_USER: Hkey = 0x8000_0001usize as Hkey;
    const KEY_QUERY_VALUE: u32 = 1;
    const KEY_SET_VALUE: u32 = 2;
    const REG_SZ: u32 = 1;
    const ERROR_SUCCESS: i32 = 0;

    #[repr(C)]
    struct Point {
        x: i32,
        y: i32,
    }

    #[repr(C)]
    struct Msg {
        hwnd: Hwnd,
        message: u32,
        w_param: Wparam,
        l_param: Lparam,
        time: u32,
        point: Point,
        private: u32,
    }

    #[repr(C)]
    struct WndClassW {
        style: u32,
        wnd_proc: Option<unsafe extern "system" fn(Hwnd, u32, Wparam, Lparam) -> Lresult>,
        cls_extra: i32,
        wnd_extra: i32,
        instance: Hinstance,
        icon: Hicon,
        cursor: *mut c_void,
        background: *mut c_void,
        menu_name: *const u16,
        class_name: *const u16,
    }

    #[repr(C)]
    struct NotifyIconDataW {
        size: u32,
        hwnd: Hwnd,
        id: u32,
        flags: u32,
        callback_message: u32,
        icon: Hicon,
        tip: [u16; 128],
        state: u32,
        state_mask: u32,
        info: [u16; 256],
        timeout_or_version: u32,
        info_title: [u16; 64],
        info_flags: u32,
        guid_item: [u8; 16],
        balloon_icon: Hicon,
    }

    #[repr(C)]
    struct BitmapInfoHeader {
        size: u32,
        width: i32,
        height: i32,
        planes: u16,
        bit_count: u16,
        compression: u32,
        size_image: u32,
        x_pels_per_meter: i32,
        y_pels_per_meter: i32,
        clr_used: u32,
        clr_important: u32,
    }

    #[repr(C)]
    struct BitmapInfo {
        header: BitmapInfoHeader,
        colors: [u32; 1],
    }

    #[repr(C)]
    struct IconInfo {
        is_icon: i32,
        x_hotspot: u32,
        y_hotspot: u32,
        mask: Hbitmap,
        color: Hbitmap,
    }

    #[repr(C)]
    struct StartupInfoW {
        cb: u32,
        reserved: *mut u16,
        desktop: *mut u16,
        title: *mut u16,
        x: u32,
        y: u32,
        x_size: u32,
        y_size: u32,
        x_count_chars: u32,
        y_count_chars: u32,
        fill_attribute: u32,
        flags: u32,
        show_window: u16,
        reserved2_size: u16,
        reserved2: *mut u8,
        stdin: *mut c_void,
        stdout: *mut c_void,
        stderr: *mut c_void,
    }

    impl Default for StartupInfoW {
        fn default() -> Self {
            Self {
                cb: size_of::<Self>() as u32,
                reserved: null_mut(),
                desktop: null_mut(),
                title: null_mut(),
                x: 0,
                y: 0,
                x_size: 0,
                y_size: 0,
                x_count_chars: 0,
                y_count_chars: 0,
                fill_attribute: 0,
                flags: 0,
                show_window: 0,
                reserved2_size: 0,
                reserved2: null_mut(),
                stdin: null_mut(),
                stdout: null_mut(),
                stderr: null_mut(),
            }
        }
    }

    #[repr(C)]
    struct ProcessInformation {
        process: *mut c_void,
        thread: *mut c_void,
        process_id: u32,
        thread_id: u32,
    }

    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn CreateMutexW(
            attributes: *mut c_void,
            initial_owner: i32,
            name: *const u16,
        ) -> *mut c_void;
        fn GetLastError() -> u32;
        fn GetModuleFileNameW(module: Hinstance, filename: *mut u16, size: u32) -> u32;
        fn CreateProcessW(
            application_name: *const u16,
            command_line: *mut u16,
            process_attributes: *mut c_void,
            thread_attributes: *mut c_void,
            inherit_handles: i32,
            creation_flags: u32,
            environment: *mut c_void,
            current_directory: *const u16,
            startup_info: *mut StartupInfoW,
            process_information: *mut ProcessInformation,
        ) -> i32;
        fn CloseHandle(handle: *mut c_void) -> i32;
        fn CreateEventW(
            attributes: *mut c_void,
            manual_reset: i32,
            initial_state: i32,
            name: *const u16,
        ) -> *mut c_void;
        fn OpenEventW(access: u32, inherit: i32, name: *const u16) -> *mut c_void;
        fn SetEvent(event: *mut c_void) -> i32;
        fn GetModuleHandleW(name: *const u16) -> Hinstance;
        fn CreateWindowExW(
            ex_style: u32,
            class: *const u16,
            name: *const u16,
            style: u32,
            x: i32,
            y: i32,
            width: i32,
            height: i32,
            parent: Hwnd,
            menu: Hmenu,
            instance: Hinstance,
            parameter: *mut c_void,
        ) -> Hwnd;
        fn RegisterClassW(class: *const WndClassW) -> u16;
        fn GetMessageW(message: *mut Msg, window: Hwnd, min: u32, max: u32) -> i32;
        fn TranslateMessage(message: *const Msg) -> i32;
        fn DispatchMessageW(message: *const Msg) -> Lresult;
        fn DefWindowProcW(window: Hwnd, message: u32, w_param: Wparam, l_param: Lparam) -> Lresult;
        fn PostQuitMessage(exit_code: i32);
        fn DestroyWindow(window: Hwnd) -> i32;
        fn SetTimer(window: Hwnd, id: usize, interval_ms: u32, callback: *mut c_void) -> usize;
        fn KillTimer(window: Hwnd, id: usize) -> i32;
        fn CreatePopupMenu() -> Hmenu;
        fn AppendMenuW(menu: Hmenu, flags: u32, id: usize, text: *const u16) -> i32;
        fn TrackPopupMenu(
            menu: Hmenu,
            flags: u32,
            x: i32,
            y: i32,
            reserved: i32,
            window: Hwnd,
            rect: *const c_void,
        ) -> u32;
        fn DestroyMenu(menu: Hmenu) -> i32;
        fn GetCursorPos(point: *mut Point) -> i32;
        fn MessageBoxW(window: Hwnd, text: *const u16, caption: *const u16, kind: u32) -> i32;
        fn CreateIconIndirect(info: *const IconInfo) -> Hicon;
        fn DestroyIcon(icon: Hicon) -> i32;
    }

    #[link(name = "gdi32")]
    unsafe extern "system" {
        fn CreateDIBSection(
            dc: *mut c_void,
            info: *const BitmapInfo,
            usage: u32,
            bits: *mut *mut c_void,
            section: *mut c_void,
            offset: u32,
        ) -> Hbitmap;
        fn CreateCompatibleDC(dc: *mut c_void) -> *mut c_void;
        fn DeleteDC(dc: *mut c_void) -> i32;
        fn CreateBitmap(
            width: i32,
            height: i32,
            planes: u32,
            bits_per_pixel: u32,
            bits: *const c_void,
        ) -> Hbitmap;
        fn DeleteObject(object: *mut c_void) -> i32;
    }

    #[link(name = "shell32")]
    unsafe extern "system" {
        fn Shell_NotifyIconW(action: u32, data: *mut NotifyIconDataW) -> i32;
    }

    #[link(name = "user32")]
    unsafe extern "system" {
        fn GetDC(window: Hwnd) -> *mut c_void;
        fn ReleaseDC(window: Hwnd, dc: *mut c_void) -> i32;
    }

    #[link(name = "advapi32")]
    unsafe extern "system" {
        fn RegOpenKeyExW(
            key: Hkey,
            subkey: *const u16,
            options: u32,
            access: u32,
            result: *mut Hkey,
        ) -> i32;
        fn RegQueryValueExW(
            key: Hkey,
            name: *const u16,
            reserved: *mut u32,
            kind: *mut u32,
            data: *mut u8,
            bytes: *mut u32,
        ) -> i32;
        fn RegSetValueExW(
            key: Hkey,
            name: *const u16,
            reserved: u32,
            kind: u32,
            data: *const u8,
            bytes: u32,
        ) -> i32;
        fn RegDeleteValueW(key: Hkey, name: *const u16) -> i32;
        fn RegCloseKey(key: Hkey) -> i32;
    }

    const fn wide<const N: usize>(value: &str) -> [u16; N] {
        let bytes = value.as_bytes();
        let mut result = [0; N];
        let mut index = 0;
        while index < bytes.len() && index < N - 1 {
            result[index] = bytes[index] as u16;
            index += 1;
        }
        result
    }

    fn to_wide(value: &str) -> Vec<u16> {
        value.encode_utf16().chain(std::iter::once(0)).collect()
    }

    #[derive(Clone, Copy, PartialEq, Eq)]
    enum Mode {
        CpuRam,
        Cpu,
        Ram,
    }

    struct AppState {
        cpu: CpuSampler,
        cpu_pct: f32,
        mem: MemInfo,
        mode: Mode,
        interval_ms: u32,
        icon: Hicon,
        data: NotifyIconDataW,
        hwnd: Hwnd,
        previous_visual: Option<(u32, u32, u8, u8, Mode)>,
        metrics: crate::state::SharedMetrics,
        last_full_mem: Option<Instant>,
    }

    // The notification icon/window handles are owned by the single UI thread.
    // State access is serialized by the mutex while Win32 callbacks run on that thread.
    unsafe impl Send for AppState {}

    static STATE: OnceLock<Mutex<AppState>> = OnceLock::new();

    fn with_state<T>(f: impl FnOnce(&mut AppState) -> T) -> T {
        let state = STATE.get().expect("tray state initialized");
        let mut state = state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        f(&mut state)
    }

    pub fn run() {
        let shutdown_event_name = wide::<32>("Local\\PeakCoreUiExit");
        // Keep the named event alive so the UI can observe tray Exit or CLI shutdown.
        let _dashboard_exit_event =
            unsafe { CreateEventW(null_mut(), 1, 0, shutdown_event_name.as_ptr()) };
        let mutex_name = wide::<24>("Local\\PeakCoreDaemon");
        // SAFETY: valid NUL-terminated name; null security attributes are permitted.
        let mutex = unsafe { CreateMutexW(null_mut(), 0, mutex_name.as_ptr()) };
        // SAFETY: GetLastError has no preconditions.
        if mutex.is_null() {
            return;
        }
        if unsafe { GetLastError() } == ERROR_ALREADY_EXISTS {
            let _ = peakcore_core::ipc_client::request("show_window", None);
            return;
        }

        let state_data = unsafe { zeroed() };
        let metrics = crate::state::new_shared_metrics();
        let _ = STATE.set(Mutex::new(AppState {
            cpu: CpuSampler::default(),
            cpu_pct: 0.0,
            mem: MemInfo::default(),
            mode: Mode::CpuRam,
            interval_ms: 2_000,
            icon: null_mut(),
            data: state_data,
            hwnd: null_mut(),
            previous_visual: None,
            metrics: metrics.clone(),
            last_full_mem: None,
        }));

        let class_name = wide::<16>("PeakCoreTray");
        // SAFETY: null module name asks for the current executable module.
        let instance = unsafe { GetModuleHandleW(null()) };
        let class = WndClassW {
            style: 0,
            wnd_proc: Some(window_proc),
            cls_extra: 0,
            wnd_extra: 0,
            instance,
            icon: null_mut(),
            cursor: null_mut(),
            background: null_mut(),
            menu_name: null(),
            class_name: class_name.as_ptr(),
        };
        // SAFETY: class points to a valid WNDCLASSW for registration.
        if unsafe { RegisterClassW(&class) } == 0 {
            return;
        }

        let mut data: NotifyIconDataW = unsafe { zeroed() };
        data.size = size_of::<NotifyIconDataW>() as u32;
        data.id = IDI_APP as u32;
        data.flags = NIF_MESSAGE | NIF_ICON | NIF_TIP;
        data.callback_message = WM_USER + 1;
        set_tooltip(&mut data, 0.0, 0, 0);
        let hwnd = unsafe {
            CreateWindowExW(
                0,
                class_name.as_ptr(),
                class_name.as_ptr(),
                0,
                0,
                0,
                0,
                0,
                null_mut(),
                null_mut(),
                instance,
                null_mut(),
            )
        };
        if hwnd.is_null() {
            return;
        }
        with_state(|state| {
            state.hwnd = hwnd;
            state.data.hwnd = hwnd;
            state.data.tip = data.tip;
            state.data.flags = NIF_MESSAGE | NIF_ICON | NIF_TIP;
            state.data.callback_message = WM_USER + 1;
        });
        crate::ipc::start_server(metrics, hwnd as usize);
        let icon = with_state(|state| render_icon(state.cpu_pct, 0.0, state.mode));
        with_state(|state| {
            state.icon = icon;
            state.data.icon = icon;
            // SAFETY: data contains the hidden window, icon, and valid tooltip.
            unsafe {
                Shell_NotifyIconW(NIM_ADD, &mut state.data);
            }
            update_sample(state);
            unsafe {
                SetTimer(hwnd, TIMER_ID, state.interval_ms, null_mut());
            }
        });

        let mut message: Msg = unsafe { zeroed() };
        loop {
            // SAFETY: message is writable and -1 indicates an error, 0 indicates WM_QUIT.
            let result = unsafe { GetMessageW(&mut message, null_mut(), 0, 0) };
            if result <= 0 {
                break;
            }
            // SAFETY: message was populated by GetMessageW.
            unsafe {
                TranslateMessage(&message);
                DispatchMessageW(&message);
            }
        }
        with_state(|state| {
            // SAFETY: stop timer, remove shell icon, and release our icon handle.
            unsafe {
                KillTimer(hwnd, TIMER_ID);
                Shell_NotifyIconW(NIM_DELETE, &mut state.data);
                if !state.icon.is_null() {
                    DestroyIcon(state.icon);
                }
            }
        });
        let _ = mutex; // Keep the named mutex handle alive for the process lifetime.
    }

    unsafe extern "system" fn window_proc(
        hwnd: Hwnd,
        message: u32,
        w_param: Wparam,
        l_param: Lparam,
    ) -> Lresult {
        match message {
            value if value == WM_USER + 1 => {
                if l_param == WM_LBUTTONUP {
                    open_dashboard(hwnd);
                } else if l_param == WM_RBUTTONUP || l_param == WM_CONTEXTMENU {
                    show_menu(hwnd);
                }
                0
            }
            WM_TIMER if w_param == TIMER_ID => {
                with_state(update_sample);
                0
            }
            WM_APP_IPC if w_param == 0 => {
                open_dashboard(hwnd);
                0
            }
            WM_APP_IPC if w_param == 1 => {
                signal_dashboard_exit();
                // SAFETY: the tray window owns the shutdown lifecycle.
                unsafe {
                    DestroyWindow(hwnd);
                }
                0
            }
            WM_CLOSE => {
                signal_dashboard_exit();
                // SAFETY: the tray window owns the shutdown lifecycle.
                unsafe {
                    DestroyWindow(hwnd);
                }
                0
            }
            WM_DESTROY => {
                // SAFETY: posts WM_QUIT to the current thread's message queue.
                unsafe {
                    PostQuitMessage(0);
                }
                0
            }
            _ => unsafe { DefWindowProcW(hwnd, message, w_param, l_param) },
        }
    }

    fn update_sample(state: &mut AppState) {
        if let Ok(Some(cpu)) = state.cpu.sample() {
            state.cpu_pct = cpu;
        }
        let now = Instant::now();
        let full_memory_due = state
            .last_full_mem
            .is_none_or(|last| now.duration_since(last) >= std::time::Duration::from_secs(30));
        let memory = if full_memory_due {
            peakcore_sys::mem_info().ok().map(|memory| (memory, true))
        } else {
            None
        };
        let memory = memory.or_else(|| {
            peakcore_sys::basic_mem_info()
                .ok()
                .map(|memory| (memory, false))
        });
        if let Some((mut memory, full_sample)) = memory {
            if !full_sample {
                memory.cached = state.mem.cached;
                memory.commit_used = state.mem.commit_used;
                memory.commit_limit = state.mem.commit_limit;
            }
            state.mem = memory;
            if full_sample {
                state.last_full_mem = Some(now);
            }
        }
        let ts_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        let mem_pct = percentage(state.mem.used, state.mem.total);
        {
            let mut metrics = state
                .metrics
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            metrics.snapshot = SysSnapshot {
                ts_ms,
                cpu_total: state.cpu_pct,
                cpu_cores: Vec::new(),
                mem: state.mem.clone(),
            };
            metrics.history.push(HistoryPoint {
                ts_ms,
                cpu: state.cpu_pct,
                mem_pct,
            });
        }
        let ram_pct = percentage(state.mem.used, state.mem.total);
        let cpu = state.cpu_pct.round().clamp(0.0, 99.0) as u32;
        let ram = ram_pct.round().clamp(0.0, 99.0) as u32;
        let visual = (cpu, ram, level(state.cpu_pct), level(ram_pct), state.mode);
        let old_tooltip = state.data.tip;
        set_tooltip(
            &mut state.data,
            state.cpu_pct,
            state.mem.used,
            state.mem.total,
        );
        if state.previous_visual == Some(visual) {
            if old_tooltip != state.data.tip {
                // Refresh changed tooltip text without repainting an unchanged icon.
                state.data.flags = NIF_TIP;
                unsafe {
                    Shell_NotifyIconW(NIM_MODIFY, &mut state.data);
                }
                state.data.flags = NIF_MESSAGE | NIF_ICON | NIF_TIP;
            }
            return;
        }
        state.previous_visual = Some(visual);
        let icon = render_icon(state.cpu_pct, ram_pct, state.mode);
        if icon.is_null() {
            return;
        }
        // SAFETY: shell notification data is valid, and the previous icon belongs to this process.
        unsafe {
            let old = state.icon;
            state.icon = icon;
            state.data.icon = icon;
            Shell_NotifyIconW(NIM_MODIFY, &mut state.data);
            if !old.is_null() {
                DestroyIcon(old);
            }
        }
    }

    fn percentage(value: u64, total: u64) -> f32 {
        if total == 0 {
            0.0
        } else {
            (value as f64 * 100.0 / total as f64).clamp(0.0, 100.0) as f32
        }
    }

    fn level(value: f32) -> u8 {
        if value < 60.0 {
            0
        } else if value <= 85.0 {
            1
        } else {
            2
        }
    }

    fn set_tooltip(data: &mut NotifyIconDataW, cpu: f32, used: u64, total: u64) {
        let text = if total == 0 {
            format!("PeakCore | CPU {cpu:.0}% | RAM —")
        } else {
            format!(
                "PeakCore | CPU {cpu:.0}% | RAM {:.1}/{:.1} GiB",
                used as f64 / 1_073_741_824.0,
                total as f64 / 1_073_741_824.0
            )
        };
        data.tip = [0; 128];
        for (slot, unit) in data.tip.iter_mut().zip(text.encode_utf16()) {
            *slot = unit;
        }
    }

    fn render_icon(cpu_pct: f32, ram_pct: f32, mode: Mode) -> Hicon {
        const SIZE: i32 = 16;
        let pixels = render_pixels(cpu_pct, ram_pct, mode);

        let info = BitmapInfo {
            header: BitmapInfoHeader {
                size: size_of::<BitmapInfoHeader>() as u32,
                width: SIZE,
                height: -SIZE,
                planes: 1,
                bit_count: 32,
                compression: 0,
                size_image: 0,
                x_pels_per_meter: 0,
                y_pels_per_meter: 0,
                clr_used: 0,
                clr_important: 0,
            },
            colors: [0],
        };
        let dc = unsafe { GetDC(null_mut()) };
        let memory_dc = unsafe { CreateCompatibleDC(dc) };
        let mut bits = null_mut();
        let color_bitmap =
            unsafe { CreateDIBSection(memory_dc, &info, 0, &mut bits, null_mut(), 0) };
        if !color_bitmap.is_null() && !bits.is_null() {
            unsafe {
                std::ptr::copy_nonoverlapping(pixels.as_ptr(), bits.cast::<u8>(), pixels.len());
            }
        }
        let mask = unsafe { CreateBitmap(SIZE, SIZE, 1, 1, null()) };
        let icon_info = IconInfo {
            is_icon: 1,
            x_hotspot: 0,
            y_hotspot: 0,
            mask,
            color: color_bitmap,
        };
        let icon = if !color_bitmap.is_null() && !mask.is_null() {
            unsafe { CreateIconIndirect(&icon_info) }
        } else {
            null_mut()
        };
        unsafe {
            if !mask.is_null() {
                DeleteObject(mask);
            }
            if !color_bitmap.is_null() {
                DeleteObject(color_bitmap);
            }
            if !memory_dc.is_null() {
                DeleteDC(memory_dc);
            }
            if !dc.is_null() {
                ReleaseDC(null_mut(), dc);
            }
        }
        icon
    }

    fn render_pixels(cpu_pct: f32, ram_pct: f32, mode: Mode) -> Vec<u8> {
        const SIZE: i32 = 16;
        let mut pixels = vec![0u8; (SIZE * SIZE * 4) as usize];
        for y in 0..SIZE {
            for x in 0..SIZE {
                let index = ((y * SIZE + x) * 4) as usize;
                pixels[index..index + 4].copy_from_slice(&[24, 26, 31, 245]);
            }
        }
        let cpu = cpu_pct.round().clamp(0.0, 99.0) as u32;
        let ram = ram_pct.round().clamp(0.0, 99.0) as u32;
        if mode != Mode::Ram {
            draw_number(&mut pixels, cpu, 1, color(cpu_pct));
        }
        if mode != Mode::Cpu {
            draw_number(&mut pixels, ram, 9, color(ram_pct));
        }
        pixels
    }

    fn draw_number(pixels: &mut [u8], value: u32, y: i32, color: [u8; 3]) {
        const DIGITS: [[u8; 6]; 10] = [
            [0b0110, 0b1001, 0b1001, 0b1001, 0b1001, 0b0110],
            [0b0010, 0b0110, 0b0010, 0b0010, 0b0010, 0b0111],
            [0b0110, 0b1001, 0b0001, 0b0010, 0b0100, 0b1111],
            [0b1110, 0b0001, 0b0110, 0b0001, 0b1001, 0b0110],
            [0b0010, 0b0110, 0b1010, 0b1111, 0b0010, 0b0010],
            [0b1111, 0b1000, 0b1110, 0b0001, 0b1001, 0b0110],
            [0b0110, 0b1000, 0b1110, 0b1001, 0b1001, 0b0110],
            [0b1111, 0b0001, 0b0010, 0b0010, 0b0100, 0b0100],
            [0b0110, 0b1001, 0b0110, 0b1001, 0b1001, 0b0110],
            [0b0110, 0b1001, 0b1001, 0b0111, 0b0001, 0b0110],
        ];
        let digits = [((value / 10) % 10) as usize, (value % 10) as usize];
        for (position, digit) in digits.into_iter().enumerate() {
            for (row, bitmap_row) in DIGITS[digit].iter().enumerate() {
                for col in 0..4 {
                    if bitmap_row & (1 << (3 - col)) != 0 {
                        let x = 3 + position as i32 * 5 + col;
                        let py = y + row as i32;
                        let index = ((py * 16 + x) * 4) as usize;
                        pixels[index..index + 4]
                            .copy_from_slice(&[color[2], color[1], color[0], 255]);
                    }
                }
            }
        }
    }

    fn color(value: f32) -> [u8; 3] {
        match level(value) {
            0 => [89, 211, 145],
            1 => [255, 201, 87],
            _ => [255, 102, 112],
        }
    }

    fn show_menu(hwnd: Hwnd) {
        let menu = unsafe { CreatePopupMenu() };
        if menu.is_null() {
            return;
        }
        let (mode, interval) = with_state(|state| (state.mode, state.interval_ms));
        append(menu, ID_STATS, "Подробная статистика");
        unsafe {
            AppendMenuW(menu, MF_SEPARATOR, 0, null());
        }
        append_checked(
            menu,
            ID_MODE_CPU_RAM,
            "Режим иконки: CPU + RAM",
            mode == Mode::CpuRam,
        );
        append_checked(
            menu,
            ID_MODE_CPU,
            "Режим иконки: только CPU",
            mode == Mode::Cpu,
        );
        append_checked(
            menu,
            ID_MODE_RAM,
            "Режим иконки: только RAM",
            mode == Mode::Ram,
        );
        append_checked(
            menu,
            ID_INTERVAL_1,
            "Обновление: 1 секунда",
            interval == 1_000,
        );
        append_checked(
            menu,
            ID_INTERVAL_2,
            "Обновление: 2 секунды",
            interval == 2_000,
        );
        append_checked(
            menu,
            ID_INTERVAL_5,
            "Обновление: 5 секунд",
            interval == 5_000,
        );
        append(
            menu,
            ID_AUTOSTART,
            if autostart_enabled() {
                "Отключить запуск с Windows"
            } else {
                "Запускать с Windows"
            },
        );
        append(menu, ID_CONFIG, "Открыть конфиг");
        unsafe {
            AppendMenuW(menu, MF_SEPARATOR, 0, null());
        }
        append(menu, ID_EXIT, "Выход");
        let mut point = Point { x: 0, y: 0 };
        unsafe {
            GetCursorPos(&mut point);
        }
        let selected = unsafe {
            TrackPopupMenu(
                menu,
                TPM_RIGHTBUTTON | TPM_RETURNCMD | TPM_NONOTIFY,
                point.x,
                point.y,
                0,
                hwnd,
                null(),
            )
        };
        unsafe {
            DestroyMenu(menu);
        }
        dispatch_command(hwnd, selected);
    }

    fn append(menu: Hmenu, id: u32, text: &str) {
        append_checked(menu, id, text, false);
    }

    fn append_checked(menu: Hmenu, id: u32, text: &str, checked: bool) {
        let label = to_wide(text);
        let flags = MF_STRING | if checked { MF_CHECKED } else { 0 };
        unsafe {
            AppendMenuW(menu, flags, id as usize, label.as_ptr());
        }
    }

    fn dispatch_command(hwnd: Hwnd, command: u32) {
        match command {
            ID_STATS => open_dashboard(hwnd),
            ID_MODE_CPU_RAM => set_mode(hwnd, Mode::CpuRam),
            ID_MODE_CPU => set_mode(hwnd, Mode::Cpu),
            ID_MODE_RAM => set_mode(hwnd, Mode::Ram),
            ID_INTERVAL_1 => set_interval(hwnd, 1_000),
            ID_INTERVAL_2 => set_interval(hwnd, 2_000),
            ID_INTERVAL_5 => set_interval(hwnd, 5_000),
            ID_AUTOSTART => toggle_autostart(hwnd),
            ID_CONFIG => open_config(hwnd),
            ID_EXIT => unsafe {
                signal_dashboard_exit();
                PostQuitMessage(0);
            },
            _ => {}
        }
    }

    fn signal_dashboard_exit() {
        let name = wide::<32>("Local\\PeakCoreUiExit");
        let event = unsafe { OpenEventW(EVENT_MODIFY_STATE, 0, name.as_ptr()) };
        if !event.is_null() {
            unsafe {
                SetEvent(event);
                CloseHandle(event);
            }
        }
    }

    fn set_mode(hwnd: Hwnd, mode: Mode) {
        with_state(|state| {
            state.mode = mode;
            state.previous_visual = None;
            update_sample(state);
        });
        let _ = hwnd;
    }

    fn set_interval(hwnd: Hwnd, interval: u32) {
        with_state(|state| {
            state.interval_ms = interval;
            // SAFETY: replaces the timer with the requested sampling period.
            unsafe {
                KillTimer(hwnd, TIMER_ID);
                SetTimer(hwnd, TIMER_ID, interval, null_mut());
            }
        });
    }

    fn show_message(hwnd: Hwnd, text: &str, title: &str) {
        let text = to_wide(text);
        let title = to_wide(title);
        unsafe {
            MessageBoxW(hwnd, text.as_ptr(), title.as_ptr(), 0);
        }
    }

    fn open_dashboard(hwnd: Hwnd) {
        if let Err(error) = launch_dashboard() {
            show_message(
                hwnd,
                &format!("Не удалось открыть окно PeakCore:\n{error}"),
                "PeakCore",
            );
        }
    }

    fn launch_dashboard() -> std::io::Result<()> {
        let mut module_path = vec![0u16; 32_768];
        // SAFETY: the buffer is writable and its length matches the supplied capacity.
        let length = unsafe {
            GetModuleFileNameW(
                null_mut(),
                module_path.as_mut_ptr(),
                module_path.len() as u32,
            )
        } as usize;
        if length == 0 || length >= module_path.len() {
            return Err(std::io::Error::last_os_error());
        }
        module_path.truncate(length);
        let daemon_path = std::path::PathBuf::from(String::from_utf16_lossy(&module_path));
        let daemon_directory = daemon_path.parent().ok_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "daemon executable directory is unavailable",
            )
        })?;

        let mut candidates = vec![daemon_directory.join("peakcore-ui.exe")];
        let profile = daemon_directory
            .file_name()
            .and_then(|name| name.to_str())
            .filter(|name| matches!(*name, "debug" | "release"));
        if let Some(profile) = profile.filter(|_| {
            daemon_directory
                .parent()
                .and_then(std::path::Path::file_name)
                .is_some_and(|name| name == "target")
        }) {
            if let Some(workspace_root) =
                daemon_directory.parent().and_then(std::path::Path::parent)
            {
                candidates.push(
                    workspace_root
                        .join("ui")
                        .join("src-tauri")
                        .join("target")
                        .join(profile)
                        .join("peakcore-ui.exe"),
                );
            }
        }

        let dashboard = candidates
            .iter()
            .find(|path| path.is_file())
            .ok_or_else(|| {
                let searched = candidates
                    .iter()
                    .map(|path| path.display().to_string())
                    .collect::<Vec<_>>()
                    .join("\n");
                std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    format!("peakcore-ui.exe was not found. Searched:\n{searched}"),
                )
            })?;
        let application_name = to_wide(&dashboard.to_string_lossy());
        let working_directory = dashboard
            .parent()
            .map(|path| to_wide(&path.to_string_lossy()))
            .unwrap_or_default();
        let mut startup = StartupInfoW::default();
        let mut process = ProcessInformation {
            process: null_mut(),
            thread: null_mut(),
            process_id: 0,
            thread_id: 0,
        };
        // SAFETY: application_name and both output structures remain valid for this call.
        if unsafe {
            CreateProcessW(
                application_name.as_ptr(),
                null_mut(),
                null_mut(),
                null_mut(),
                0,
                0,
                null_mut(),
                working_directory.as_ptr(),
                &mut startup,
                &mut process,
            )
        } == 0
        {
            return Err(std::io::Error::last_os_error());
        }
        // SAFETY: CreateProcessW returned two owned handles which are no longer needed.
        unsafe {
            CloseHandle(process.thread);
            CloseHandle(process.process);
        }
        Ok(())
    }

    fn open_config(hwnd: Hwnd) {
        let Some(app_data) = std::env::var_os("APPDATA") else {
            show_message(hwnd, "Не найдена папка AppData.", "PeakCore");
            return;
        };
        let directory = std::path::PathBuf::from(app_data).join("PeakCore");
        let path = directory.join("config.toml");
        let result = (|| -> std::io::Result<()> {
            std::fs::create_dir_all(&directory)?;
            if !path.exists() {
                std::fs::write(
                    &path,
                    r#"[sampling]
tray_interval_ms = 2000

[tray]
mode = "cpu_ram"
warn_pct = 60
crit_pct = 85
"#,
                )?;
            }
            std::process::Command::new("notepad.exe")
                .arg(&path)
                .spawn()?;
            Ok(())
        })();
        if let Err(error) = result {
            show_message(
                hwnd,
                &format!("Не удалось открыть конфиг: {error}"),
                "PeakCore",
            );
        }
    }

    fn autostart_enabled() -> bool {
        let mut key = null_mut();
        let subkey = wide::<64>("Software\\Microsoft\\Windows\\CurrentVersion\\Run");
        if unsafe {
            RegOpenKeyExW(
                HKEY_CURRENT_USER,
                subkey.as_ptr(),
                0,
                KEY_QUERY_VALUE,
                &mut key,
            )
        } != ERROR_SUCCESS
        {
            return false;
        }
        let name = wide::<16>("PeakCore");
        let mut bytes = 0;
        let result = unsafe {
            RegQueryValueExW(
                key,
                name.as_ptr(),
                null_mut(),
                null_mut(),
                null_mut(),
                &mut bytes,
            )
        };
        unsafe {
            RegCloseKey(key);
        }
        result == ERROR_SUCCESS
    }

    fn toggle_autostart(hwnd: Hwnd) {
        let subkey = wide::<64>("Software\\Microsoft\\Windows\\CurrentVersion\\Run");
        let mut key = null_mut();
        if unsafe {
            RegOpenKeyExW(
                HKEY_CURRENT_USER,
                subkey.as_ptr(),
                0,
                KEY_QUERY_VALUE | KEY_SET_VALUE,
                &mut key,
            )
        } != ERROR_SUCCESS
        {
            show_message(
                hwnd,
                "Could not open the current user's Run registry key.",
                "PeakCore",
            );
            return;
        }
        let name = wide::<16>("PeakCore");
        let result = if autostart_enabled() {
            unsafe { RegDeleteValueW(key, name.as_ptr()) }
        } else {
            let executable = match std::env::current_exe() {
                Ok(path) => path,
                Err(error) => {
                    show_message(
                        hwnd,
                        &format!("Could not locate executable: {error}"),
                        "PeakCore",
                    );
                    unsafe {
                        RegCloseKey(key);
                    }
                    return;
                }
            };
            let value = to_wide(&format!("\"{}\"", executable.display()));
            unsafe {
                RegSetValueExW(
                    key,
                    name.as_ptr(),
                    0,
                    REG_SZ,
                    value.as_ptr().cast(),
                    (value.len() * 2) as u32,
                )
            }
        };
        unsafe {
            RegCloseKey(key);
        }
        if result != ERROR_SUCCESS {
            show_message(hwnd, "Could not update the startup setting.", "PeakCore");
        }
    }

    #[cfg(test)]
    mod icon_snapshot_tests {
        use super::*;

        #[test]
        fn cpu_ram_icon_pixels_match_snapshot() {
            let pixels = render_pixels(12.0, 34.0, Mode::CpuRam);
            let hash = pixels
                .iter()
                .fold(14_695_981_039_346_656_037u64, |hash, byte| {
                    (hash ^ u64::from(*byte)).wrapping_mul(1_099_511_628_211)
                });
            assert_eq!(hash, 4_621_873_223_345_363_145);
        }
    }
}

#[cfg(windows)]
pub fn run() {
    platform::run();
}

#[cfg(not(windows))]
pub fn run() {
    eprintln!("PeakCore tray mode is supported on Windows only.");
    std::process::exit(1);
}
