//! Builtin image OCR plugin (`image-ocr`).
//! macOS: Vision · Windows: Media.Ocr · other: unsupported.

use super::runner::{PluginInput, PluginOutput};
use crate::ocr::ocr_image_file;
use std::path::{Path, PathBuf};

pub fn run_builtin_image_ocr(
    data_root: &Path,
    input: &PluginInput,
) -> Result<PluginOutput, String> {
    if input.r#type != "img" && input.r#type != "image" {
        return Ok(PluginOutput::fail(
            "image-ocr",
            "仅支持 type=img（图片）",
        ));
    }

    let path = resolve_image_content_path(data_root, &input.content)?;
    match ocr_image_file(&path) {
        Ok(text) => {
            let preview = {
                let one = text.replace('\n', " ");
                if one.chars().count() > 72 {
                    format!("{}…", one.chars().take(72).collect::<String>())
                } else {
                    one
                }
            };
            Ok(PluginOutput {
                ok: true,
                title: "图片文字".into(),
                body: text,
                preview,
                hint: ocr_engine_hint(),
                error: String::new(),
                plugin_id: "image-ocr".into(),
            })
        }
        Err(err) => Ok(PluginOutput::fail("image-ocr", err)),
    }
}

fn ocr_engine_hint() -> String {
    #[cfg(target_os = "macos")]
    {
        "Vision 本地".into()
    }
    #[cfg(target_os = "windows")]
    {
        "Windows OCR".into()
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        "OCR".into()
    }
}

/// `content` may be absolute path, relative under data root, or images/<id>.png style.
fn resolve_image_content_path(data_root: &Path, content: &str) -> Result<PathBuf, String> {
    let c = content.trim();
    if c.is_empty() {
        return Err("图片路径为空".into());
    }
    let p = PathBuf::from(c);
    if p.is_absolute() && p.exists() {
        return Ok(p);
    }
    let under_root = data_root.join(c);
    if under_root.exists() {
        return Ok(under_root);
    }
    // history stores relative image_path like images/<uuid>.png
    let under_images = data_root.join("images").join(c);
    if under_images.exists() {
        return Ok(under_images);
    }
    Err(format!("找不到图片文件: {c}"))
}
