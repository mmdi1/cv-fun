//! Quick-paste popup: show a compact history list at the cursor and paste on select.
//!
//! Critical path for "paste into previous app":
//! 1. Remember the frontmost app **before** FunCV steals focus
//! 2. Write clipboard + hide popup
//! 3. Reactivate that app, wait for focus, then simulate Cmd/Ctrl+V

use crate::append_copy::simulate_system_paste;
use crate::commands::AppState;
use crate::history::{HistoryItem, ItemKind};
use arboard::{Clipboard, ImageData};
use std::borrow::Cow;
use std::sync::atomic::{AtomicI32, AtomicUsize, Ordering};
use std::thread;
use std::time::Duration;
use tauri::{
    AppHandle, Emitter, LogicalPosition, LogicalSize, Manager, WebviewUrl, WebviewWindowBuilder,
};

pub const PASTE_WINDOW_LABEL: &str = "paste";

const POPUP_WIDTH: f64 = 300.0;
const POPUP_ROW_H: f64 = 34.0;
const POPUP_PAD: f64 = 12.0;
/// Visible rows ≈ 12 items + header + "更多" (list scrolls for more).
const POPUP_VISIBLE_ROWS: f64 = 12.0;
const PASTE_LIST_LIMIT: usize = 50;

/// PID of the app that was frontmost when the paste popup opened (0 = unknown).
static PREV_APP_PID: AtomicI32 = AtomicI32::new(0);

/// Windows HWND of the previous foreground window (0 = unknown). Stored as usize.
#[cfg(target_os = "windows")]
static PREV_HWND: AtomicUsize = AtomicUsize::new(0);

#[cfg(not(target_os = "windows"))]
#[allow(dead_code)]
static PREV_HWND: AtomicUsize = AtomicUsize::new(0);

/// Capture the currently frontmost third-party app (call on hotkey, before show).
pub fn capture_previous_app() {
    #[cfg(target_os = "macos")]
    {
        use objc2_app_kit::NSWorkspace;
        let our_pid = std::process::id() as i32;
        if let Some(app) = NSWorkspace::sharedWorkspace().frontmostApplication() {
            let pid = app.processIdentifier();
            if pid != 0 && pid != our_pid {
                PREV_APP_PID.store(pid, Ordering::SeqCst);
            }
        }
    }
    #[cfg(target_os = "windows")]
    {
        #[link(name = "user32")]
        extern "system" {
            fn GetForegroundWindow() -> isize;
            fn GetWindowThreadProcessId(hwnd: isize, lpdw_process_id: *mut u32) -> u32;
        }
        let hwnd = unsafe { GetForegroundWindow() };
        if hwnd != 0 {
            let mut pid: u32 = 0;
            unsafe {
                GetWindowThreadProcessId(hwnd, &mut pid);
            }
            let our = std::process::id();
            if pid != 0 && pid != our {
                PREV_HWND.store(hwnd as usize, Ordering::SeqCst);
                PREV_APP_PID.store(pid as i32, Ordering::SeqCst);
            }
        }
    }
}

/// Restore the app that was frontmost before the paste popup (best-effort).
fn restore_previous_app() -> bool {
    #[cfg(target_os = "macos")]
    {
        use objc2::MainThreadMarker;
        use objc2_app_kit::{NSApplication, NSApplicationActivationOptions, NSRunningApplication};

        let pid = PREV_APP_PID.load(Ordering::SeqCst);
        if pid <= 0 {
            return false;
        }
        let our_pid = std::process::id() as i32;
        if pid == our_pid {
            return false;
        }

        // Deactivate FunCV so the previous app can take key window status.
        let mtm =
            MainThreadMarker::new().unwrap_or_else(|| unsafe { MainThreadMarker::new_unchecked() });
        let ns_app = NSApplication::sharedApplication(mtm);
        ns_app.deactivate();

        if let Some(app) = NSRunningApplication::runningApplicationWithProcessIdentifier(pid) {
            // Prefer ActivateAllWindows; IgnoringOtherApps is deprecated (no-op on macOS 14+).
            #[allow(deprecated)]
            let opts = NSApplicationActivationOptions::ActivateAllWindows
                | NSApplicationActivationOptions::ActivateIgnoringOtherApps;
            let ok = app.activateWithOptions(opts);
            return ok;
        }
        false
    }
    #[cfg(target_os = "windows")]
    {
        windows_force_foreground(PREV_HWND.load(Ordering::SeqCst) as isize)
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        false
    }
}

/// Windows: reliably bring a previous HWND to the foreground so Ctrl+V lands there.
/// Plain `SetForegroundWindow` often fails without AttachThreadInput.
#[cfg(target_os = "windows")]
fn windows_force_foreground(hwnd: isize) -> bool {
    if hwnd == 0 {
        return false;
    }
    #[link(name = "user32")]
    extern "system" {
        fn IsWindow(hwnd: isize) -> i32;
        fn IsIconic(hwnd: isize) -> i32;
        fn ShowWindow(hwnd: isize, n_cmd_show: i32) -> i32;
        fn GetForegroundWindow() -> isize;
        fn GetWindowThreadProcessId(hwnd: isize, lpdw_process_id: *mut u32) -> u32;
        fn GetCurrentThreadId() -> u32;
        fn AttachThreadInput(id_attach: u32, id_attach_to: u32, f_attach: i32) -> i32;
        fn BringWindowToTop(hwnd: isize) -> i32;
        fn SetForegroundWindow(hwnd: isize) -> i32;
        fn SetFocus(hwnd: isize) -> isize;
        fn keybd_event(b_vk: u8, b_scan: u8, dw_flags: u32, dw_extra_info: usize);
    }
    const SW_RESTORE: i32 = 9;
    const SW_SHOW: i32 = 5;
    const KEYEVENTF_KEYUP: u32 = 0x0002;
    const VK_MENU: u8 = 0x12; // Alt — brief pulse helps Win32 allow foreground switch

    unsafe {
        if IsWindow(hwnd) == 0 {
            return false;
        }
        if IsIconic(hwnd) != 0 {
            ShowWindow(hwnd, SW_RESTORE);
        } else {
            ShowWindow(hwnd, SW_SHOW);
        }

        let fg = GetForegroundWindow();
        let mut fg_pid: u32 = 0;
        let mut target_pid: u32 = 0;
        let fg_tid = GetWindowThreadProcessId(fg, &mut fg_pid);
        let target_tid = GetWindowThreadProcessId(hwnd, &mut target_pid);
        let cur_tid = GetCurrentThreadId();

        if fg_tid != 0 && fg_tid != cur_tid {
            AttachThreadInput(cur_tid, fg_tid, 1);
        }
        if target_tid != 0 && target_tid != cur_tid {
            AttachThreadInput(cur_tid, target_tid, 1);
        }

        // Alt key pulse — common workaround for SetForegroundWindow restrictions.
        keybd_event(VK_MENU, 0, 0, 0);
        keybd_event(VK_MENU, 0, KEYEVENTF_KEYUP, 0);

        BringWindowToTop(hwnd);
        let ok = SetForegroundWindow(hwnd) != 0;
        SetFocus(hwnd);

        if fg_tid != 0 && fg_tid != cur_tid {
            AttachThreadInput(cur_tid, fg_tid, 0);
        }
        if target_tid != 0 && target_tid != cur_tid {
            AttachThreadInput(cur_tid, target_tid, 0);
        }
        ok
    }
}

fn popup_height() -> f64 {
    // header ~30 + 12 rows + "更多" bar ~36 + padding
    POPUP_PAD * 2.0 + 30.0 + POPUP_VISIBLE_ROWS * POPUP_ROW_H + 36.0
}

/// Ensure the paste webview exists (created once, reused).
pub fn ensure_paste_window(app: &AppHandle) -> Result<tauri::WebviewWindow, String> {
    if let Some(w) = app.get_webview_window(PASTE_WINDOW_LABEL) {
        return Ok(w);
    }

    // Hash route so frontend always mounts PastePopup even if label is late.
    let win = WebviewWindowBuilder::new(
        app,
        PASTE_WINDOW_LABEL,
        WebviewUrl::App("index.html#paste".into()),
    )
    .title("快速粘贴")
    .inner_size(POPUP_WIDTH, popup_height())
    .min_inner_size(POPUP_WIDTH, popup_height())
    .max_inner_size(POPUP_WIDTH, popup_height())
    .decorations(false)
    .resizable(false)
    .maximizable(false)
    .minimizable(false)
    .closable(false)
    .always_on_top(true)
    .skip_taskbar(true)
    .visible(false)
    .focused(false)
    .transparent(true)
    .shadow(true)
    .build()
    .map_err(|e| format!("创建粘贴弹窗失败: {e}"))?;

    let handle = app.clone();
    win.on_window_event(move |event| {
        if let tauri::WindowEvent::Focused(false) = event {
            // Delay: ignore brief focus blips; re-check before hide (click-outside).
            let handle = handle.clone();
            thread::spawn(move || {
                thread::sleep(Duration::from_millis(140));
                if let Some(w) = handle.get_webview_window(PASTE_WINDOW_LABEL) {
                    if w.is_visible().unwrap_or(false) && !w.is_focused().unwrap_or(false) {
                        let _ = w.hide();
                    }
                }
            });
        }
    });

    Ok(win)
}

/// Current mouse position in logical coordinates (top-left origin for Tauri).
fn cursor_logical_position(app: &AppHandle) -> Option<(f64, f64)> {
    #[cfg(target_os = "macos")]
    {
        use objc2::MainThreadMarker;
        use objc2_app_kit::{NSEvent, NSScreen};
        let _ = app;
        let point = NSEvent::mouseLocation();
        let mtm =
            MainThreadMarker::new().unwrap_or_else(|| unsafe { MainThreadMarker::new_unchecked() });
        // Cocoa: origin bottom-left of primary screen. Convert to top-left.
        let screen_h = NSScreen::mainScreen(mtm)
            .map(|s| s.frame().size.height)
            .unwrap_or(900.0);
        let x = point.x;
        let y = screen_h - point.y;
        return Some((x, y));
    }
    #[cfg(target_os = "windows")]
    {
        let _ = app;
        let (x, y, _scale) = windows_cursor_and_scale()?;
        return Some((x, y));
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        let _ = app;
        None
    }
}

/// Windows: cursor in logical coords + scale of the monitor under the cursor.
#[cfg(target_os = "windows")]
fn windows_cursor_and_scale() -> Option<(f64, f64, f64)> {
    #[repr(C)]
    #[derive(Clone, Copy)]
    struct Point {
        x: i32,
        y: i32,
    }
    #[link(name = "user32")]
    extern "system" {
        fn GetCursorPos(lp_point: *mut Point) -> i32;
        fn MonitorFromPoint(pt: Point, dw_flags: u32) -> isize;
    }
    const MONITOR_DEFAULTTONEAREST: u32 = 2;
    let mut p = Point { x: 0, y: 0 };
    if unsafe { GetCursorPos(&mut p) } == 0 {
        return None;
    }
    let hmon = unsafe { MonitorFromPoint(p, MONITOR_DEFAULTTONEAREST) };
    let scale = windows_monitor_scale(hmon);
    Some((p.x as f64 / scale, p.y as f64 / scale, scale))
}

#[cfg(target_os = "windows")]
fn windows_monitor_scale(hmon: isize) -> f64 {
    // GetDpiForMonitor (shcore) — Windows 8.1+
    #[link(name = "shcore")]
    extern "system" {
        fn GetDpiForMonitor(
            hmonitor: isize,
            dpi_type: u32,
            dpi_x: *mut u32,
            dpi_y: *mut u32,
        ) -> i32;
    }
    const MDT_EFFECTIVE_DPI: u32 = 0;
    let mut dx: u32 = 96;
    let mut dy: u32 = 96;
    let hr = unsafe { GetDpiForMonitor(hmon, MDT_EFFECTIVE_DPI, &mut dx, &mut dy) };
    if hr == 0 && dx > 0 {
        return dx as f64 / 96.0;
    }
    1.0
}

/// Screen work-area bounds in top-left logical coordinates: (left, top, right, bottom).
/// On Windows this is the **monitor under the cursor** (taskbar excluded via rcWork).
fn work_area_bounds(app: &AppHandle) -> (f64, f64, f64, f64) {
    #[cfg(target_os = "macos")]
    {
        use objc2::MainThreadMarker;
        use objc2_app_kit::NSScreen;
        let _ = app;
        let mtm =
            MainThreadMarker::new().unwrap_or_else(|| unsafe { MainThreadMarker::new_unchecked() });
        if let Some(screen) = NSScreen::mainScreen(mtm) {
            let frame = screen.frame();
            let vis = screen.visibleFrame();
            // Cocoa bottom-left → top-left
            let screen_h = frame.size.height;
            let left = vis.origin.x;
            let top = screen_h - (vis.origin.y + vis.size.height);
            let right = vis.origin.x + vis.size.width;
            let bottom = top + vis.size.height;
            return (left, top, right, bottom);
        }
        return (0.0, 0.0, 1440.0, 900.0);
    }
    #[cfg(target_os = "windows")]
    {
        let _ = app;
        return windows_work_area_at_cursor().unwrap_or((0.0, 0.0, 1920.0, 1080.0));
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        let _ = app;
        (0.0, 0.0, 1920.0, 1080.0)
    }
}

/// Windows work area of the monitor containing the cursor (logical coords).
#[cfg(target_os = "windows")]
fn windows_work_area_at_cursor() -> Option<(f64, f64, f64, f64)> {
    #[repr(C)]
    #[derive(Clone, Copy)]
    struct Point {
        x: i32,
        y: i32,
    }
    #[repr(C)]
    #[derive(Clone, Copy)]
    struct Rect {
        left: i32,
        top: i32,
        right: i32,
        bottom: i32,
    }
    #[repr(C)]
    struct MonitorInfo {
        cb_size: u32,
        rc_monitor: Rect,
        rc_work: Rect,
        dw_flags: u32,
    }
    #[link(name = "user32")]
    extern "system" {
        fn GetCursorPos(lp_point: *mut Point) -> i32;
        fn MonitorFromPoint(pt: Point, dw_flags: u32) -> isize;
        fn GetMonitorInfoW(h_monitor: isize, lpmi: *mut MonitorInfo) -> i32;
    }
    const MONITOR_DEFAULTTONEAREST: u32 = 2;

    let mut p = Point { x: 0, y: 0 };
    if unsafe { GetCursorPos(&mut p) } == 0 {
        return None;
    }
    let hmon = unsafe { MonitorFromPoint(p, MONITOR_DEFAULTTONEAREST) };
    if hmon == 0 {
        return None;
    }
    let mut mi = MonitorInfo {
        cb_size: std::mem::size_of::<MonitorInfo>() as u32,
        rc_monitor: Rect {
            left: 0,
            top: 0,
            right: 0,
            bottom: 0,
        },
        rc_work: Rect {
            left: 0,
            top: 0,
            right: 0,
            bottom: 0,
        },
        dw_flags: 0,
    };
    if unsafe { GetMonitorInfoW(hmon, &mut mi) } == 0 {
        return None;
    }
    let scale = windows_monitor_scale(hmon);
    let w = &mi.rc_work;
    Some((
        w.left as f64 / scale,
        w.top as f64 / scale,
        w.right as f64 / scale,
        w.bottom as f64 / scale,
    ))
}

/// Position popup at the caret/mouse:
/// - Prefer **below**: first row top == cursor Y when it fits
/// - If the panel would overflow the desktop bottom, **clamp to bottom**
///   (bottom edge flush with work area) — do **not** flip above just because
///   full height doesn't fit under the cursor
/// - Only place **above** the cursor when usable space below is tiny
/// - Horizontally: left edge near cursor; flip left if overflowing the right edge
fn position_near_cursor(app: &AppHandle, window: &tauri::WebviewWindow) {
    let (cx, cy) = cursor_logical_position(app).unwrap_or((120.0, 120.0));
    let w = POPUP_WIDTH;
    let h = popup_height();
    let margin = 4.0;
    // If less than ~1.5 rows below cursor, treat as "no room" → open upward.
    let min_usable_below = POPUP_ROW_H * 1.5;
    let (left, top, right, bottom) = work_area_bounds(app);
    let bottom_limit = bottom - margin;
    let top_limit = top + margin;

    // Horizontal: start at cursor X; if overflowing right, shift left of cursor.
    let mut x = cx;
    if x + w > right - margin {
        x = (cx - w).max(left + margin);
    }
    x = x.clamp(left + margin, (right - w - margin).max(left + margin));

    let space_below = bottom_limit - cy;
    let y = if space_below >= min_usable_below {
        // Prefer below: first row flush with cursor.
        let mut y = cy;
        // Overflow desktop bottom → 顶满底部 (shift up just enough).
        if y + h > bottom_limit {
            y = bottom_limit - h;
        }
        // Keep within work area top.
        y.max(top_limit)
    } else {
        // Almost no space below → open above the cursor.
        let mut y = cy - h;
        if y < top_limit {
            y = top_limit;
        }
        // If still overflows bottom (very short screen), clamp.
        if y + h > bottom_limit {
            y = (bottom_limit - h).max(top_limit);
        }
        y
    };

    let y = y.clamp(top_limit, (bottom_limit - h).max(top_limit));

    let _ = window.set_position(tauri::Position::Logical(LogicalPosition::new(x, y)));
}

/// Toggle / show paste popup at mouse cursor.
/// Important: do **not** raise the main window — only the compact paste panel.
pub fn show_paste_popup(app: &AppHandle) {
    // Capture *before* we steal focus (hotkey thread + main-thread again).
    capture_previous_app();

    let app = app.clone();
    let _ = app.clone().run_on_main_thread(move || {
        // One more capture on main thread in case hotkey path raced.
        capture_previous_app();

        let Ok(window) = ensure_paste_window(&app) else {
            return;
        };

        // If already visible and focused → hide (toggle)
        let visible = window.is_visible().unwrap_or(false);
        let focused = window.is_focused().unwrap_or(false);
        if visible && focused {
            let _ = window.hide();
            return;
        }

        // Keep main UI out of the way: if it is not the focused user surface,
        // hide it so app activation does not surface the full FunCV window.
        if let Some(main) = app.get_webview_window("main") {
            let main_focused = main.is_focused().unwrap_or(false);
            let main_visible = main.is_visible().unwrap_or(false);
            if main_visible && !main_focused {
                let _ = main.hide();
            }
        }

        // Fixed compact size (scroll inside for 50 items).
        let _ = window.set_size(tauri::Size::Logical(LogicalSize::new(
            POPUP_WIDTH,
            popup_height(),
        )));
        position_near_cursor(&app, &window);

        #[cfg(target_os = "macos")]
        {
            // Order only the paste panel front — avoid activateIgnoringOtherApps
            // which can unhide / raise every app window including main.
            if let Ok(ns_ptr) = window.ns_window() {
                if !ns_ptr.is_null() {
                    use objc2_app_kit::NSWindow;
                    let ns_window = unsafe { &*(ns_ptr as *const NSWindow) };
                    let _ = window.show();
                    ns_window.orderFrontRegardless();
                    ns_window.makeKeyAndOrderFront(None);
                } else {
                    let _ = window.show();
                }
            } else {
                let _ = window.show();
            }
            let _ = window.set_focus();
        }
        #[cfg(target_os = "windows")]
        {
            let _ = window.set_always_on_top(true);
            let _ = window.show();
            let _ = window.set_focus();
            // Ensure Win32 HWND can take focus for keyboard shortcuts (Ctrl+1..9).
            if let Ok(hwnd) = window.hwnd() {
                let hwnd = hwnd.0 as isize;
                let _ = windows_force_foreground(hwnd);
            }
        }
        #[cfg(not(any(target_os = "macos", target_os = "windows")))]
        {
            let _ = window.show();
            let _ = window.set_focus();
        }

        let _ = window.emit("paste-popup-show", ());
    });
}

pub fn hide_paste_popup(app: &AppHandle) {
    if let Some(window) = app.get_webview_window(PASTE_WINDOW_LABEL) {
        let _ = window.hide();
    }
}

/// Copy history item to clipboard, hide popup, restore previous app, simulate paste.
pub fn paste_history_item(app: &AppHandle, state: &AppState, id: &str) -> Result<(), String> {
    // 1) Write clipboard from history
    {
        let store = state.store.lock();
        let item = store.get(id).map_err(|e| e.to_string())?;
        let mut clipboard = Clipboard::new().map_err(|e| format!("剪贴板不可用: {e}"))?;

        match item.kind {
            ItemKind::Text => {
                let text = item.text.unwrap_or_default();
                clipboard
                    .set_text(text)
                    .map_err(|e| format!("写入剪贴板失败: {e}"))?;
            }
            ItemKind::Image => {
                let rel = item
                    .image_path
                    .ok_or_else(|| "图片路径缺失".to_string())?;
                let abs = store.root().join(rel);
                let img = image::open(&abs)
                    .map_err(|e| format!("读取图片失败: {e}"))?
                    .to_rgba8();
                let (w, h) = img.dimensions();
                let data = ImageData {
                    width: w as usize,
                    height: h as usize,
                    bytes: Cow::Owned(img.into_raw()),
                };
                clipboard
                    .set_image(data)
                    .map_err(|e| format!("写入图片失败: {e}"))?;
            }
        }
        let _ = store.mark_used(id);
    }

    // 2) Hide FunCV windows so key focus can leave this process
    hide_paste_popup(app);
    if let Some(main) = app.get_webview_window("main") {
        let _ = main.hide();
    }

    // 3) Restore previous app on main thread, then inject Cmd/Ctrl+V
    let handle = app.clone();
    thread::Builder::new()
        .name("quick-paste".into())
        .spawn(move || {
            // Brief yield so hide() is processed.
            thread::sleep(Duration::from_millis(40));

            // Focus restore must run on main thread for AppKit.
            let (tx, rx) = std::sync::mpsc::channel();
            let _ = handle.run_on_main_thread(move || {
                let ok = restore_previous_app();
                let _ = tx.send(ok);
            });
            let restored = rx.recv_timeout(Duration::from_millis(800)).unwrap_or(false);

            // Give the target app time to become key and restore caret.
            thread::sleep(Duration::from_millis(if restored { 180 } else { 280 }));

            match simulate_system_paste() {
                Ok(()) => {
                    let _ = handle.emit(
                        "paste-popup-result",
                        serde_json::json!({ "ok": true, "message": "已粘贴" }),
                    );
                }
                Err(err) => {
                    let hint = if cfg!(target_os = "macos") {
                        "macOS 请在「辅助功能」中允许 FunCV"
                    } else if cfg!(target_os = "windows") {
                        "内容已在剪贴板，可手动 Ctrl+V；若持续失败请检查前台权限"
                    } else {
                        "内容已在剪贴板"
                    };
                    let _ = handle.emit(
                        "paste-popup-result",
                        serde_json::json!({
                            "ok": false,
                            "message": format!("{err}（{hint}）")
                        }),
                    );
                }
            }
        })
        .ok();

    Ok(())
}

/// Lightweight list for the paste popup (max 50 items). Optional search query.
pub fn list_for_paste(state: &AppState, query: &str) -> Result<Vec<HistoryItem>, String> {
    let items = state
        .store
        .lock()
        .list(query)
        .map_err(|e| e.to_string())?;
    Ok(items.into_iter().take(PASTE_LIST_LIMIT).collect())
}

/// Keep paste window at fixed compact height (scroll inside).
pub fn resize_paste_window(app: &AppHandle, _item_count: usize) {
    if let Some(window) = app.get_webview_window(PASTE_WINDOW_LABEL) {
        let _ = window.set_size(tauri::Size::Logical(LogicalSize::new(
            POPUP_WIDTH,
            popup_height(),
        )));
    }
}
