use crate::history::HistoryError;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

const CONFIG_FILE: &str = "config.json";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppConfig {
    pub history: HistoryConfig,
    pub poll_interval_ms: u64,
    /// Display form e.g. "Option+Space" / "Ctrl+Shift+V"
    #[serde(default = "default_toggle_hotkey")]
    pub toggle_hotkey: String,
    /// 3-key combo: append selection to existing text clipboard with double-space.
    /// Empty string = disabled.
    #[serde(default = "default_append_copy_hotkey")]
    pub append_copy_hotkey: String,
    /// 3-key combo: copy selection then show FunCV for parse.
    /// Empty string = disabled. e.g. "Shift+Option+X"
    #[serde(default = "default_parse_copy_hotkey")]
    pub parse_copy_hotkey: String,
    /// Quick paste popup at cursor. Empty = disabled. e.g. "Cmd+F2"
    #[serde(default = "default_paste_hotkey")]
    pub paste_hotkey: String,
    /// Capture text clipboard changes into history (default true).
    #[serde(default = "default_true")]
    pub watch_text: bool,
    /// Capture image clipboard changes into history (default true).
    #[serde(default = "default_true")]
    pub watch_image: bool,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryConfig {
    pub max_items: usize,
}

fn default_toggle_hotkey() -> String {
    if cfg!(target_os = "macos") {
        "Option+Space".into()
    } else {
        "Alt+Space".into()
    }
}

fn default_append_copy_hotkey() -> String {
    if cfg!(target_os = "macos") {
        "Cmd+Shift+C".into()
    } else {
        "Ctrl+Shift+C".into()
    }
}

fn default_parse_copy_hotkey() -> String {
    if cfg!(target_os = "macos") {
        "Shift+Option+X".into()
    } else {
        "Shift+Alt+X".into()
    }
}

fn default_paste_hotkey() -> String {
    if cfg!(target_os = "macos") {
        "Cmd+F2".into()
    } else {
        "Ctrl+F2".into()
    }
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            history: HistoryConfig { max_items: 500 },
            poll_interval_ms: 400,
            toggle_hotkey: default_toggle_hotkey(),
            append_copy_hotkey: default_append_copy_hotkey(),
            parse_copy_hotkey: default_parse_copy_hotkey(),
            paste_hotkey: default_paste_hotkey(),
            watch_text: true,
            watch_image: true,
        }
    }
}

pub fn load_config(root: &PathBuf) -> Result<AppConfig, HistoryError> {
    let path = root.join(CONFIG_FILE);
    if !path.exists() {
        let defaults = AppConfig::default();
        save_config(root, &defaults)?;
        return Ok(defaults);
    }
    let data = fs::read(&path)?;
    if data.iter().all(|b| b.is_ascii_whitespace()) {
        return Ok(AppConfig::default());
    }
    let mut config: AppConfig = serde_json::from_slice(&data)
        .map_err(|e| HistoryError::Message(format!("parse config: {e}")))?;
    sanitize_config(&mut config);
    Ok(config)
}

pub fn save_config(root: &PathBuf, config: &AppConfig) -> Result<(), HistoryError> {
    fs::create_dir_all(root)?;
    let mut config = config.clone();
    sanitize_config(&mut config);
    let data = serde_json::to_vec_pretty(&config)
        .map_err(|e| HistoryError::Message(format!("serialize config: {e}")))?;
    let path = root.join(CONFIG_FILE);
    let temp = path.with_extension("json.tmp");
    fs::write(&temp, data)?;
    fs::rename(temp, path)?;
    Ok(())
}

fn sanitize_config(config: &mut AppConfig) {
    if config.history.max_items == 0 {
        config.history.max_items = 500;
    }
    if config.poll_interval_ms < 150 {
        config.poll_interval_ms = 150;
    }
    if config.toggle_hotkey.trim().is_empty() {
        config.toggle_hotkey = default_toggle_hotkey();
    }
    config.append_copy_hotkey = config.append_copy_hotkey.trim().to_string();
    config.parse_copy_hotkey = config.parse_copy_hotkey.trim().to_string();
    config.paste_hotkey = config.paste_hotkey.trim().to_string();
    // If both off, force text on so the app is never fully deaf
    if !config.watch_text && !config.watch_image {
        config.watch_text = true;
    }
}

/// Convert UI display shortcut (Option+Space / Ctrl+Shift+V) into global-hotkey format.
pub fn to_global_hotkey(display: &str) -> Result<String, HistoryError> {
    let parts: Vec<&str> = display
        .split('+')
        .map(str::trim)
        .filter(|p| !p.is_empty())
        .collect();
    if parts.len() < 2 {
        return Err(HistoryError::Message(format!(
            "快捷键需包含修饰键: {display}"
        )));
    }

    let mut mods = Vec::new();
    let mut key: Option<String> = None;
    for part in parts {
        let lower = part.to_ascii_lowercase();
        match lower.as_str() {
            "option" | "alt" => mods.push("Alt"),
            "ctrl" | "control" => mods.push("Control"),
            "cmd" | "command" | "meta" | "super" => mods.push("Super"),
            "shift" => mods.push("Shift"),
            " " | "space" | "spacebar" => key = Some("Space".into()),
            "," | "comma" => key = Some("Comma".into()),
            "." | "period" => key = Some("Period".into()),
            other if other.len() == 1 => {
                let c = other.chars().next().unwrap();
                key = Some(c.to_ascii_uppercase().to_string());
            }
            other => {
                let mut s = other.to_string();
                if let Some(first) = s.get_mut(0..1) {
                    first.make_ascii_uppercase();
                }
                key = Some(s);
            }
        }
    }

    let key = key.ok_or_else(|| HistoryError::Message(format!("快捷键缺少主键: {display}")))?;
    mods.push(key.as_str());
    Ok(mods.join("+"))
}

/// 3-key combo (2 modifiers + key), e.g. Cmd+Shift+C / Shift+Option+X.
pub fn to_three_key_hotkey(display: &str, label: &str) -> Result<String, HistoryError> {
    let display = display.trim();
    if display.is_empty() {
        return Err(HistoryError::Message(format!("{label}快捷键为空")));
    }
    let parts: Vec<&str> = display
        .split('+')
        .map(str::trim)
        .filter(|p| !p.is_empty())
        .collect();
    if parts.len() < 3 {
        return Err(HistoryError::Message(format!(
            "{label}需 3 键组合（两个修饰键 + 主键），例如 Shift+Option+X：{display}"
        )));
    }
    to_global_hotkey(display)
}

/// Append-copy requires a 3-key combination.
pub fn to_append_copy_hotkey(display: &str) -> Result<String, HistoryError> {
    to_three_key_hotkey(display, "追加复制")
}

/// Parse-copy (copy selection + show app) requires a 3-key combination.
pub fn to_parse_copy_hotkey(display: &str) -> Result<String, HistoryError> {
    to_three_key_hotkey(display, "复制并解析")
}
