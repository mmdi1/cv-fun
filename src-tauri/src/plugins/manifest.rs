use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Supported external / builtin runtimes.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum PluginRuntime {
    Builtin,
    Node,
    Python,
    Go,
    Shell,
}

impl PluginRuntime {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Builtin => "builtin",
            Self::Node => "node",
            Self::Python => "python",
            Self::Go => "go",
            Self::Shell => "shell",
        }
    }
}

/// `plugin.json` schema for a plugin package.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginManifest {
    pub id: String,
    pub name: String,
    #[serde(default = "default_version")]
    pub version: String,
    pub runtime: PluginRuntime,
    /// Entry file relative to plugin dir (ignored for builtin).
    #[serde(default)]
    pub entry: String,
    #[serde(default)]
    pub description: String,
    /// Content types this plugin accepts: "text", "img", …
    #[serde(default = "default_types")]
    pub types: Vec<String>,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub builtin: bool,
}

fn default_version() -> String {
    "1.0.0".into()
}
fn default_types() -> Vec<String> {
    vec!["text".into()]
}
fn default_true() -> bool {
    true
}

/// UI-facing plugin descriptor.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginInfo {
    pub id: String,
    pub name: String,
    pub version: String,
    pub runtime: String,
    pub description: String,
    pub types: Vec<String>,
    pub enabled: bool,
    pub builtin: bool,
    pub path: Option<String>,
}

impl PluginInfo {
    pub fn from_manifest(m: &PluginManifest, dir: Option<&PathBuf>) -> Self {
        Self {
            id: m.id.clone(),
            name: m.name.clone(),
            version: m.version.clone(),
            runtime: m.runtime.as_str().into(),
            description: m.description.clone(),
            types: m.types.clone(),
            enabled: m.enabled,
            builtin: m.builtin,
            path: dir.map(|p| p.display().to_string()),
        }
    }
}

/// Builtin plugin definitions (no files required).
pub fn builtin_manifests() -> Vec<PluginManifest> {
    vec![
        PluginManifest {
            id: "translate-en-zh".into(),
            name: "英汉互译".into(),
            version: "1.0.0".into(),
            runtime: PluginRuntime::Builtin,
            entry: String::new(),
            description: "本地 ECDICT 词典：英文查中文，中文反查英文词条。需先下载词典资源。"
                .into(),
            types: vec!["text".into()],
            enabled: true,
            builtin: true,
        },
        PluginManifest {
            id: "image-ocr".into(),
            name: "图片文字识别".into(),
            version: "1.0.0".into(),
            runtime: PluginRuntime::Builtin,
            entry: String::new(),
            description: image_ocr_description().into(),
            types: vec!["img".into()],
            // Default on: image panel button + optional plugin suggestions
            enabled: true,
            builtin: true,
        },
    ]
}

fn image_ocr_description() -> &'static str {
    #[cfg(target_os = "macos")]
    {
        "本地 OCR：macOS 系统 Vision 框架，无需联网/下载模型。图片详情点「识别文字」。"
    }
    #[cfg(target_os = "windows")]
    {
        "本地 OCR：Windows.Media.Ocr（系统能力）。图片详情点「识别文字」。可在插件列表关闭。"
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        "图片文字识别（当前平台需自行扩展）。"
    }
}
