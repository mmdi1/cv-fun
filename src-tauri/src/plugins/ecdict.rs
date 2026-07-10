//! ECDICT resource download + SQLite index.
//! Source: https://github.com/skywind3000/ECDICT

use rusqlite::{params, Connection};
use serde::Serialize;
use std::fs;
use std::io::{BufReader, Read, Write};
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Emitter};

const ECDICT_CSV_URL: &str =
    "https://raw.githubusercontent.com/skywind3000/ECDICT/master/ecdict.csv";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EcdictStatus {
    pub ready: bool,
    pub path: String,
    pub entries: i64,
    pub size_bytes: u64,
}

pub fn ecdict_dir(data_root: &Path) -> PathBuf {
    data_root.join("plugins").join("data").join("ecdict")
}

pub fn ecdict_db_path(data_root: &Path) -> PathBuf {
    ecdict_dir(data_root).join("ecdict.db")
}

pub fn ecdict_ready(data_root: &Path) -> EcdictStatus {
    let path = ecdict_db_path(data_root);
    if !path.exists() {
        return EcdictStatus {
            ready: false,
            path: path.display().to_string(),
            entries: 0,
            size_bytes: 0,
        };
    }
    let size_bytes = fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
    let entries = Connection::open(&path)
        .ok()
        .and_then(|c| {
            c.query_row("SELECT COUNT(*) FROM entries", [], |r| r.get::<_, i64>(0))
                .ok()
        })
        .unwrap_or(0);
    EcdictStatus {
        ready: entries > 0,
        path: path.display().to_string(),
        entries,
        size_bytes,
    }
}

/// Download ECDICT CSV and import into local SQLite (word → translation).
pub fn install_ecdict(app: &AppHandle, data_root: &Path) -> Result<EcdictStatus, String> {
    let dir = ecdict_dir(data_root);
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let csv_path = dir.join("ecdict.csv");
    let db_path = ecdict_db_path(data_root);

    let _ = app.emit(
        "ecdict-progress",
        serde_json::json!({ "phase": "download", "message": "开始下载 ECDICT…", "percent": 0 }),
    );

    download_file(app, ECDICT_CSV_URL, &csv_path)?;

    let _ = app.emit(
        "ecdict-progress",
        serde_json::json!({ "phase": "import", "message": "导入词典到 SQLite…", "percent": 0 }),
    );

    import_csv_to_sqlite(app, &csv_path, &db_path)?;

    // CSV is huge; keep optional — remove to save space after import
    let _ = fs::remove_file(&csv_path);

    let status = ecdict_ready(data_root);
    let _ = app.emit(
        "ecdict-progress",
        serde_json::json!({
            "phase": "done",
            "message": format!("完成 · {} 词条", status.entries),
            "percent": 100,
        }),
    );
    Ok(status)
}

fn download_file(app: &AppHandle, url: &str, dest: &Path) -> Result<(), String> {
    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(600))
        .user_agent("FunCV/0.1 (ECDICT installer)")
        .build()
        .map_err(|e| e.to_string())?;

    let mut resp = client
        .get(url)
        .send()
        .map_err(|e| format!("下载失败: {e}"))?
        .error_for_status()
        .map_err(|e| format!("下载 HTTP 错误: {e}"))?;

    let total = resp.content_length().unwrap_or(0);
    let mut file = fs::File::create(dest).map_err(|e| e.to_string())?;
    let mut downloaded: u64 = 0;
    let mut last_emit = 0u64;

    let mut buf = [0u8; 64 * 1024];
    loop {
        let n = resp.read(&mut buf).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        file.write_all(&buf[..n]).map_err(|e| e.to_string())?;
        downloaded += n as u64;
        if downloaded - last_emit > 2 * 1024 * 1024 || (total > 0 && downloaded == total) {
            last_emit = downloaded;
            let percent = if total > 0 {
                ((downloaded as f64 / total as f64) * 100.0).min(99.0) as u32
            } else {
                0
            };
            let _ = app.emit(
                "ecdict-progress",
                serde_json::json!({
                    "phase": "download",
                    "message": format!("下载中 {:.1} MB…", downloaded as f64 / 1_048_576.0),
                    "percent": percent,
                    "downloaded": downloaded,
                    "total": total,
                }),
            );
        }
    }
    file.flush().map_err(|e| e.to_string())?;
    Ok(())
}

fn import_csv_to_sqlite(app: &AppHandle, csv_path: &Path, db_path: &Path) -> Result<(), String> {
    if db_path.exists() {
        fs::remove_file(db_path).map_err(|e| e.to_string())?;
    }

    let mut conn = Connection::open(db_path).map_err(|e| e.to_string())?;
    conn.execute_batch(
        "PRAGMA journal_mode=WAL;
         PRAGMA synchronous=OFF;
         CREATE TABLE entries (
           word TEXT PRIMARY KEY COLLATE NOCASE,
           phonetic TEXT,
           translation TEXT,
           definition TEXT
         );
         CREATE INDEX idx_entries_word ON entries(word);",
    )
    .map_err(|e| e.to_string())?;

    let file = fs::File::open(csv_path).map_err(|e| e.to_string())?;
    let reader = BufReader::with_capacity(1024 * 1024, file);
    let mut rdr = csv::ReaderBuilder::new()
        .flexible(true)
        .from_reader(reader);

    let tx = conn.transaction().map_err(|e| e.to_string())?;
    {
        let mut stmt = tx
            .prepare(
                "INSERT OR REPLACE INTO entries(word, phonetic, translation, definition)
                 VALUES (?1, ?2, ?3, ?4)",
            )
            .map_err(|e| e.to_string())?;

        let mut count: u64 = 0;
        for result in rdr.records() {
            let record = match result {
                Ok(r) => r,
                Err(_) => continue,
            };
            // word,phonetic,definition,translation,pos,...
            let word = record.get(0).unwrap_or("").trim();
            if word.is_empty() {
                continue;
            }
            let phonetic = record.get(1).unwrap_or("").trim();
            let definition = record.get(2).unwrap_or("").trim();
            let translation = record.get(3).unwrap_or("").trim();
            if translation.is_empty() && definition.is_empty() {
                continue;
            }
            let _ = stmt.execute(params![word, phonetic, translation, definition]);
            count += 1;
            if count % 50_000 == 0 {
                let _ = app.emit(
                    "ecdict-progress",
                    serde_json::json!({
                        "phase": "import",
                        "message": format!("已导入 {count} 词条…"),
                        "percent": 0,
                    }),
                );
            }
        }
    }
    tx.commit().map_err(|e| e.to_string())?;
    conn.execute_batch("PRAGMA optimize;").ok();
    Ok(())
}

pub fn lookup_word(data_root: &Path, word: &str) -> Result<Option<(String, String, String)>, String> {
    let db = ecdict_db_path(data_root);
    if !db.exists() {
        return Err("词典未安装，请先在插件面板下载 ECDICT".into());
    }
    let conn = Connection::open(&db).map_err(|e| e.to_string())?;
    let key = word.trim();
    // Exact then lowercase
    let row = conn
        .query_row(
            "SELECT word, phonetic, translation FROM entries WHERE word = ?1 COLLATE NOCASE LIMIT 1",
            params![key],
            |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                ))
            },
        )
        .ok();
    Ok(row)
}

/// Reverse-ish search: Chinese fragment in translation field.
pub fn search_by_chinese(
    data_root: &Path,
    query: &str,
    limit: usize,
) -> Result<Vec<(String, String, String)>, String> {
    let db = ecdict_db_path(data_root);
    if !db.exists() {
        return Err("词典未安装，请先在插件面板下载 ECDICT".into());
    }
    let conn = Connection::open(&db).map_err(|e| e.to_string())?;
    let q = format!("%{}%", query.trim());
    let mut stmt = conn
        .prepare(
            "SELECT word, phonetic, translation FROM entries
             WHERE translation LIKE ?1
             LIMIT ?2",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map(params![q, limit as i64], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
            ))
        })
        .map_err(|e| e.to_string())?;
    let mut out = Vec::new();
    for row in rows {
        if let Ok(v) = row {
            out.push(v);
        }
    }
    Ok(out)
}
