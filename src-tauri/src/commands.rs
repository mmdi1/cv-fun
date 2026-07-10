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


