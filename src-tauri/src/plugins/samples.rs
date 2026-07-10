//! Built-in sample plugin packs users can export for custom development.

use serde::Serialize;
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SamplePack {
    pub id: String,
    pub name: String,
    pub runtime: String,
    pub description: String,
    pub files: Vec<String>,
}

struct SampleFile {
    relative: &'static str,
    content: &'static str,
}

struct SampleDef {
    id: &'static str,
    name: &'static str,
    runtime: &'static str,
    description: &'static str,
    files: &'static [SampleFile],
}

const SAMPLES: &[SampleDef] = &[
    SampleDef {
        id: "node-echo",
        name: "Node 回显示例",
        runtime: "node",
        description: "Node.js：stdin 读 JSON，stdout 回写结果。需安装 node。",
        files: &[
            SampleFile {
                relative: "plugin.json",
                content: include_str!("../../../plugin-examples/node-echo/plugin.json"),
            },
            SampleFile {
                relative: "main.js",
                content: include_str!("../../../plugin-examples/node-echo/main.js"),
            },
        ],
    },
    SampleDef {
        id: "python-upper",
        name: "Python 大写示例",
        runtime: "python",
        description: "Python3：将文本转为大写。需安装 python3。",
        files: &[
            SampleFile {
                relative: "plugin.json",
                content: include_str!("../../../plugin-examples/python-upper/plugin.json"),
            },
            SampleFile {
                relative: "main.py",
                content: include_str!("../../../plugin-examples/python-upper/main.py"),
            },
        ],
    },
    SampleDef {
        id: "go-echo",
        name: "Go 回显示例",
        runtime: "go",
        description: "Go：go run main.go 处理 stdin JSON。需安装 Go 工具链。",
        files: &[
            SampleFile {
                relative: "plugin.json",
                content: include_str!("../../../plugin-examples/go-echo/plugin.json"),
            },
            SampleFile {
                relative: "main.go",
                content: include_str!("../../../plugin-examples/go-echo/main.go"),
            },
        ],
    },
    SampleDef {
        id: "shell-wc",
        name: "Shell 字数统计",
        runtime: "shell",
        description: "Shell + python3：统计字符/行数。macOS/Linux 可用。",
        files: &[
            SampleFile {
                relative: "plugin.json",
                content: include_str!("../../../plugin-examples/shell-wc/plugin.json"),
            },
            SampleFile {
                relative: "main.sh",
                content: include_str!("../../../plugin-examples/shell-wc/main.sh"),
            },
        ],
    },
];

pub fn list_sample_packs() -> Vec<SamplePack> {
    SAMPLES
        .iter()
        .map(|s| SamplePack {
            id: s.id.into(),
            name: s.name.into(),
            runtime: s.runtime.into(),
            description: s.description.into(),
            files: s.files.iter().map(|f| f.relative.into()).collect(),
        })
        .collect()
}

/// Export one sample pack into `dest_parent/sample_id/`.
pub fn export_sample_pack(sample_id: &str, dest_parent: &Path) -> Result<String, String> {
    let def = SAMPLES
        .iter()
        .find(|s| s.id == sample_id)
        .ok_or_else(|| format!("未知示例: {sample_id}"))?;

    let dir = dest_parent.join(def.id);
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    for f in def.files {
        let path = dir.join(f.relative);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        fs::write(&path, f.content).map_err(|e| format!("写入 {}: {e}", path.display()))?;
        // Make shell scripts executable on Unix
        #[cfg(unix)]
        if f.relative.ends_with(".sh") {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = fs::metadata(&path).map_err(|e| e.to_string())?.permissions();
            perms.set_mode(0o755);
            let _ = fs::set_permissions(&path, perms);
        }
    }
    Ok(dir.display().to_string())
}

/// Export all sample packs into dest_parent.
pub fn export_all_samples(dest_parent: &Path) -> Result<Vec<String>, String> {
    let mut paths = Vec::new();
    for s in SAMPLES {
        paths.push(export_sample_pack(s.id, dest_parent)?);
    }
    Ok(paths)
}
