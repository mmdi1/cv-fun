use super::ecdict::{lookup_word, search_by_chinese};
use super::runner::{PluginInput, PluginOutput};
use std::path::Path;

/// Builtin EN↔ZH using local ECDICT.
pub fn run_builtin_translate(
    data_root: &Path,
    input: &PluginInput,
) -> Result<PluginOutput, String> {
    if input.r#type != "text" {
        return Ok(PluginOutput::fail(
            "translate-en-zh",
            "仅支持 type=text",
        ));
    }
    let text = input.content.trim();
    if text.is_empty() {
        return Ok(PluginOutput::fail("translate-en-zh", "内容为空"));
    }
    // Too long for dictionary lookup
    if text.chars().count() > 200 {
        return Ok(PluginOutput::fail(
            "translate-en-zh",
            "内容过长，词典适合单词/短语",
        ));
    }

    if is_mostly_ascii_word(text) {
        // EN → ZH
        let word = text.trim_matches(|c: char| !c.is_alphanumeric() && c != '-' && c != '\'');
        if word.is_empty() {
            return Ok(PluginOutput::fail("translate-en-zh", "未识别到英文词"));
        }
        match lookup_word(data_root, word)? {
            Some((w, phonetic, translation)) => {
                let mut body = format!("{w}");
                if !phonetic.is_empty() {
                    body.push_str(&format!("\n/{phonetic}/"));
                }
                if !translation.is_empty() {
                    body.push_str(&format!("\n{translation}"));
                }
                // Clean ECDICT \\n escapes
                let body = body.replace("\\n", "\n");
                let preview = translation.replace('\\', " ").replace('\n', " ");
                let preview = if preview.chars().count() > 72 {
                    format!("{}…", preview.chars().take(72).collect::<String>())
                } else {
                    preview
                };
                Ok(PluginOutput {
                    ok: true,
                    title: "英→汉".into(),
                    body,
                    preview,
                    hint: "ECDICT 本地".into(),
                    error: String::new(),
                    plugin_id: "translate-en-zh".into(),
                })
            }
            None => Ok(PluginOutput::fail(
                "translate-en-zh",
                format!("未找到词条: {word}"),
            )),
        }
    } else if contains_cjk(text) {
        // ZH → EN candidates
        let hits = search_by_chinese(data_root, text, 12)?;
        if hits.is_empty() {
            return Ok(PluginOutput::fail(
                "translate-en-zh",
                format!("未找到含「{text}」的英文词条"),
            ));
        }
        let mut lines = Vec::new();
        for (w, phonetic, translation) in &hits {
            if phonetic.is_empty() {
                lines.push(format!("{w}\n  {}", translation.replace("\\n", "; ")));
            } else {
                lines.push(format!(
                    "{w} /{phonetic}/\n  {}",
                    translation.replace("\\n", "; ")
                ));
            }
        }
        let body = lines.join("\n\n");
        let preview = hits
            .iter()
            .map(|(w, _, _)| w.as_str())
            .take(5)
            .collect::<Vec<_>>()
            .join(", ");
        Ok(PluginOutput {
            ok: true,
            title: "汉→英".into(),
            body,
            preview,
            hint: format!("ECDICT · {} 条", hits.len()),
            error: String::new(),
            plugin_id: "translate-en-zh".into(),
        })
    } else {
        Ok(PluginOutput::fail(
            "translate-en-zh",
            "请输入英文单词/短语或中文关键词",
        ))
    }
}

fn is_mostly_ascii_word(s: &str) -> bool {
    let t = s.trim();
    if t.is_empty() {
        return false;
    }
    // Allow multi-word English phrases up to ~6 words
    let words: Vec<_> = t.split_whitespace().collect();
    if words.is_empty() || words.len() > 6 {
        return false;
    }
    words.iter().all(|w| {
        w.chars()
            .all(|c| c.is_ascii_alphabetic() || c == '-' || c == '\'')
    })
}

fn contains_cjk(s: &str) -> bool {
    s.chars().any(|c| {
        let u = c as u32;
        (0x4E00..=0x9FFF).contains(&u)
            || (0x3400..=0x4DBF).contains(&u)
            || (0xF900..=0xFAFF).contains(&u)
    })
}
