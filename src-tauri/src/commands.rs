use crate::append_copy::{run_append_copy, run_copy_selection};
use crate::config::{
    load_config, save_config, to_append_copy_hotkey, to_global_hotkey, to_parse_copy_hotkey,
    AppConfig,
};
use crate::history::{HistoryItem, HistoryStore, ItemKind};
use crate::paste_popup;
use arboard::{Clipboard, ImageData};
use parking_lot::Mutex;
use std::borrow::Cow;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex as StdMutex};
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};

pub struct AppState {
    pub store: Mutex<HistoryStore>,
    pub data_root: PathBuf,
    pub registered_hotkeys: StdMutex<Vec<String>>,
    /// Live flags for clipboard watcher (updated on save without restart).
    pub watch_text: Arc<AtomicBool>,
    pub watch_image: Arc<AtomicBool>,
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

/// Clear history. `mode`: `keep_saved` | `keep_favorites` | `all`
#[tauri::command]
pub fn clear_history(state: State<'_, AppState>, mode: Option<String>) -> Result<u32, String> {
    let mode = mode.unwrap_or_else(|| "all".into());
    let n = state.store.lock().clear(&mode).map_err(map_err)?;
    Ok(n as u32)
}

#[tauri::command]
pub fn set_history_pinned(
    state: State<'_, AppState>,
    id: String,
    pinned: bool,
) -> Result<HistoryItem, String> {
    state.store.lock().set_pinned(&id, pinned).map_err(map_err)
}

#[tauri::command]
pub fn set_history_favorited(
    state: State<'_, AppState>,
    id: String,
    favorited: bool,
) -> Result<HistoryItem, String> {
    state
        .store
        .lock()
        .set_favorited(&id, favorited)
        .map_err(map_err)
}

#[tauri::command]
pub fn set_history_note(
    state: State<'_, AppState>,
    id: String,
    note: String,
) -> Result<HistoryItem, String> {
    state.store.lock().set_note(&id, &note).map_err(map_err)
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

    // Plugin / parse result always wins when override is provided (even for image items).
    if let Some(text) = text_override {
        clipboard.set_text(text).map_err(map_err)?;
        store.mark_used(&id).map_err(map_err)?;
        return Ok(());
    }

    match item.kind {
        ItemKind::Text => {
            let text = item.text.unwrap_or_default();
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

/// Local OCR for a history image item. macOS Vision / Windows Media.Ocr.
#[tauri::command]
pub fn ocr_history_image(state: State<'_, AppState>, id: String) -> Result<String, String> {
    let store = state.store.lock();
    let item = store.get(&id).map_err(map_err)?;
    if item.kind != ItemKind::Image {
        return Err("仅支持图片记录".into());
    }
    let rel = item
        .image_path
        .ok_or_else(|| "图片路径缺失".to_string())?;
    let abs = store.root().join(rel);
    drop(store);
    crate::ocr::ocr_image_file(&abs)
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
    config.append_copy_hotkey = config.append_copy_hotkey.trim().to_string();
    config.parse_copy_hotkey = config.parse_copy_hotkey.trim().to_string();
    config.paste_hotkey = config.paste_hotkey.trim().to_string();

    if !config.append_copy_hotkey.is_empty() {
        let _ = to_append_copy_hotkey(&config.append_copy_hotkey).map_err(map_err)?;
        if config.append_copy_hotkey.eq_ignore_ascii_case(config.toggle_hotkey.trim()) {
            return Err("追加复制快捷键不能与唤起/隐藏快捷键相同".into());
        }
    }
    if !config.parse_copy_hotkey.is_empty() {
        let _ = to_parse_copy_hotkey(&config.parse_copy_hotkey).map_err(map_err)?;
        if config.parse_copy_hotkey.eq_ignore_ascii_case(config.toggle_hotkey.trim()) {
            return Err("复制并解析快捷键不能与唤起/隐藏快捷键相同".into());
        }
        if !config.append_copy_hotkey.is_empty()
            && config
                .parse_copy_hotkey
                .eq_ignore_ascii_case(config.append_copy_hotkey.trim())
        {
            return Err("复制并解析快捷键不能与追加复制快捷键相同".into());
        }
    }
    if !config.paste_hotkey.is_empty() {
        let _ = to_global_hotkey(&config.paste_hotkey).map_err(map_err)?;
        let paste = config.paste_hotkey.trim();
        if paste.eq_ignore_ascii_case(config.toggle_hotkey.trim()) {
            return Err("快速粘贴快捷键不能与唤起/隐藏快捷键相同".into());
        }
        if !config.append_copy_hotkey.is_empty()
            && paste.eq_ignore_ascii_case(config.append_copy_hotkey.trim())
        {
            return Err("快速粘贴快捷键不能与追加复制快捷键相同".into());
        }
        if !config.parse_copy_hotkey.is_empty()
            && paste.eq_ignore_ascii_case(config.parse_copy_hotkey.trim())
        {
            return Err("快速粘贴快捷键不能与复制并解析快捷键相同".into());
        }
    }

    save_config(&state.data_root, &config).map_err(map_err)?;
    state.store.lock().set_max_items(config.history.max_items);
    state
        .watch_text
        .store(config.watch_text, Ordering::SeqCst);
    state
        .watch_image
        .store(config.watch_image, Ordering::SeqCst);
    register_app_hotkeys(
        &app,
        &state,
        &config.toggle_hotkey,
        &config.append_copy_hotkey,
        &config.parse_copy_hotkey,
        &config.paste_hotkey,
    )?;
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

/// Register toggle + optional append-copy / parse-copy / quick-paste global shortcuts.
pub fn register_app_hotkeys(
    app: &AppHandle,
    state: &AppState,
    toggle_display: &str,
    append_display: &str,
    parse_display: &str,
    paste_display: &str,
) -> Result<(), String> {
    let _ = app.global_shortcut().unregister_all();
    if let Ok(mut registered) = state.registered_hotkeys.lock() {
        registered.clear();
    }

    // Toggle main window
    {
        let hotkey = to_global_hotkey(toggle_display).map_err(map_err)?;
        let shortcut: Shortcut = hotkey
            .parse()
            .map_err(|e| format!("无效快捷键 {toggle_display} ({hotkey}): {e}"))?;
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
    }

    // Append copy (optional; empty = disabled)
    let append = append_display.trim();
    if !append.is_empty() {
        let hotkey = to_append_copy_hotkey(append).map_err(map_err)?;
        let shortcut: Shortcut = hotkey
            .parse()
            .map_err(|e| format!("无效追加复制快捷键 {append} ({hotkey}): {e}"))?;
        let app_handle = app.clone();
        app.global_shortcut()
            .on_shortcut(shortcut, move |_app, _shortcut, event| {
                if event.state != ShortcutState::Pressed {
                    return;
                }
                let handle = app_handle.clone();
                std::thread::Builder::new()
                    .name("append-copy".into())
                    .spawn(move || {
                        let payload = match run_append_copy() {
                            Ok(merged) => {
                                let preview: String = merged.chars().take(48).collect();
                                let message = if merged.chars().count() > 48 {
                                    format!("已追加复制 · {preview}…")
                                } else {
                                    format!("已追加复制 · {preview}")
                                };
                                serde_json::json!({ "ok": true, "message": message })
                            }
                            Err(err) => {
                                if err.contains("进行中") || err.contains("处理中") {
                                    return;
                                }
                                serde_json::json!({ "ok": false, "message": err })
                            }
                        };
                        let _ = handle.emit("append-copy-result", payload);
                    })
                    .ok();
            })
            .map_err(map_err)?;
        if let Ok(mut registered) = state.registered_hotkeys.lock() {
            registered.push(hotkey);
        }
    }

    // Copy selection + show app for parse (optional)
    let parse = parse_display.trim();
    if !parse.is_empty() {
        let hotkey = to_parse_copy_hotkey(parse).map_err(map_err)?;
        let shortcut: Shortcut = hotkey
            .parse()
            .map_err(|e| format!("无效复制并解析快捷键 {parse} ({hotkey}): {e}"))?;
        let app_handle = app.clone();
        app.global_shortcut()
            .on_shortcut(shortcut, move |_app, _shortcut, event| {
                if event.state != ShortcutState::Pressed {
                    return;
                }
                let handle = app_handle.clone();
                std::thread::Builder::new()
                    .name("parse-copy".into())
                    .spawn(move || {
                        let result = run_copy_selection();
                        // Always raise the app so user can see parse (or the error).
                        crate::show_main(&handle);
                        match result {
                            Ok(text) => {
                                let preview: String = text.chars().take(48).collect();
                                let message = if text.chars().count() > 48 {
                                    format!("已复制并打开 · {preview}…")
                                } else {
                                    format!("已复制并打开 · {preview}")
                                };
                                let _ = handle.emit(
                                    "parse-copy-result",
                                    serde_json::json!({ "ok": true, "message": message }),
                                );
                            }
                            Err(err) => {
                                if err.contains("处理中") {
                                    return;
                                }
                                let _ = handle.emit(
                                    "parse-copy-result",
                                    serde_json::json!({ "ok": false, "message": err }),
                                );
                            }
                        }
                    })
                    .ok();
            })
            .map_err(map_err)?;
        if let Ok(mut registered) = state.registered_hotkeys.lock() {
            registered.push(hotkey);
        }
    }

    // Quick paste popup at cursor (optional)
    let paste = paste_display.trim();
    if !paste.is_empty() {
        let hotkey = to_global_hotkey(paste).map_err(map_err)?;
        let shortcut: Shortcut = hotkey
            .parse()
            .map_err(|e| format!("无效快速粘贴快捷键 {paste} ({hotkey}): {e}"))?;
        let app_handle = app.clone();
        app.global_shortcut()
            .on_shortcut(shortcut, move |_app, _shortcut, event| {
                if event.state != ShortcutState::Pressed {
                    return;
                }
                // Remember frontmost app *before* FunCV steals focus.
                paste_popup::capture_previous_app();
                paste_popup::show_paste_popup(&app_handle);
            })
            .map_err(map_err)?;
        if let Ok(mut registered) = state.registered_hotkeys.lock() {
            registered.push(hotkey);
        }
    }

    Ok(())
}

/// Recent history for the paste popup (max 12).
#[tauri::command]
pub fn list_paste_history(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<Vec<HistoryItem>, String> {
    let items = paste_popup::list_for_paste(&state)?;
    paste_popup::resize_paste_window(&app, items.len());
    Ok(items)
}

#[tauri::command]
pub fn hide_paste_popup(app: AppHandle) -> Result<(), String> {
    paste_popup::hide_paste_popup(&app);
    Ok(())
}

/// Copy history item and simulate paste into the previously focused app.
#[tauri::command]
pub fn paste_history_item(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
) -> Result<(), String> {
    paste_popup::paste_history_item(&app, &state, &id)
}

/// "查看更多" — open main UI from the paste popup.
#[tauri::command]
pub fn open_main_from_paste(app: AppHandle) -> Result<(), String> {
    paste_popup::hide_paste_popup(&app);
    crate::show_main(&app);
    Ok(())
}


#[tauri::command]
pub fn data_root_path(state: State<'_, AppState>) -> Result<String, String> {
    Ok(state.data_root.to_string_lossy().into_owned())
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportResult {
    pub path: String,
    pub count: u32,
    pub mode: String,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportResult {
    pub mode: String,
    pub path: String,
    /// saved: newly merged/inserted; full: total rows after restore
    pub count: u32,
    /// saved only: skipped rows
    pub skipped: u32,
}

/// Export data. `mode`: `saved` | `full`.
/// - saved: pinned/favorited → export.json (+ images if include_images)
/// - full: copy history.db + images (+ config if include_config)
#[tauri::command]
pub fn export_data(
    state: State<'_, AppState>,
    mode: String,
    dest_dir: String,
    include_images: Option<bool>,
    include_config: Option<bool>,
) -> Result<ExportResult, String> {
    let dest = PathBuf::from(dest_dir.trim());
    if dest_dir.trim().is_empty() || !dest.is_dir() {
        return Err("请选择有效的导出目录".into());
    }
    let store = state.store.lock();
    let mode = mode.trim().to_ascii_lowercase();
    let (path, count) = match mode.as_str() {
        "saved" | "favorites" | "pinned" => store
            .export_saved(&dest, include_images.unwrap_or(true))
            .map_err(map_err)?,
        "full" | "all" => store
            .export_full(&dest, include_config.unwrap_or(true))
            .map_err(map_err)?,
        other => return Err(format!("未知导出模式: {other}")),
    };
    Ok(ExportResult {
        path: path.to_string_lossy().into_owned(),
        count: count as u32,
        mode,
    })
}

/// Import data. `mode`: `saved` | `full` | `auto` (detect from folder).
/// `source_path`: FunCV-saved-* or FunCV-full-* directory (or export.json for saved).
#[tauri::command]
pub fn import_data(
    state: State<'_, AppState>,
    mode: String,
    source_path: String,
    include_config: Option<bool>,
) -> Result<ImportResult, String> {
    let src = PathBuf::from(source_path.trim());
    if source_path.trim().is_empty() || (!src.is_dir() && !src.is_file()) {
        return Err("请选择有效的导入路径".into());
    }

    let mode = mode.trim().to_ascii_lowercase();
    let detected = if mode == "auto" || mode.is_empty() {
        if src.join("history.db").is_file()
            || (src.is_file()
                && src
                    .file_name()
                    .and_then(|s| s.to_str())
                    .map(|s| s == "history.db")
                    .unwrap_or(false))
        {
            "full"
        } else if src.join("export.json").is_file()
            || (src.is_file()
                && src
                    .file_name()
                    .and_then(|s| s.to_str())
                    .map(|s| s.eq_ignore_ascii_case("export.json"))
                    .unwrap_or(false))
        {
            "saved"
        } else {
            return Err(
                "无法识别包类型：需要 export.json（收藏）或 history.db（完整库）".into(),
            );
        }
    } else {
        mode.as_str()
    };

    let package = if src.is_file() {
        src.parent()
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| PathBuf::from("."))
    } else {
        src.clone()
    };

    let store = state.store.lock();
    match detected {
        "saved" | "favorites" => {
            let (count, skipped) = store.import_saved(&package).map_err(map_err)?;
            Ok(ImportResult {
                mode: "saved".into(),
                path: package.to_string_lossy().into_owned(),
                count: count as u32,
                skipped: skipped as u32,
            })
        }
        "full" | "all" => {
            let count = store
                .import_full(&package, include_config.unwrap_or(true))
                .map_err(map_err)?;
            Ok(ImportResult {
                mode: "full".into(),
                path: package.to_string_lossy().into_owned(),
                count: count as u32,
                skipped: 0,
            })
        }
        other => Err(format!("未知导入模式: {other}")),
    }
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

#[tauri::command]
pub fn get_fun_stats(state: State<'_, AppState>) -> Result<crate::history::StatsSnapshot, String> {
    state.store.lock().fun_stats().map_err(map_err)
}

// -------------------- Plugins --------------------

#[tauri::command]
pub fn list_plugins(state: State<'_, AppState>) -> Result<Vec<crate::plugins::PluginInfo>, String> {
    crate::plugins::list_plugins(&state.data_root)
}

#[tauri::command]
pub fn set_plugin_enabled(
    state: State<'_, AppState>,
    id: String,
    enabled: bool,
) -> Result<(), String> {
    crate::plugins::set_plugin_enabled(&state.data_root, &id, enabled)
}

#[tauri::command]
pub fn import_plugin(state: State<'_, AppState>, path: String) -> Result<crate::plugins::PluginInfo, String> {
    crate::plugins::import_plugin_dir(&state.data_root, std::path::Path::new(&path))
}

#[tauri::command]
pub fn remove_plugin(state: State<'_, AppState>, id: String) -> Result<(), String> {
    crate::plugins::remove_plugin(&state.data_root, &id)
}

#[tauri::command]
pub fn run_plugin(
    state: State<'_, AppState>,
    id: String,
    content: String,
    r#type: String,
) -> Result<crate::plugins::PluginOutput, String> {
    let input = crate::plugins::PluginInput {
        content,
        r#type,
    };
    crate::plugins::run_plugin(&state.data_root, &id, &input)
}

#[tauri::command]
pub fn run_enabled_plugins(
    state: State<'_, AppState>,
    content: String,
    r#type: String,
) -> Result<Vec<crate::plugins::PluginOutput>, String> {
    let input = crate::plugins::PluginInput {
        content,
        r#type,
    };
    crate::plugins::run_all_enabled(&state.data_root, &input)
}

#[tauri::command]
pub fn ecdict_status(state: State<'_, AppState>) -> Result<crate::plugins::EcdictStatus, String> {
    Ok(crate::plugins::ecdict_ready(&state.data_root))
}

/// Download + import ECDICT (may take several minutes). Progress via `ecdict-progress` event.
#[tauri::command]
pub async fn install_ecdict(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<crate::plugins::EcdictStatus, String> {
    let root = state.data_root.clone();
    let app2 = app.clone();
    tauri::async_runtime::spawn_blocking(move || crate::plugins::install_ecdict(&app2, &root))
        .await
        .map_err(|e| format!("词典安装任务失败: {e}"))?
}

#[tauri::command]
pub fn plugin_protocol_help() -> String {
    r#"# FunCV 插件协议

## 1. 通用入参（仅 2 个字段）

通过 stdin 传入 JSON：

{
  "content": "剪贴板文本，或图片相关路径",
  "type": "text"
}

type 取值：
- "text"  文本剪贴板
- "img"   图片剪贴板（可扩展更多类型）

## 2. 通用出参

通过 stdout 输出 JSON（建议最后一行是完整 JSON）：

成功：
{
  "ok": true,
  "title": "显示标题（推荐解析卡片）",
  "body": "完整结果（点击后写入主面板）",
  "preview": "一行预览",
  "hint": "可选副标题"
}

失败：
{ "ok": false, "error": "原因说明" }

## 3. plugin.json 清单

{
  "id": "my-plugin",
  "name": "我的插件",
  "version": "1.0.0",
  "runtime": "node",
  "entry": "main.js",
  "description": "说明文字",
  "types": ["text", "img"],
  "enabled": true
}

runtime 可选：node | python | go | shell

## 4. 运行时如何启动

- node   →  node <entry>
- python →  python3 <entry>
- go     →  go run <entry.go>  或直接执行二进制
- shell  →  sh <entry>

工作目录 = 插件目录；环境变量：
- NFUN_PLUGIN_ID
- NFUN_PLUGIN_DIR

## 5. 开发流程

1. 在「自定义」页下载示例
2. 修改脚本，保证 stdin/stdout 协议
3. 在「列表」页点击「上传插件」选择目录导入
4. 启用后，选中历史记录时自动参与推荐解析
"#
    .into()
}

#[tauri::command]
pub fn list_plugin_samples() -> Vec<crate::plugins::SamplePack> {
    crate::plugins::list_sample_packs()
}

/// Export one sample pack into a parent directory (creates subfolder by sample id).
#[tauri::command]
pub fn export_plugin_sample(id: String, dest_dir: String) -> Result<String, String> {
    crate::plugins::export_sample_pack(&id, std::path::Path::new(&dest_dir))
}

/// Export all sample packs into dest_dir.
#[tauri::command]
pub fn export_all_plugin_samples(dest_dir: String) -> Result<Vec<String>, String> {
    crate::plugins::export_all_samples(std::path::Path::new(&dest_dir))
}


