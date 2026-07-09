use crate::config::{load_config, save_config, to_global_hotkey, AppConfig};
use crate::history::{HistoryItem, HistoryStore, ItemKind};
use arboard::{Clipboard, ImageData};
use parking_lot::Mutex;
use std::borrow::Cow;
use std::path::PathBuf;
use std::sync::Mutex as StdMutex;
use tauri::{AppHandle, Manager, State};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};

pub struct AppState {
    pub store: Mutex<HistoryStore>,
    pub data_root: PathBuf,
    pub registered_hotkeys: StdMutex<Vec<String>>,
}

fn map_err(err: impl ToString) -> String {
    err.to_string()
}

#[tauri::command]
pub fn list_history(state: State<'_, AppState>, query: String) -> Result<Vec<HistoryItem>, String> {
    state.store.lock().list(&query).map_err(map_err)
}

#[tauri::command]
pub fn delete_history(state: State<'_, AppState>, id: String) -> Result<(), String> {
    state.store.lock().delete(&id).map_err(map_err)
}

#[tauri::command]
pub fn clear_history(state: State<'_, AppState>) -> Result<(), String> {
    state.store.lock().clear().map_err(map_err)
}

#[tauri::command]
pub fn get_history_item(
    state: State<'_, AppState>,
    id: String,
) -> Result<HistoryItem, String> {
    state.store.lock().get(&id).map_err(map_err)
}

/// Copy a history item back to the system clipboard.
/// When `text_override` is set (frontend inferred transform), write that text instead of stored raw.
#[tauri::command]
pub fn copy_history(
    state: State<'_, AppState>,
    id: String,
    text_override: Option<String>,
) -> Result<(), String> {
    let store = state.store.lock();
    let item = store.get(&id).map_err(map_err)?;
    let mut clipboard = Clipboard::new().map_err(map_err)?;

    match item.kind {
        ItemKind::Text => {
            let text = text_override.unwrap_or_else(|| item.text.unwrap_or_default());
            clipboard.set_text(text).map_err(map_err)?;
        }
        ItemKind::Image => {
            let rel = item
                .image_path
                .ok_or_else(|| "image path missing".to_string())?;
            let abs = store.root().join(rel);
            let img = image::open(&abs).map_err(map_err)?.to_rgba8();
            let (w, h) = img.dimensions();
            let data = ImageData {
                width: w as usize,
                height: h as usize,
                bytes: Cow::Owned(img.into_raw()),
            };
            clipboard.set_image(data).map_err(map_err)?;
        }
    }
    store.mark_used(&id).map_err(map_err)?;
    Ok(())
}

/// Absolute filesystem path for an image item (frontend convertFileSrc).
#[tauri::command]
pub fn resolve_image_path(state: State<'_, AppState>, id: String) -> Result<String, String> {
    let store = state.store.lock();
    let item = store.get(&id).map_err(map_err)?;
    let rel = item
        .image_path
        .ok_or_else(|| "not an image item".to_string())?;
    let abs = store.root().join(rel);
    if !abs.exists() {
        return Err("image file missing".into());
    }
    Ok(abs.to_string_lossy().into_owned())
}

#[tauri::command]
pub fn get_config(state: State<'_, AppState>) -> Result<AppConfig, String> {
    load_config(&state.data_root).map_err(map_err)
}

#[tauri::command]
pub fn save_app_config(
    app: AppHandle,
    state: State<'_, AppState>,
    config: AppConfig,
) -> Result<AppConfig, String> {
    let mut config = config;
    if config.history.max_items == 0 {
        config.history.max_items = 500;
    }
    if config.poll_interval_ms < 150 {
        config.poll_interval_ms = 150;
    }
    if config.toggle_hotkey.trim().is_empty() {
        return Err("请设置唤起/隐藏快捷键".into());
    }
    // Validate hotkey before saving
    let _ = to_global_hotkey(&config.toggle_hotkey).map_err(map_err)?;

    save_config(&state.data_root, &config).map_err(map_err)?;
    state.store.lock().set_max_items(config.history.max_items);
    register_toggle_hotkey(&app, &state, &config.toggle_hotkey)?;
    Ok(config)
}

pub fn toggle_main_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let visible = window.is_visible().unwrap_or(false);
        let focused = window.is_focused().unwrap_or(false);
        let minimized = window.is_minimized().unwrap_or(false);

        // Only hide when the panel is already frontmost.
        // Visible but buried under other apps (Dock icon present, not top) → raise + focus.
        if visible && focused && !minimized {
            crate::hide_to_tray(app);
        } else {
            crate::show_main(app);
        }
    } else {
        crate::show_main(app);
    }
}

pub fn register_toggle_hotkey(
    app: &AppHandle,
    state: &AppState,
    display: &str,
) -> Result<(), String> {
    let _ = app.global_shortcut().unregister_all();
    if let Ok(mut registered) = state.registered_hotkeys.lock() {
        registered.clear();
    }

    let hotkey = to_global_hotkey(display).map_err(map_err)?;
    let shortcut: Shortcut = hotkey
        .parse()
        .map_err(|e| format!("无效快捷键 {display} ({hotkey}): {e}"))?;

    let app_handle = app.clone();
    app.global_shortcut()
        .on_shortcut(shortcut, move |_app, _shortcut, event| {
            if event.state != ShortcutState::Pressed {
                return;
            }
            toggle_main_window(&app_handle);
        })
        .map_err(map_err)?;

    if let Ok(mut registered) = state.registered_hotkeys.lock() {
        registered.push(hotkey);
    }
    Ok(())
}

#[tauri::command]
pub fn data_root_path(state: State<'_, AppState>) -> Result<String, String> {
    Ok(state.data_root.to_string_lossy().into_owned())
}

#[tauri::command]
pub fn hide_main_window(app: AppHandle) -> Result<(), String> {
    crate::hide_to_tray(&app);
    Ok(())
}

#[tauri::command]
pub fn show_main_window(app: AppHandle) -> Result<(), String> {
    crate::show_main(&app);
    Ok(())
}


