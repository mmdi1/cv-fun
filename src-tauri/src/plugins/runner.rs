use super::manifest::PluginRuntime;
use super::registry::{list_plugins, resolve_plugin};
use super::translate::run_builtin_translate;
use serde::{Deserialize, Serialize};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;

/// Universal plugin input — only two fields for maximum extensibility.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginInput {
    pub content: String,
    /// `"text"` | `"img"` (and future types)
    #[serde(rename = "type")]
    pub r#type: String,
}

/// Universal plugin output.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginOutput {
    pub ok: bool,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub body: String,
    #[serde(default)]
    pub preview: String,
    #[serde(default)]
    pub hint: String,
    #[serde(default)]
    pub error: String,
    /// Filled by host
    #[serde(default, alias = "plugin_id")]
    pub plugin_id: String,
}

impl PluginOutput {
    pub fn fail(plugin_id: &str, err: impl Into<String>) -> Self {
        Self {
            ok: false,
            title: String::new(),
            body: String::new(),
            preview: String::new(),
            hint: String::new(),
            error: err.into(),
            plugin_id: plugin_id.into(),
        }
    }
}

pub fn run_plugin(
    data_root: &Path,
    plugin_id: &str,
    input: &PluginInput,
) -> Result<PluginOutput, String> {
    let (manifest, dir) = resolve_plugin(data_root, plugin_id)?;
    if !manifest.enabled {
        return Ok(PluginOutput::fail(plugin_id, "插件已禁用"));
    }
    if !manifest.types.is_empty() && !manifest.types.iter().any(|t| t == &input.r#type) {
        return Ok(PluginOutput::fail(
            plugin_id,
            format!("插件不支持 type={}", input.r#type),
        ));
    }

    let mut out = match manifest.runtime {
        PluginRuntime::Builtin => match plugin_id {
            "translate-en-zh" => run_builtin_translate(data_root, input)?,
            other => PluginOutput::fail(other, "未知内置插件"),
        },
        PluginRuntime::Node
        | PluginRuntime::Python
        | PluginRuntime::Go
        | PluginRuntime::Shell => {
            let dir = dir.ok_or_else(|| "外部插件缺少目录".to_string())?;
            run_external(&manifest.runtime, &dir, &manifest.entry, input, plugin_id)?
        }
    };
    out.plugin_id = plugin_id.into();
    if out.ok && out.preview.is_empty() && !out.body.is_empty() {
        let one = out.body.replace('\n', " ");
        out.preview = if one.chars().count() > 72 {
            format!("{}…", one.chars().take(72).collect::<String>())
        } else {
            one
        };
    }
    if out.ok && out.title.is_empty() {
        out.title = manifest.name;
    }
    Ok(out)
}

/// Run all enabled plugins that accept this type; skip failures quietly.
pub fn run_all_enabled(data_root: &Path, input: &PluginInput) -> Result<Vec<PluginOutput>, String> {
    let plugins = list_plugins(data_root)?;
    let mut results = Vec::new();
    for p in plugins {
        if !p.enabled {
            continue;
        }
        if !p.types.is_empty() && !p.types.iter().any(|t| t == &input.r#type) {
            continue;
        }
        match run_plugin(data_root, &p.id, input) {
            Ok(out) if out.ok && !out.body.is_empty() => results.push(out),
            Ok(_) => {}
            Err(err) => {
                eprintln!("[nfun-cv] plugin {} error: {err}", p.id);
            }
        }
    }
    Ok(results)
}

fn run_external(
    runtime: &PluginRuntime,
    dir: &Path,
    entry: &str,
    input: &PluginInput,
    plugin_id: &str,
) -> Result<PluginOutput, String> {
    let entry_path = dir.join(entry);
    if !entry_path.exists() {
        return Ok(PluginOutput::fail(
            plugin_id,
            format!("入口不存在: {}", entry_path.display()),
        ));
    }

    let payload = serde_json::to_string(input).map_err(|e| e.to_string())?;
    let mut cmd = build_command(runtime, dir, &entry_path)?;
    cmd.current_dir(dir)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    // Pass env for convenience
    cmd.env("NFUN_PLUGIN_ID", plugin_id);
    cmd.env("NFUN_PLUGIN_DIR", dir);

    let mut child = cmd.spawn().map_err(|e| format!("启动插件失败: {e}"))?;
    if let Some(mut stdin) = child.stdin.take() {
        stdin
            .write_all(payload.as_bytes())
            .map_err(|e| format!("写入 stdin: {e}"))?;
    }

    // Timeout ~8s
    let start = std::time::Instant::now();
    let timeout = Duration::from_secs(8);
    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) if start.elapsed() > timeout => {
                let _ = child.kill();
                return Ok(PluginOutput::fail(plugin_id, "插件执行超时 (8s)"));
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(30)),
            Err(e) => return Ok(PluginOutput::fail(plugin_id, format!("等待插件: {e}"))),
        }
    }

    let output = child
        .wait_with_output()
        .map_err(|e| format!("读取插件输出: {e}"))?;
    if !output.status.success() {
        let err = String::from_utf8_lossy(&output.stderr);
        let err = err.trim();
        return Ok(PluginOutput::fail(
            plugin_id,
            if err.is_empty() {
                format!("插件退出码 {:?}", output.status.code())
            } else {
                err.chars().take(400).collect()
            },
        ));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stdout = stdout.trim();
    if stdout.is_empty() {
        return Ok(PluginOutput::fail(plugin_id, "插件无输出"));
    }

    // Allow last JSON line (scripts may log to stdout)
    let json_line = stdout
        .lines()
        .rev()
        .find(|l| l.trim_start().starts_with('{'))
        .unwrap_or(stdout);

    match serde_json::from_str::<PluginOutput>(json_line) {
        Ok(mut o) => {
            o.plugin_id = plugin_id.into();
            Ok(o)
        }
        Err(e) => Ok(PluginOutput::fail(
            plugin_id,
            format!("输出 JSON 无效: {e}; raw={}", truncate(stdout, 200)),
        )),
    }
}

fn build_command(
    runtime: &PluginRuntime,
    _dir: &Path,
    entry: &PathBuf,
) -> Result<Command, String> {
    match runtime {
        PluginRuntime::Node => {
            let mut c = Command::new("node");
            c.arg(entry);
            Ok(c)
        }
        PluginRuntime::Python => {
            let py = which_first(&["python3", "python"]).ok_or("未找到 python3/python")?;
            let mut c = Command::new(py);
            c.arg(entry);
            Ok(c)
        }
        PluginRuntime::Go => {
            let path = entry.to_string_lossy();
            if path.ends_with(".go") {
                let mut c = Command::new("go");
                c.arg("run").arg(entry);
                Ok(c)
            } else {
                // Prebuilt binary
                Ok(Command::new(entry))
            }
        }
        PluginRuntime::Shell => {
            #[cfg(windows)]
            {
                let mut c = Command::new("cmd");
                c.arg("/C").arg(entry);
                Ok(c)
            }
            #[cfg(not(windows))]
            {
                let mut c = Command::new("sh");
                c.arg(entry);
                Ok(c)
            }
        }
        PluginRuntime::Builtin => Err("builtin 不走外部命令".into()),
    }
}

fn which_first(names: &[&str]) -> Option<String> {
    for name in names {
        if Command::new(name)
            .arg("--version")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
        {
            return Some((*name).into());
        }
    }
    None
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        format!("{}…", s.chars().take(max).collect::<String>())
    }
}
