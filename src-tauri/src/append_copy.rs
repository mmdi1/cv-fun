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

/// Wait until common modifiers are up so synthetic Cmd+C does not re-fire Cmd+Shift+C etc.
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
    #[cfg(not(target_os = "macos"))]
    {
        // Best-effort delay for Windows/Linux
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

/// Simulate system Copy via CGEvent on macOS (main-thread preferred), enigo elsewhere.
fn simulate_system_copy() -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        return macos_post_cmd_c();
    }
    #[cfg(not(target_os = "macos"))]
    {
        use enigo::{Direction, Enigo, Key, Keyboard, Settings};
        let mut enigo =
            Enigo::new(&Settings::default()).map_err(|e| format!("键盘模拟初始化失败: {e}"))?;
        enigo
            .key(Key::Control, Direction::Press)
            .map_err(|e| format!("按键失败: {e}"))?;
        thread::sleep(Duration::from_millis(12));
        enigo
            .key(Key::Unicode('c'), Direction::Click)
            .map_err(|e| format!("按键失败: {e}"))?;
        thread::sleep(Duration::from_millis(12));
        enigo
            .key(Key::Control, Direction::Release)
            .map_err(|e| format!("按键失败: {e}"))?;
        Ok(())
    }
}

#[cfg(target_os = "macos")]
fn macos_post_cmd_c() -> Result<(), String> {
    // Use CoreGraphics keyboard events directly — more predictable than enigo on hotkey paths.
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
    // kVK_ANSI_C = 0x08
    // kCGEventFlagMaskCommand = 0x100000
    // kCGHIDEventTap = 0
    const HID_SYSTEM: i32 = 1;
    const KEY_C: u16 = 0x08;
    const FLAG_CMD: u64 = 0x100000;
    const HID_TAP: u32 = 0;

    unsafe {
        let source = CGEventSourceCreate(HID_SYSTEM);
        if source.is_null() {
            return Err("无法创建键盘事件源".into());
        }

        let down = CGEventCreateKeyboardEvent(source, KEY_C, true);
        let up = CGEventCreateKeyboardEvent(source, KEY_C, false);
        if down.is_null() || up.is_null() {
            CFRelease(source);
            return Err("无法创建 Cmd+C 事件".into());
        }
        CGEventSetFlags(down, FLAG_CMD);
        CGEventSetFlags(up, FLAG_CMD);
        CGEventPost(HID_TAP, down);
        thread::sleep(Duration::from_millis(8));
        CGEventPost(HID_TAP, up);

        CFRelease(down);
        CFRelease(up);
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
