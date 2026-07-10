use super::manifest::{builtin_manifests, PluginInfo, PluginManifest};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RegistryFile {
    /// id → enabled override
    #[serde(default)]
    enabled: HashMap<String, bool>,
}

pub fn plugins_root(data_root: &Path) -> PathBuf {
    data_root.join("plugins")
}

pub fn user_plugins_dir(data_root: &Path) -> PathBuf {
    plugins_root(data_root).join("user")
}

fn registry_path(data_root: &Path) -> PathBuf {
    plugins_root(data_root).join("registry.json")
}

fn load_registry(data_root: &Path) -> RegistryFile {
    let path = registry_path(data_root);
    if !path.exists() {
        return RegistryFile::default();
    }
    fs::read_to_string(&path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

fn save_registry(data_root: &Path, reg: &RegistryFile) -> Result<(), String> {
    let root = plugins_root(data_root);
    fs::create_dir_all(&root).map_err(|e| e.to_string())?;
    let path = registry_path(data_root);
    let json = serde_json::to_string_pretty(reg).map_err(|e| e.to_string())?;
    fs::write(path, json).map_err(|e| e.to_string())
}

/// Ensure plugin directories exist.
pub fn ensure_dirs(data_root: &Path) -> Result<(), String> {
    fs::create_dir_all(user_plugins_dir(data_root)).map_err(|e| e.to_string())?;
    Ok(())
}

fn read_manifest(dir: &Path) -> Result<PluginManifest, String> {
    let path = dir.join("plugin.json");
    let raw = fs::read_to_string(&path).map_err(|e| format!("读取 plugin.json: {e}"))?;
    let mut m: PluginManifest =
        serde_json::from_str(&raw).map_err(|e| format!("解析 plugin.json: {e}"))?;
    m.builtin = false;
    if m.id.trim().is_empty() {
        return Err("plugin.json 缺少 id".into());
    }
    if m.name.trim().is_empty() {
        m.name = m.id.clone();
    }
    Ok(m)
}

/// List builtin + user plugins with enabled flags applied.
pub fn list_plugins(data_root: &Path) -> Result<Vec<PluginInfo>, String> {
    ensure_dirs(data_root)?;
    let reg = load_registry(data_root);
    let mut out: Vec<PluginInfo> = Vec::new();

    for mut m in builtin_manifests() {
        if let Some(&en) = reg.enabled.get(&m.id) {
            m.enabled = en;
        }
        out.push(PluginInfo::from_manifest(&m, None));
    }

    let user_dir = user_plugins_dir(data_root);
    if user_dir.is_dir() {
        let mut entries: Vec<_> = fs::read_dir(&user_dir)
            .map_err(|e| e.to_string())?
            .filter_map(|e| e.ok())
            .filter(|e| e.path().is_dir())
            .collect();
        entries.sort_by_key(|e| e.file_name());

        for ent in entries {
            let dir = ent.path();
            match read_manifest(&dir) {
                Ok(mut m) => {
                    if let Some(&en) = reg.enabled.get(&m.id) {
                        m.enabled = en;
                    }
                    out.push(PluginInfo::from_manifest(&m, Some(&dir)));
                }
                Err(err) => {
                    eprintln!("[nfun-cv] skip plugin {:?}: {err}", dir);
                }
            }
        }
    }

    Ok(out)
}

pub fn set_plugin_enabled(data_root: &Path, id: &str, enabled: bool) -> Result<(), String> {
    let mut reg = load_registry(data_root);
    reg.enabled.insert(id.to_string(), enabled);
    save_registry(data_root, &reg)
}

/// Import a plugin directory (must contain plugin.json). Copies into user plugins.
pub fn import_plugin_dir(data_root: &Path, source: &Path) -> Result<PluginInfo, String> {
    ensure_dirs(data_root)?;
    if !source.is_dir() {
        return Err("请选择包含 plugin.json 的插件目录".into());
    }
    let mut manifest = read_manifest(source)?;
    if matches!(manifest.runtime, super::manifest::PluginRuntime::Builtin) {
        return Err("不能导入 builtin 运行时插件".into());
    }
    if manifest.entry.trim().is_empty() {
        return Err("plugin.json 需要 entry 入口文件".into());
    }
    let entry_path = source.join(&manifest.entry);
    if !entry_path.exists() {
        return Err(format!("入口文件不存在: {}", manifest.entry));
    }

    // Sanitize id for path
    let safe_id: String = manifest
        .id
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();
    if safe_id.is_empty() {
        return Err("无效的插件 id".into());
    }
    manifest.id = safe_id.clone();

    let dest = user_plugins_dir(data_root).join(&safe_id);
    if dest.exists() {
        fs::remove_dir_all(&dest).map_err(|e| e.to_string())?;
    }
    copy_dir_recursive(source, &dest)?;

    // Rewrite manifest with normalized id
    let manifest_path = dest.join("plugin.json");
    let json = serde_json::to_string_pretty(&manifest).map_err(|e| e.to_string())?;
    fs::write(manifest_path, json).map_err(|e| e.to_string())?;

    let mut reg = load_registry(data_root);
    reg.enabled.insert(safe_id, true);
    save_registry(data_root, &reg)?;

    Ok(PluginInfo::from_manifest(&manifest, Some(&dest)))
}

pub fn remove_plugin(data_root: &Path, id: &str) -> Result<(), String> {
    if builtin_manifests().iter().any(|m| m.id == id) {
        return Err("内置插件不能删除，可在列表中关闭".into());
    }
    let dest = user_plugins_dir(data_root).join(id);
    if dest.exists() {
        fs::remove_dir_all(&dest).map_err(|e| e.to_string())?;
    }
    let mut reg = load_registry(data_root);
    reg.enabled.remove(id);
    save_registry(data_root, &reg)?;
    Ok(())
}

/// Resolve plugin dir + manifest for running.
pub fn resolve_plugin(
    data_root: &Path,
    id: &str,
) -> Result<(PluginManifest, Option<PathBuf>), String> {
    let reg = load_registry(data_root);
    for mut m in builtin_manifests() {
        if m.id == id {
            if let Some(&en) = reg.enabled.get(&m.id) {
                m.enabled = en;
            }
            return Ok((m, None));
        }
    }
    let dir = user_plugins_dir(data_root).join(id);
    if !dir.is_dir() {
        return Err(format!("插件不存在: {id}"));
    }
    let mut m = read_manifest(&dir)?;
    if let Some(&en) = reg.enabled.get(&m.id) {
        m.enabled = en;
    }
    Ok((m, Some(dir)))
}

fn copy_dir_recursive(src: &Path, dst: &Path) -> Result<(), String> {
    fs::create_dir_all(dst).map_err(|e| e.to_string())?;
    for entry in walkdir::WalkDir::new(src).min_depth(1) {
        let entry = entry.map_err(|e| e.to_string())?;
        let rel = entry.path().strip_prefix(src).map_err(|e| e.to_string())?;
        let target = dst.join(rel);
        if entry.file_type().is_dir() {
            fs::create_dir_all(&target).map_err(|e| e.to_string())?;
        } else if entry.file_type().is_file() {
            if let Some(parent) = target.parent() {
                fs::create_dir_all(parent).map_err(|e| e.to_string())?;
            }
            fs::copy(entry.path(), &target).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}
