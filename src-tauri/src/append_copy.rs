//! Append-copy: hotkey copies current selection and appends to existing clipboard text.
//! Separator: two spaces (`  `). Consecutive append of the same segment is ignored.
//!
//! Important: when the hotkey itself includes `C` (e.g. Cmd+Shift+C), simulating Cmd+C
//! while Shift is still held re-fires the same global shortcut. We therefore:
//! - guard against re-entry
//! - wait for modifier keys to be released
//! - avoid synthesizing events that re-trigger the registered hotkey when possible

use arboard::Clipboard;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::Duration;

/// Prevent re-entrant hotkey paths that synthesize Cmd+C.
static COPY_HOTKEY_IN_PROGRESS: AtomicBool = AtomicBool::new(false);

fn try_begin_copy_hotkey() -> Result<(), String> {
    if COPY_HOTKEY_IN_PROGRESS
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        return Err("复制快捷键处理中".into());
    }
    Ok(())
}

fn end_copy_hotkey() {
    COPY_HOTKEY_IN_PROGRESS.store(false, Ordering::SeqCst);
}

/// Merge `addition` onto `base` with `"  "` separator.
/// If `addition` equals the last segment (split by `"  "`), leave `base` unchanged.
pub fn merge_append_text(base: &str, addition: &str) -> String {
    let addition = addition.trim_end_matches('\0').trim();
    if addition.is_empty() {
        return base.to_string();
    }
    if base.is_empty() {
        return addition.to_string();
    }
    let last = base.rsplit("  ").next().unwrap_or(base);
    if last == addition {
        return base.to_string();
    }
    format!("{base}  {addition}")
}

/// Read current text clipboard; None if empty / not text.
fn clipboard_text(cb: &mut Clipboard) -> Option<String> {
    match cb.get_text() {
        Ok(t) if !t.is_empty() => Some(t),
        _ => None,
    }
}

/// Wait until common modifiers are up so synthetic Cmd/Ctrl chords do not re-fire hotkeys.
fn wait_modifiers_released(max_ms: u64) {
    #[cfg(target_os = "macos")]
    {
        // CGEventSourceFlagsState — poll until Command/Shift/Option/Control are up
        // or timeout (user still holding keys).
        let start = std::time::Instant::now();
        while start.elapsed() < Duration::from_millis(max_ms) {
            if !macos_any_modifier_down() {
                // brief settle
                thread::sleep(Duration::from_millis(30));
                if !macos_any_modifier_down() {
                    return;
                }
            }
            thread::sleep(Duration::from_millis(16));
        }
        // Extra buffer even if still held — reduce race
        thread::sleep(Duration::from_millis(40));
        return;
    }
    #[cfg(target_os = "windows")]
    {
        let start = std::time::Instant::now();
        while start.elapsed() < Duration::from_millis(max_ms) {
            if !windows_any_modifier_down() {
                thread::sleep(Duration::from_millis(30));
                if !windows_any_modifier_down() {
                    return;
                }
            }
            thread::sleep(Duration::from_millis(16));
        }
        thread::sleep(Duration::from_millis(40));
        return;
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        thread::sleep(Duration::from_millis(max_ms.min(180)));
    }
}

#[cfg(target_os = "macos")]
fn macos_any_modifier_down() -> bool {
    // kCGEventSourceStateCombinedSessionState = 0
    // NX_SHIFTMASK etc via CGEventSourceFlagsState
    #[link(name = "CoreGraphics", kind = "framework")]
    extern "C" {
        fn CGEventSourceFlagsState(state_id: u32) -> u64;
    }
    // From CGEventFlags / Carbon: Command=0x100000, Shift=0x20000, Option=0x80000, Control=0x40000
    const CMD: u64 = 0x100000;
    const SHIFT: u64 = 0x20000;
    const OPTION: u64 = 0x80000;
    const CONTROL: u64 = 0x40000;
    let flags = unsafe { CGEventSourceFlagsState(0) };
    (flags & (CMD | SHIFT | OPTION | CONTROL)) != 0
}

#[cfg(target_os = "windows")]
fn windows_any_modifier_down() -> bool {
    #[link(name = "user32")]
    extern "system" {
        fn GetAsyncKeyState(v_key: i32) -> i16;
    }
    // VK_SHIFT=0x10, VK_CONTROL=0x11, VK_MENU(Alt)=0x12, VK_LWIN=0x5B, VK_RWIN=0x5C
    const KEYS: [i32; 5] = [0x10, 0x11, 0x12, 0x5B, 0x5C];
    unsafe {
        KEYS.iter()
            .any(|&k| (GetAsyncKeyState(k) as u16 & 0x8000) != 0)
    }
}

/// Simulate system Copy via CGEvent on macOS (main-thread preferred), enigo/SendInput elsewhere.
fn simulate_system_copy() -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        return macos_post_cmd_key(0x08); // kVK_ANSI_C
    }
    #[cfg(target_os = "windows")]
    {
        return windows_ctrl_key('c');
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        simulate_mod_key_click('c')
    }
}

/// Simulate system Paste (Cmd+V / Ctrl+V) into the **currently focused** app.
/// Caller must restore the target app's focus before invoking this.
pub fn simulate_system_paste() -> Result<(), String> {
    // Wait for user modifiers (e.g. still holding ⌘/Ctrl after digit shortcut) to settle.
    wait_modifiers_released(450);
    thread::sleep(Duration::from_millis(30));
    #[cfg(target_os = "macos")]
    {
        // Full Command-down → V → Command-up sequence is more reliable than
        // flagging only the V event when injecting into another app.
        if macos_post_cmd_chord(0x09).is_ok() {
            return Ok(());
        }
        // Fallback: enigo Meta+V
        return macos_enigo_paste();
    }
    #[cfg(target_os = "windows")]
    {
        // Prefer SendInput chord; fall back to enigo.
        if windows_ctrl_key('v').is_ok() {
            return Ok(());
        }
        return simulate_mod_key_click('v');
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        simulate_mod_key_click('v')
    }
}

#[cfg(target_os = "macos")]
fn macos_enigo_paste() -> Result<(), String> {
    use enigo::{Direction, Enigo, Key, Keyboard, Settings};
    let mut enigo =
        Enigo::new(&Settings::default()).map_err(|e| format!("键盘模拟初始化失败: {e}"))?;
    enigo
        .key(Key::Meta, Direction::Press)
        .map_err(|e| format!("按键失败: {e}"))?;
    thread::sleep(Duration::from_millis(16));
    enigo
        .key(Key::Unicode('v'), Direction::Click)
        .map_err(|e| format!("按键失败: {e}"))?;
    thread::sleep(Duration::from_millis(16));
    enigo
        .key(Key::Meta, Direction::Release)
        .map_err(|e| format!("按键失败: {e}"))?;
    Ok(())
}

/// Windows: Ctrl + key via keybd_event (Ctrl down → key → Ctrl up).
#[cfg(target_os = "windows")]
fn windows_ctrl_key(ch: char) -> Result<(), String> {
    #[link(name = "user32")]
    extern "system" {
        fn keybd_event(b_vk: u8, b_scan: u8, dw_flags: u32, dw_extra_info: usize);
    }

    const KEYEVENTF_KEYUP: u32 = 0x0002;
    const VK_CONTROL: u8 = 0x11;
    let vk = match ch.to_ascii_lowercase() {
        'c' => 0x43u8, // VK_C
        'v' => 0x56u8, // VK_V
        other => {
            return Err(format!("不支持的按键: {other}"));
        }
    };

    unsafe {
        keybd_event(VK_CONTROL, 0, 0, 0);
        thread::sleep(Duration::from_millis(12));
        keybd_event(vk, 0, 0, 0);
        thread::sleep(Duration::from_millis(12));
        keybd_event(vk, 0, KEYEVENTF_KEYUP, 0);
        thread::sleep(Duration::from_millis(12));
        keybd_event(VK_CONTROL, 0, KEYEVENTF_KEYUP, 0);
    }
    Ok(())
}

#[cfg(not(target_os = "macos"))]
fn simulate_mod_key_click(ch: char) -> Result<(), String> {
    use enigo::{Direction, Enigo, Key, Keyboard, Settings};
    let mut enigo =
        Enigo::new(&Settings::default()).map_err(|e| format!("键盘模拟初始化失败: {e}"))?;
    enigo
        .key(Key::Control, Direction::Press)
        .map_err(|e| format!("按键失败: {e}"))?;
    thread::sleep(Duration::from_millis(12));
    enigo
        .key(Key::Unicode(ch), Direction::Click)
        .map_err(|e| format!("按键失败: {e}"))?;
    thread::sleep(Duration::from_millis(12));
    enigo
        .key(Key::Control, Direction::Release)
        .map_err(|e| format!("按键失败: {e}"))?;
    Ok(())
}

#[cfg(target_os = "macos")]
fn macos_post_cmd_key(virtual_key: u16) -> Result<(), String> {
    // Used by copy path: V/C with Command flag only.
    macos_post_cmd_chord(virtual_key)
}

/// Post Command + `virtual_key` as a full chord:
/// ⌘ down → key down → key up → ⌘ up (with Command flag on key events).
#[cfg(target_os = "macos")]
fn macos_post_cmd_chord(virtual_key: u16) -> Result<(), String> {
    #[link(name = "CoreGraphics", kind = "framework")]
    extern "C" {
        fn CGEventSourceCreate(state_id: i32) -> *mut std::ffi::c_void;
        fn CGEventCreateKeyboardEvent(
            source: *mut std::ffi::c_void,
            virtual_key: u16,
            key_down: bool,
        ) -> *mut std::ffi::c_void;
        fn CGEventSetFlags(event: *mut std::ffi::c_void, flags: u64);
        fn CGEventPost(tap: u32, event: *mut std::ffi::c_void);
        fn CFRelease(cf: *mut std::ffi::c_void);
    }

    // kCGEventSourceStateHIDSystemState = 1
    // kCGEventFlagMaskCommand = 0x100000
    // kCGHIDEventTap = 0
    // kVK_Command = 0x37
    const HID_SYSTEM: i32 = 1;
    const FLAG_CMD: u64 = 0x100000;
    const HID_TAP: u32 = 0;
    const KEY_CMD: u16 = 0x37;

    unsafe {
        let source = CGEventSourceCreate(HID_SYSTEM);
        if source.is_null() {
            return Err("无法创建键盘事件源".into());
        }

        let cmd_down = CGEventCreateKeyboardEvent(source, KEY_CMD, true);
        let key_down = CGEventCreateKeyboardEvent(source, virtual_key, true);
        let key_up = CGEventCreateKeyboardEvent(source, virtual_key, false);
        let cmd_up = CGEventCreateKeyboardEvent(source, KEY_CMD, false);
        if cmd_down.is_null() || key_down.is_null() || key_up.is_null() || cmd_up.is_null() {
            if !cmd_down.is_null() {
                CFRelease(cmd_down);
            }
            if !key_down.is_null() {
                CFRelease(key_down);
            }
            if !key_up.is_null() {
                CFRelease(key_up);
            }
            if !cmd_up.is_null() {
                CFRelease(cmd_up);
            }
            CFRelease(source);
            return Err("无法创建 Cmd 按键事件".into());
        }

        CGEventSetFlags(cmd_down, FLAG_CMD);
        CGEventSetFlags(key_down, FLAG_CMD);
        CGEventSetFlags(key_up, FLAG_CMD);
        CGEventSetFlags(cmd_up, 0);

        CGEventPost(HID_TAP, cmd_down);
        thread::sleep(Duration::from_millis(12));
        CGEventPost(HID_TAP, key_down);
        thread::sleep(Duration::from_millis(12));
        CGEventPost(HID_TAP, key_up);
        thread::sleep(Duration::from_millis(12));
        CGEventPost(HID_TAP, cmd_up);

        CFRelease(cmd_down);
        CFRelease(key_down);
        CFRelease(key_up);
        CFRelease(cmd_up);
        CFRelease(source);
    }
    Ok(())
}

/// Wait until clipboard text changes from `before`, or timeout.
fn wait_clipboard_change(cb: &mut Clipboard, before: &str, timeout_ms: u64) -> Option<String> {
    let start = std::time::Instant::now();
    let step = Duration::from_millis(25);
    while start.elapsed() < Duration::from_millis(timeout_ms) {
        thread::sleep(step);
        if let Some(t) = clipboard_text(cb) {
            if t != before {
                return Some(t);
            }
        }
    }
    clipboard_text(cb)
}

/// Copy the current selection into the system clipboard (simulate Cmd/Ctrl+C).
/// Returns the clipboard text after copy. Does not require prior clipboard text.
pub fn run_copy_selection() -> Result<String, String> {
    try_begin_copy_hotkey()?;
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| run_copy_selection_inner()));
    end_copy_hotkey();
    match result {
        Ok(r) => r,
        Err(_) => Err("复制选区异常中断（已恢复）".into()),
    }
}

fn run_copy_selection_inner() -> Result<String, String> {
    let mut cb = Clipboard::new().map_err(|e| format!("剪贴板不可用: {e}"))?;
    let before = clipboard_text(&mut cb).unwrap_or_default();

    // Release Shift/Alt/Cmd so synthetic Cmd+C does not re-fire the global hotkey.
    wait_modifiers_released(400);
    simulate_system_copy()?;
    thread::sleep(Duration::from_millis(50));

    // Prefer detecting change; if selection equals previous clipboard, change won't show.
    let after = wait_clipboard_change(&mut cb, &before, 380).unwrap_or_default();
    let text = after.trim().to_string();
    if text.is_empty() {
        return Err(
            "未获取到选中文本（请先选中内容；macOS 需在「辅助功能」中允许 FunCV）".into(),
        );
    }
    Ok(text)
}

/// Append currently selected text to the existing text clipboard.
///
/// - Only runs when clipboard already holds non-empty text.
/// - Waits for modifiers release, simulates Copy, merges with dedup.
/// - Returns the final clipboard string.
pub fn run_append_copy() -> Result<String, String> {
    try_begin_copy_hotkey().map_err(|_| "追加复制进行中".to_string())?;
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| run_append_copy_inner()));
    end_copy_hotkey();
    match result {
        Ok(r) => r,
        Err(_) => Err("追加复制异常中断（已恢复）".into()),
    }
}

fn run_append_copy_inner() -> Result<String, String> {
    let mut cb = Clipboard::new().map_err(|e| format!("剪贴板不可用: {e}"))?;

    let base = clipboard_text(&mut cb).ok_or_else(|| {
        "当前剪贴板不是文本（或为空），追加复制仅在已有文本时可用".to_string()
    })?;

    // Critical: release Cmd/Shift before injecting Cmd+C, else Cmd+Shift+C re-fires.
    wait_modifiers_released(400);

    simulate_system_copy()?;
    // Allow pasteboard to update
    thread::sleep(Duration::from_millis(40));

    let selected = wait_clipboard_change(&mut cb, &base, 350).unwrap_or_default();
    let selected = selected.trim().to_string();

    if selected.is_empty() {
        let _ = cb.set_text(&base);
        return Err("未获取到选中文本（请先选中内容；macOS 需在「辅助功能」中允许 FunCV）".into());
    }

    let merged = merge_append_text(&base, &selected);
    if merged == base {
        // Dedup hit — clipboard may already be `selected` from simulated copy
        cb.set_text(&base)
            .map_err(|e| format!("写回剪贴板失败: {e}"))?;
        return Ok(merged);
    }

    cb.set_text(&merged)
        .map_err(|e| format!("写回剪贴板失败: {e}"))?;
    Ok(merged)
}

#[cfg(test)]
mod tests {
    use super::merge_append_text;

    #[test]
    fn append_basic() {
        assert_eq!(merge_append_text("A", "B"), "A  B");
        assert_eq!(merge_append_text("A  B", "C"), "A  B  C");
    }

    #[test]
    fn append_dedup_last() {
        assert_eq!(merge_append_text("A  B", "B"), "A  B");
        assert_eq!(merge_append_text("A", "A"), "A");
        assert_eq!(merge_append_text("A  B  C", "C"), "A  B  C");
    }

    #[test]
    fn append_empty() {
        assert_eq!(merge_append_text("", "B"), "B");
        assert_eq!(merge_append_text("A", "  "), "A");
    }
}
