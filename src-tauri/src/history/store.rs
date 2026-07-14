use chrono::{DateTime, Datelike, Local, Timelike, Utc};
use parking_lot::Mutex;
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};
use thiserror::Error;
use uuid::Uuid;

const DEFAULT_MAX_ITEMS: usize = 500;

#[derive(Debug, Error)]
pub enum HistoryError {
    #[error("{0}")]
    Message(String),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Sqlite(#[from] rusqlite::Error),
}

pub type HistoryResult<T> = Result<T, HistoryError>;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ItemKind {
    Text,
    Image,
}

impl ItemKind {
    fn as_str(self) -> &'static str {
        match self {
            Self::Text => "text",
            Self::Image => "image",
        }
    }

    fn parse(value: &str) -> Self {
        if value == "image" {
            Self::Image
        } else {
            Self::Text
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryItem {
    pub id: String,
    pub kind: ItemKind,
    pub text: Option<String>,
    pub preview: String,
    pub hash: String,
    pub image_path: Option<String>,
    pub image_width: Option<u32>,
    pub image_height: Option<u32>,
    pub copied_at: DateTime<Utc>,
    pub last_used_at: DateTime<Utc>,
    pub use_count: i64,
    pub pinned: bool,
    /// User favorite / bookmark (independent of pin).
    #[serde(default)]
    pub favorited: bool,
    /// Optional note (esp. for favorites); included in search.
    #[serde(default)]
    pub note: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TopRepeat {
    pub hash: String,
    pub preview: String,
    pub kind: String,
    pub count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DailyStat {
    pub day: String,
    pub total: i64,
    pub duplicates: i64,
    pub text: i64,
    pub image: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StatsSnapshot {
    pub total_copies: i64,
    pub total_duplicates: i64,
    pub text_copies: i64,
    pub image_copies: i64,
    pub today_copies: i64,
    pub today_duplicates: i64,
    pub history_items: i64,
    /// 0.0 ~ 1.0
    pub duplicate_rate: f64,
    pub peak_hour: Option<i64>,
    pub peak_hour_count: i64,
    pub streak_days: i64,
    pub top_repeats: Vec<TopRepeat>,
    pub daily: Vec<DailyStat>,
}

/// Result of adding clipboard content (includes duplicate flag for stats).
#[derive(Debug, Clone)]
pub struct AddResult {
    pub item: HistoryItem,
    pub is_duplicate: bool,
    pub is_consecutive: bool,
}

pub struct HistoryStore {
    root: PathBuf,
    images_dir: PathBuf,
    max_items: usize,
    /// Long-lived connection — avoids open/close on every list/get click.
    conn: Mutex<Connection>,
}

impl HistoryStore {
    pub fn open(root: impl Into<PathBuf>, max_items: usize) -> HistoryResult<Self> {
        let root = root.into();
        let images_dir = root.join("images");
        let db_path = root.join("history.db");
        fs::create_dir_all(&images_dir)?;

        let conn = Connection::open(&db_path)?;
        // Keep first-open PRAGMAs minimal — WAL mode change can be slow on cold start.
        conn.execute_batch(
            "PRAGMA synchronous=NORMAL;
             PRAGMA foreign_keys=ON;
             PRAGMA temp_store=MEMORY;
             PRAGMA cache_size=-8000;",
        )?;

        let store = Self {
            root,
            images_dir,
            max_items: if max_items == 0 {
                DEFAULT_MAX_ITEMS
            } else {
                max_items
            },
            conn: Mutex::new(conn),
        };
        store.init_schema()?;
        // WAL after schema so UI can appear sooner; ignore if already set.
        {
            let c = store.conn.lock();
            let _ = c.execute_batch("PRAGMA journal_mode=WAL;");
        }
        Ok(store)
    }

    pub fn data_root() -> HistoryResult<PathBuf> {
        let config = dirs::config_dir()
            .ok_or_else(|| HistoryError::Message("resolve user config dir failed".into()))?;
        Ok(config.join("NfunCv"))
    }

    pub fn set_max_items(&mut self, max_items: usize) {
        self.max_items = if max_items == 0 {
            DEFAULT_MAX_ITEMS
        } else {
            max_items
        };
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    fn init_schema(&self) -> HistoryResult<()> {
        let conn = self.conn.lock();
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS history (
                id TEXT PRIMARY KEY NOT NULL,
                kind TEXT NOT NULL,
                text TEXT,
                preview TEXT NOT NULL,
                hash TEXT NOT NULL UNIQUE,
                image_path TEXT,
                image_width INTEGER,
                image_height INTEGER,
                copied_at TEXT NOT NULL,
                last_used_at TEXT NOT NULL,
                use_count INTEGER NOT NULL DEFAULT 1,
                pinned INTEGER NOT NULL DEFAULT 0,
                favorited INTEGER NOT NULL DEFAULT 0,
                note TEXT
            );
            CREATE INDEX IF NOT EXISTS idx_history_copied_at ON history(copied_at DESC);
            CREATE INDEX IF NOT EXISTS idx_history_hash ON history(hash);

            CREATE TABLE IF NOT EXISTS clipboard_events (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                ts TEXT NOT NULL,
                day TEXT NOT NULL,
                hour INTEGER NOT NULL,
                kind TEXT NOT NULL,
                hash TEXT NOT NULL,
                history_id TEXT,
                preview TEXT,
                is_duplicate INTEGER NOT NULL DEFAULT 0,
                is_consecutive INTEGER NOT NULL DEFAULT 0,
                byte_len INTEGER NOT NULL DEFAULT 0
            );
            CREATE INDEX IF NOT EXISTS idx_events_ts ON clipboard_events(ts DESC);
            CREATE INDEX IF NOT EXISTS idx_events_day ON clipboard_events(day);
            CREATE INDEX IF NOT EXISTS idx_events_hash ON clipboard_events(hash);

            CREATE TABLE IF NOT EXISTS stats_global (
                key TEXT PRIMARY KEY,
                value INTEGER NOT NULL DEFAULT 0
            );
            INSERT OR IGNORE INTO stats_global(key, value) VALUES
                ('total_copies', 0),
                ('total_duplicates', 0),
                ('text_copies', 0),
                ('image_copies', 0);

            CREATE TABLE IF NOT EXISTS stats_daily (
                day TEXT PRIMARY KEY,
                total INTEGER NOT NULL DEFAULT 0,
                duplicates INTEGER NOT NULL DEFAULT 0,
                text INTEGER NOT NULL DEFAULT 0,
                image INTEGER NOT NULL DEFAULT 0
            );",
        )?;
        // Migrations for existing installs
        let _ = conn.execute(
            "ALTER TABLE history ADD COLUMN favorited INTEGER NOT NULL DEFAULT 0",
            [],
        );
        let _ = conn.execute("ALTER TABLE history ADD COLUMN note TEXT", []);
        Ok(())
    }

    pub fn add_text(&self, text: &str) -> HistoryResult<AddResult> {
        let text = text.trim();
        if text.is_empty() {
            return Err(HistoryError::Message("clipboard text is empty".into()));
        }
        let hash = hash_bytes(text.as_bytes());
        let preview = preview_text(text);
        let byte_len = text.len() as i64;
        self.upsert_and_track(
            ItemKind::Text,
            Some(text.to_string()),
            preview,
            hash,
            None,
            None,
            None,
            byte_len,
        )
    }

    pub fn add_image(&self, rgba: &[u8], width: u32, height: u32) -> HistoryResult<AddResult> {
        if width == 0 || height == 0 || rgba.is_empty() {
            return Err(HistoryError::Message("clipboard image is empty".into()));
        }
        let hash = hash_bytes(rgba);
        let byte_len = rgba.len() as i64;
        let conn = self.conn.lock();

        let consecutive = last_event_hash(&conn)? == Some(hash.clone());
        if let Some(existing) = find_by_hash(&conn, &hash)? {
            let item = touch_existing(&conn, existing)?;
            record_event(
                &conn,
                &item,
                true,
                consecutive,
                byte_len,
            )?;
            return Ok(AddResult {
                item,
                is_duplicate: true,
                is_consecutive: consecutive,
            });
        }

        drop(conn); // release before heavy image encode/save

        let id = Uuid::new_v4().simple().to_string();
        let rel = format!("images/{id}.png");
        let abs = self.images_dir.join(format!("{id}.png"));
        let img = image::RgbaImage::from_raw(width, height, rgba.to_vec()).ok_or_else(|| {
            HistoryError::Message("invalid image buffer dimensions".into())
        })?;
        // Ensure dir still exists (user may have cleaned it)
        fs::create_dir_all(&self.images_dir)?;
        img.save(&abs)
            .map_err(|e| HistoryError::Message(format!("save image: {e}")))?;

        let preview = format!("图片 {width}×{height}");
        let conn = self.conn.lock();
        // Re-check hash in case of race
        if let Some(existing) = find_by_hash(&conn, &hash)? {
            let item = touch_existing(&conn, existing)?;
            let consecutive = last_event_hash(&conn)? == Some(hash.clone());
            record_event(&conn, &item, true, consecutive, byte_len)?;
            let _ = fs::remove_file(&abs);
            return Ok(AddResult {
                item,
                is_duplicate: true,
                is_consecutive: consecutive,
            });
        }

        let item = insert_new(
            &conn,
            &self.root,
            self.max_items,
            id,
            ItemKind::Image,
            None,
            preview,
            hash,
            Some(rel),
            Some(width),
            Some(height),
        )?;
        record_event(&conn, &item, false, false, byte_len)?;
        Ok(AddResult {
            item,
            is_duplicate: false,
            is_consecutive: false,
        })
    }

    fn upsert_and_track(
        &self,
        kind: ItemKind,
        text: Option<String>,
        preview: String,
        hash: String,
        image_path: Option<String>,
        image_width: Option<u32>,
        image_height: Option<u32>,
        byte_len: i64,
    ) -> HistoryResult<AddResult> {
        let conn = self.conn.lock();
        let consecutive = last_event_hash(&conn)? == Some(hash.clone());
        if let Some(existing) = find_by_hash(&conn, &hash)? {
            let item = touch_existing(&conn, existing)?;
            record_event(&conn, &item, true, consecutive, byte_len)?;
            return Ok(AddResult {
                item,
                is_duplicate: true,
                is_consecutive: consecutive,
            });
        }
        let id = Uuid::new_v4().simple().to_string();
        let item = insert_new(
            &conn,
            &self.root,
            self.max_items,
            id,
            kind,
            text,
            preview,
            hash,
            image_path,
            image_width,
            image_height,
        )?;
        record_event(&conn, &item, false, false, byte_len)?;
        Ok(AddResult {
            item,
            is_duplicate: false,
            is_consecutive: false,
        })
    }

    pub fn list(&self, query: &str) -> HistoryResult<Vec<HistoryItem>> {
        let conn = self.conn.lock();
        let q = query.trim();
        let mut items = Vec::with_capacity(64);

        if q.is_empty() {
            let mut stmt = conn.prepare(
                "SELECT id, kind, NULL, preview, hash, image_path, image_width, image_height,
                        copied_at, last_used_at, use_count, pinned, favorited, note
                 FROM history
                 ORDER BY copied_at DESC
                 LIMIT 500",
            )?;
            for row in stmt.query_map([], |row| map_row(row))? {
                items.push(row?);
            }
        } else {
            let pattern = format!("%{}%", escape_like(q));
            let mut stmt = conn.prepare(
                "SELECT id, kind, NULL, preview, hash, image_path, image_width, image_height,
                        copied_at, last_used_at, use_count, pinned, favorited, note
                 FROM history
                 WHERE preview LIKE ?1 ESCAPE '\\'
                    OR IFNULL(text, '') LIKE ?1 ESCAPE '\\'
                    OR IFNULL(note, '') LIKE ?1 ESCAPE '\\'
                 ORDER BY copied_at DESC
                 LIMIT 500",
            )?;
            for row in stmt.query_map(params![pattern], |row| map_row(row))? {
                items.push(row?);
            }
        }
        Ok(items)
    }

    pub fn get(&self, id: &str) -> HistoryResult<HistoryItem> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT id, kind, text, preview, hash, image_path, image_width, image_height,
                    copied_at, last_used_at, use_count, pinned, favorited, note
             FROM history WHERE id = ?1",
        )?;
        stmt.query_row(params![id], |row| map_row(row))
            .map_err(|_| HistoryError::Message(format!("history item {id} not found")))
    }

    pub fn set_pinned(&self, id: &str, pinned: bool) -> HistoryResult<HistoryItem> {
        {
            let conn = self.conn.lock();
            let n = conn.execute(
                "UPDATE history SET pinned = ?1 WHERE id = ?2",
                params![if pinned { 1 } else { 0 }, id],
            )?;
            if n == 0 {
                return Err(HistoryError::Message(format!("history item {id} not found")));
            }
        }
        self.get(id)
    }

    pub fn set_favorited(&self, id: &str, favorited: bool) -> HistoryResult<HistoryItem> {
        {
            let conn = self.conn.lock();
            let n = conn.execute(
                "UPDATE history SET favorited = ?1 WHERE id = ?2",
                params![if favorited { 1 } else { 0 }, id],
            )?;
            if n == 0 {
                return Err(HistoryError::Message(format!("history item {id} not found")));
            }
        }
        self.get(id)
    }

    /// Set free-form note (trimmed; empty clears). Used for favorite search tags.
    pub fn set_note(&self, id: &str, note: &str) -> HistoryResult<HistoryItem> {
        let note = note.trim();
        let note_db: Option<&str> = if note.is_empty() { None } else { Some(note) };
        {
            let conn = self.conn.lock();
            let n = conn.execute(
                "UPDATE history SET note = ?1 WHERE id = ?2",
                params![note_db, id],
            )?;
            if n == 0 {
                return Err(HistoryError::Message(format!("history item {id} not found")));
            }
        }
        self.get(id)
    }

    pub fn delete(&self, id: &str) -> HistoryResult<()> {
        let item = self.get(id)?;
        let conn = self.conn.lock();
        conn.execute("DELETE FROM history WHERE id = ?1", params![id])?;
        if let Some(rel) = item.image_path {
            let _ = fs::remove_file(self.root.join(rel));
        }
        Ok(())
    }

    /// Clear history by mode. Returns number of deleted rows.
    /// - `keep_saved`: delete only items that are neither pinned nor favorited
    /// - `keep_favorites`: delete non-favorited (pinned-only still deleted)
    /// - `all`: delete everything
    pub fn clear(&self, mode: &str) -> HistoryResult<usize> {
        let mode = mode.trim().to_ascii_lowercase();
        let where_sql = match mode.as_str() {
            "keep_saved" | "unprotected" | "normal" => "pinned = 0 AND favorited = 0",
            "keep_favorites" | "non_favorites" => "favorited = 0",
            "all" => "1 = 1",
            other => {
                return Err(HistoryError::Message(format!(
                    "unknown clear mode: {other}"
                )));
            }
        };

        let conn = self.conn.lock();
        // Collect image files to remove before delete
        let sql_sel = format!("SELECT id, image_path FROM history WHERE {where_sql}");
        let mut stmt = conn.prepare(&sql_sel)?;
        let doomed: Vec<(String, Option<String>)> = stmt
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
            .filter_map(|r| r.ok())
            .collect();
        drop(stmt);

        let n = doomed.len();
        if n == 0 {
            return Ok(0);
        }

        let sql_del = format!("DELETE FROM history WHERE {where_sql}");
        conn.execute(&sql_del, [])?;
        drop(conn);

        for (_id, image_path) in doomed {
            if let Some(rel) = image_path {
                let _ = fs::remove_file(self.root.join(rel));
            }
        }
        Ok(n)
    }

    pub fn mark_used(&self, id: &str) -> HistoryResult<()> {
        let conn = self.conn.lock();
        let now = Utc::now().to_rfc3339();
        conn.execute(
            "UPDATE history SET last_used_at=?1, use_count=use_count+1 WHERE id=?2",
            params![now, id],
        )?;
        Ok(())
    }

    /// List items that are pinned and/or favorited (full text included).
    pub fn list_saved(&self) -> HistoryResult<Vec<HistoryItem>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT id, kind, text, preview, hash, image_path, image_width, image_height,
                    copied_at, last_used_at, use_count, pinned, favorited, note
             FROM history
             WHERE pinned = 1 OR favorited = 1
             ORDER BY pinned DESC, favorited DESC, copied_at DESC",
        )?;
        let mut items = Vec::new();
        for row in stmt.query_map([], |row| map_row(row))? {
            items.push(row?);
        }
        Ok(items)
    }

    /**
     * Export pinned/favorited items as a portable package under `dest_dir`:
     *   FunCV-saved-YYYYMMDD-HHMMSS/
     *     export.json
     *     images/… (optional)
     * Returns the created folder path and item count.
     */
    pub fn export_saved(
        &self,
        dest_parent: &Path,
        include_images: bool,
    ) -> HistoryResult<(PathBuf, usize)> {
        let items = self.list_saved()?;
        if items.is_empty() {
            return Err(HistoryError::Message("没有可导出的钉住/收藏记录".into()));
        }

        let stamp = Local::now().format("%Y%m%d-%H%M%S");
        let out_dir = dest_parent.join(format!("FunCV-saved-{stamp}"));
        fs::create_dir_all(&out_dir)?;
        let images_out = out_dir.join("images");
        if include_images {
            fs::create_dir_all(&images_out)?;
        }

        let mut export_items: Vec<serde_json::Value> = Vec::with_capacity(items.len());
        for it in &items {
            let mut image_file: Option<String> = None;
            if include_images {
                if let Some(rel) = it.image_path.as_ref() {
                    let src = self.root.join(rel);
                    if src.exists() {
                        let name = Path::new(rel)
                            .file_name()
                            .and_then(|s| s.to_str())
                            .unwrap_or("image.png");
                        let dest = images_out.join(name);
                        // Avoid overwrite collisions
                        let dest = if dest.exists() {
                            images_out.join(format!("{}-{}", &it.id[..8.min(it.id.len())], name))
                        } else {
                            dest
                        };
                        fs::copy(&src, &dest).map_err(|e| {
                            HistoryError::Message(format!("复制图片失败 {}: {e}", src.display()))
                        })?;
                        image_file = Some(format!(
                            "images/{}",
                            dest.file_name()
                                .and_then(|s| s.to_str())
                                .unwrap_or(name)
                        ));
                    }
                }
            }

            export_items.push(serde_json::json!({
                "id": it.id,
                "kind": it.kind.as_str(),
                "text": it.text,
                "preview": it.preview,
                "hash": it.hash,
                "imageWidth": it.image_width,
                "imageHeight": it.image_height,
                "copiedAt": it.copied_at.to_rfc3339(),
                "lastUsedAt": it.last_used_at.to_rfc3339(),
                "useCount": it.use_count,
                "pinned": it.pinned,
                "favorited": it.favorited,
                "note": it.note,
                "imageFile": image_file,
            }));
        }

        let payload = serde_json::json!({
            "format": "nfun-cv-saved-export",
            "version": 1,
            "exportedAt": Utc::now().to_rfc3339(),
            "includeImages": include_images,
            "count": export_items.len(),
            "items": export_items,
        });
        let json_path = out_dir.join("export.json");
        let json = serde_json::to_string_pretty(&payload)
            .map_err(|e| HistoryError::Message(format!("serialize export: {e}")))?;
        fs::write(&json_path, json)?;

        Ok((out_dir, export_items.len()))
    }

    /**
     * Full backup: checkpoint WAL then copy history.db (+ wal/shm if present),
     * images/, and optionally config.json into a timestamped folder.
     */
    pub fn export_full(
        &self,
        dest_parent: &Path,
        include_config: bool,
    ) -> HistoryResult<(PathBuf, usize)> {
        let stamp = Local::now().format("%Y%m%d-%H%M%S");
        let out_dir = dest_parent.join(format!("FunCV-full-{stamp}"));
        fs::create_dir_all(&out_dir)?;

        // Hold lock and checkpoint so the on-disk main DB is consistent.
        let count = {
            let conn = self.conn.lock();
            let _ = conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);");
            let n: i64 = conn
                .query_row("SELECT COUNT(*) FROM history", [], |r| r.get(0))
                .unwrap_or(0);
            n as usize
        };

        let db_src = self.root.join("history.db");
        if !db_src.exists() {
            return Err(HistoryError::Message("history.db 不存在".into()));
        }
        fs::copy(&db_src, out_dir.join("history.db")).map_err(|e| {
            HistoryError::Message(format!("复制 history.db 失败: {e}"))
        })?;

        for extra in ["history.db-wal", "history.db-shm"] {
            let p = self.root.join(extra);
            if p.exists() {
                let _ = fs::copy(&p, out_dir.join(extra));
            }
        }

        // Copy images directory
        let images_src = self.images_dir.clone();
        let images_dst = out_dir.join("images");
        if images_src.exists() {
            copy_dir_recursive(&images_src, &images_dst)?;
        } else {
            fs::create_dir_all(&images_dst)?;
        }

        if include_config {
            let cfg = self.root.join("config.json");
            if cfg.exists() {
                let _ = fs::copy(&cfg, out_dir.join("config.json"));
            }
        }

        // Small readme for humans
        let readme = format!(
            "FunCV 完整备份\n导出时间: {}\n历史条数: {}\n\n恢复说明:\n1. 退出 FunCV\n2. 将 history.db 与 images/ 复制到应用数据目录（覆盖）\n3. 可选恢复 config.json\n4. 重新打开 FunCV\n",
            Utc::now().to_rfc3339(),
            count
        );
        let _ = fs::write(out_dir.join("README.txt"), readme);

        Ok((out_dir, count))
    }

    /// Import package from `export_saved` (export.json + optional images/).
    /// Merges by content hash: existing rows get pin/fav/note updated; new rows inserted.
    /// Returns (imported_or_updated, skipped).
    pub fn import_saved(&self, package_dir: &Path) -> HistoryResult<(usize, usize)> {
        let json_path = resolve_export_json(package_dir)?;
        let raw = fs::read_to_string(&json_path).map_err(|e| {
            HistoryError::Message(format!("读取 export.json 失败: {e}"))
        })?;
        let root: serde_json::Value = serde_json::from_str(&raw)
            .map_err(|e| HistoryError::Message(format!("解析 export.json 失败: {e}")))?;

        let format = root.get("format").and_then(|v| v.as_str()).unwrap_or("");
        if format != "nfun-cv-saved-export" && root.get("items").is_none() {
            return Err(HistoryError::Message(
                "不是 FunCV 收藏导出包（缺少 export.json 或 format 不匹配）".into(),
            ));
        }

        let items = root
            .get("items")
            .and_then(|v| v.as_array())
            .ok_or_else(|| HistoryError::Message("export.json 缺少 items".into()))?;

        let package_root = json_path
            .parent()
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| package_dir.to_path_buf());

        let mut imported = 0usize;
        let mut skipped = 0usize;
        fs::create_dir_all(&self.images_dir)?;

        for raw_item in items {
            match self.import_one_saved_item(raw_item, &package_root) {
                Ok(true) => imported += 1,
                Ok(false) => skipped += 1,
                Err(e) => {
                    eprintln!("[nfun-cv] import saved item skip: {e}");
                    skipped += 1;
                }
            }
        }

        if imported == 0 && skipped == 0 {
            return Err(HistoryError::Message("导出包中没有条目".into()));
        }
        Ok((imported, skipped))
    }

    fn import_one_saved_item(
        &self,
        raw: &serde_json::Value,
        package_root: &Path,
    ) -> HistoryResult<bool> {
        let kind_s = raw
            .get("kind")
            .and_then(|v| v.as_str())
            .unwrap_or("text");
        let kind = ItemKind::parse(kind_s);
        let hash = raw
            .get("hash")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let pinned = raw.get("pinned").and_then(|v| v.as_bool()).unwrap_or(false);
        let favorited = raw
            .get("favorited")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let note = raw
            .get("note")
            .and_then(|v| v.as_str())
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());
        let preview = raw
            .get("preview")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let text = raw
            .get("text")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        let image_width = raw
            .get("imageWidth")
            .and_then(|v| v.as_u64())
            .map(|v| v as u32);
        let image_height = raw
            .get("imageHeight")
            .and_then(|v| v.as_u64())
            .map(|v| v as u32);

        let conn = self.conn.lock();

        // Existing by hash → merge flags / note
        if !hash.is_empty() {
            if let Some(existing) = find_by_hash(&conn, &hash)? {
                let new_pin = existing.pinned || pinned;
                let new_fav = existing.favorited || favorited;
                let new_note = note.clone().or(existing.note.clone());
                conn.execute(
                    "UPDATE history SET pinned=?1, favorited=?2, note=?3 WHERE id=?4",
                    params![
                        if new_pin { 1 } else { 0 },
                        if new_fav { 1 } else { 0 },
                        new_note,
                        existing.id
                    ],
                )?;
                return Ok(true);
            }
        }

        // New image: copy file into data root
        let mut image_path: Option<String> = None;
        if kind == ItemKind::Image {
            if let Some(rel) = raw.get("imageFile").and_then(|v| v.as_str()) {
                let src = package_root.join(rel);
                if src.exists() {
                    let name = src
                        .file_name()
                        .and_then(|s| s.to_str())
                        .unwrap_or("image.png");
                    let id_hint = raw
                        .get("id")
                        .and_then(|v| v.as_str())
                        .unwrap_or("");
                    let dest_name = if id_hint.is_empty() {
                        format!("{}.png", Uuid::new_v4().simple())
                    } else {
                        // keep original basename if free
                        let candidate = self.images_dir.join(name);
                        if candidate.exists() {
                            format!("{id_hint}.png")
                        } else {
                            name.to_string()
                        }
                    };
                    let dest = self.images_dir.join(&dest_name);
                    fs::copy(&src, &dest).map_err(|e| {
                        HistoryError::Message(format!("导入图片失败: {e}"))
                    })?;
                    image_path = Some(format!("images/{dest_name}"));
                }
            }
            if image_path.is_none() {
                return Ok(false); // image package missing file
            }
        }

        if kind == ItemKind::Text && text.as_ref().map(|t| t.trim().is_empty()).unwrap_or(true) {
            return Ok(false);
        }

        let id = raw
            .get("id")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string())
            .unwrap_or_else(|| Uuid::new_v4().simple().to_string());

        // If id collides, generate new
        let id_exists: bool = conn
            .query_row(
                "SELECT 1 FROM history WHERE id=?1",
                params![id],
                |_| Ok(true),
            )
            .optional()?
            .unwrap_or(false);
        let id = if id_exists {
            Uuid::new_v4().simple().to_string()
        } else {
            id
        };

        let hash = if hash.is_empty() {
            if let Some(ref t) = text {
                hash_bytes(t.as_bytes())
            } else if let Some(ref rel) = image_path {
                let bytes = fs::read(self.root.join(rel)).unwrap_or_default();
                hash_bytes(&bytes)
            } else {
                hash_bytes(id.as_bytes())
            }
        } else {
            hash
        };

        let preview = if preview.is_empty() {
            text.as_ref()
                .map(|t| preview_text(t))
                .unwrap_or_else(|| "导入条目".into())
        } else {
            preview
        };

        let now = Utc::now();
        let copied_at = raw
            .get("copiedAt")
            .and_then(|v| v.as_str())
            .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
            .map(|d| d.with_timezone(&Utc))
            .unwrap_or(now);
        let last_used_at = raw
            .get("lastUsedAt")
            .and_then(|v| v.as_str())
            .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
            .map(|d| d.with_timezone(&Utc))
            .unwrap_or(copied_at);
        let use_count = raw.get("useCount").and_then(|v| v.as_i64()).unwrap_or(1);

        conn.execute(
            "INSERT INTO history (
                id, kind, text, preview, hash, image_path, image_width, image_height,
                copied_at, last_used_at, use_count, pinned, favorited, note
            ) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14)",
            params![
                id,
                kind.as_str(),
                text,
                preview,
                hash,
                image_path,
                image_width.map(|v| v as i64),
                image_height.map(|v| v as i64),
                copied_at.to_rfc3339(),
                last_used_at.to_rfc3339(),
                use_count,
                if pinned { 1 } else { 0 },
                if favorited { 1 } else { 0 },
                note,
            ],
        )?;
        // Soft trim — keep imports
        trim(&conn, &self.root, self.max_items)?;
        Ok(true)
    }

    /// Restore full backup folder (history.db + images [+ config.json]).
    /// Replaces current history database. Returns row count after import.
    pub fn import_full(
        &self,
        package_dir: &Path,
        include_config: bool,
    ) -> HistoryResult<usize> {
        let src_db = package_dir.join("history.db");
        if !src_db.is_file() {
            return Err(HistoryError::Message(
                "不是完整备份包（缺少 history.db）".into(),
            ));
        }

        // Close live connection so we can replace the file.
        {
            let mut conn = self.conn.lock();
            let _ = conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);");
            // Swap to in-memory to release history.db handle
            let mem = Connection::open_in_memory().map_err(HistoryError::from)?;
            let old = std::mem::replace(&mut *conn, mem);
            drop(old);
        }

        let dst_db = self.root.join("history.db");
        // Remove local WAL/SHM so they don't pair with replaced main db
        for extra in ["history.db-wal", "history.db-shm"] {
            let p = self.root.join(extra);
            let _ = fs::remove_file(p);
        }

        fs::copy(&src_db, &dst_db).map_err(|e| {
            HistoryError::Message(format!("写入 history.db 失败: {e}"))
        })?;
        for extra in ["history.db-wal", "history.db-shm"] {
            let p = package_dir.join(extra);
            if p.exists() {
                let _ = fs::copy(&p, self.root.join(extra));
            }
        }

        // Replace images directory
        let src_images = package_dir.join("images");
        if src_images.is_dir() {
            if self.images_dir.exists() {
                let _ = fs::remove_dir_all(&self.images_dir);
            }
            copy_dir_recursive(&src_images, &self.images_dir)?;
        } else {
            fs::create_dir_all(&self.images_dir)?;
        }

        if include_config {
            let src_cfg = package_dir.join("config.json");
            if src_cfg.is_file() {
                let _ = fs::copy(&src_cfg, self.root.join("config.json"));
            }
        }

        // Reopen DB
        {
            let mut conn = self.conn.lock();
            let opened = Connection::open(&dst_db).map_err(HistoryError::from)?;
            let _ = opened.execute_batch(
                "PRAGMA synchronous=NORMAL;
                 PRAGMA foreign_keys=ON;
                 PRAGMA temp_store=MEMORY;
                 PRAGMA journal_mode=WAL;",
            );
            *conn = opened;
        }

        // Ensure schema (migrations) on imported db
        self.init_schema()?;

        let conn = self.conn.lock();
        let n: i64 = conn
            .query_row("SELECT COUNT(*) FROM history", [], |r| r.get(0))
            .unwrap_or(0);
        Ok(n as usize)
    }

    /// Fun stats for UI (total / today / top repeats / streak).
    pub fn fun_stats(&self) -> HistoryResult<StatsSnapshot> {
        let conn = self.conn.lock();
        let total_copies: i64 = global_stat(&conn, "total_copies")?;
        let total_duplicates: i64 = global_stat(&conn, "total_duplicates")?;
        let text_copies: i64 = global_stat(&conn, "text_copies")?;
        let image_copies: i64 = global_stat(&conn, "image_copies")?;

        let today = local_day_string();
        let (today_copies, today_duplicates): (i64, i64) = conn
            .query_row(
                "SELECT COALESCE(total,0), COALESCE(duplicates,0) FROM stats_daily WHERE day=?1",
                params![today],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?
            .unwrap_or((0, 0));

        let history_items: i64 =
            conn.query_row("SELECT COUNT(*) FROM history", [], |r| r.get(0))?;

        let duplicate_rate = if total_copies > 0 {
            total_duplicates as f64 / total_copies as f64
        } else {
            0.0
        };

        // Peak hour (local hour stored at write time)
        let peak = conn
            .query_row(
                "SELECT hour, COUNT(*) as c FROM clipboard_events
                 GROUP BY hour ORDER BY c DESC LIMIT 1",
                [],
                |r| Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?)),
            )
            .optional()?;

        let (peak_hour, peak_hour_count) = match peak {
            Some((h, c)) => (Some(h), c),
            None => (None, 0),
        };

        // Streak: consecutive local days with activity ending today or yesterday
        let days: Vec<String> = conn
            .prepare("SELECT day FROM stats_daily WHERE total > 0 ORDER BY day DESC LIMIT 400")?
            .query_map([], |r| r.get(0))?
            .filter_map(|r| r.ok())
            .collect();
        let streak_days = compute_streak(&days, &today);

        // Top repeated content
        let mut top_stmt = conn.prepare(
            "SELECT e.hash, COALESCE(h.preview, e.preview, e.hash), e.kind, COUNT(*) as c
             FROM clipboard_events e
             LEFT JOIN history h ON h.hash = e.hash
             GROUP BY e.hash
             HAVING c > 1
             ORDER BY c DESC
             LIMIT 8",
        )?;
        let top_repeats = top_stmt
            .query_map([], |r| {
                Ok(TopRepeat {
                    hash: r.get(0)?,
                    preview: r.get(1)?,
                    kind: r.get(2)?,
                    count: r.get(3)?,
                })
            })?
            .filter_map(|r| r.ok())
            .collect();

        let mut daily_stmt = conn.prepare(
            "SELECT day, total, duplicates, text, image FROM stats_daily
             ORDER BY day DESC LIMIT 14",
        )?;
        let mut daily: Vec<DailyStat> = daily_stmt
            .query_map([], |r| {
                Ok(DailyStat {
                    day: r.get(0)?,
                    total: r.get(1)?,
                    duplicates: r.get(2)?,
                    text: r.get(3)?,
                    image: r.get(4)?,
                })
            })?
            .filter_map(|r| r.ok())
            .collect();
        daily.reverse();

        Ok(StatsSnapshot {
            total_copies,
            total_duplicates,
            text_copies,
            image_copies,
            today_copies,
            today_duplicates,
            history_items,
            duplicate_rate,
            peak_hour,
            peak_hour_count,
            streak_days,
            top_repeats,
            daily,
        })
    }
}

fn last_event_hash(conn: &Connection) -> HistoryResult<Option<String>> {
    let h = conn
        .query_row(
            "SELECT hash FROM clipboard_events ORDER BY id DESC LIMIT 1",
            [],
            |r| r.get(0),
        )
        .optional()?;
    Ok(h)
}

fn record_event(
    conn: &Connection,
    item: &HistoryItem,
    is_duplicate: bool,
    is_consecutive: bool,
    byte_len: i64,
) -> HistoryResult<()> {
    let now = Local::now();
    let ts = now.to_rfc3339();
    let day = format!("{:04}-{:02}-{:02}", now.year(), now.month(), now.day());
    let hour = now.hour() as i64;

    conn.execute(
        "INSERT INTO clipboard_events
         (ts, day, hour, kind, hash, history_id, preview, is_duplicate, is_consecutive, byte_len)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)",
        params![
            ts,
            day,
            hour,
            item.kind.as_str(),
            item.hash,
            item.id,
            item.preview,
            if is_duplicate { 1 } else { 0 },
            if is_consecutive { 1 } else { 0 },
            byte_len,
        ],
    )?;

    bump_global(conn, "total_copies", 1)?;
    if is_duplicate {
        bump_global(conn, "total_duplicates", 1)?;
    }
    match item.kind {
        ItemKind::Text => bump_global(conn, "text_copies", 1)?,
        ItemKind::Image => bump_global(conn, "image_copies", 1)?,
    }

    conn.execute(
        "INSERT INTO stats_daily(day, total, duplicates, text, image)
         VALUES (?1, 1, ?2, ?3, ?4)
         ON CONFLICT(day) DO UPDATE SET
           total = total + 1,
           duplicates = duplicates + excluded.duplicates,
           text = text + excluded.text,
           image = image + excluded.image",
        params![
            day,
            if is_duplicate { 1 } else { 0 },
            if item.kind == ItemKind::Text { 1 } else { 0 },
            if item.kind == ItemKind::Image { 1 } else { 0 },
        ],
    )?;
    Ok(())
}

fn bump_global(conn: &Connection, key: &str, delta: i64) -> HistoryResult<()> {
    conn.execute(
        "INSERT INTO stats_global(key, value) VALUES (?1, ?2)
         ON CONFLICT(key) DO UPDATE SET value = value + excluded.value",
        params![key, delta],
    )?;
    Ok(())
}

fn global_stat(conn: &Connection, key: &str) -> HistoryResult<i64> {
    Ok(conn
        .query_row(
            "SELECT value FROM stats_global WHERE key=?1",
            params![key],
            |r| r.get(0),
        )
        .optional()?
        .unwrap_or(0))
}

fn local_day_string() -> String {
    let now = Local::now();
    format!("{:04}-{:02}-{:02}", now.year(), now.month(), now.day())
}

fn compute_streak(days_desc: &[String], today: &str) -> i64 {
    if days_desc.is_empty() {
        return 0;
    }
    use chrono::{Duration, NaiveDate};
    let mut streak = 0i64;
    let mut expect = match NaiveDate::parse_from_str(today, "%Y-%m-%d") {
        Ok(d) => d,
        Err(_) => return 0,
    };
    let first = match NaiveDate::parse_from_str(&days_desc[0], "%Y-%m-%d") {
        Ok(d) => d,
        Err(_) => return 0,
    };
    let Some(yesterday) = expect.checked_sub_signed(Duration::days(1)) else {
        return 0;
    };
    // Allow streak if last activity is today or yesterday
    if first < yesterday {
        return 0;
    }
    if first == yesterday {
        expect = first;
    }
    for d in days_desc {
        let parsed = match NaiveDate::parse_from_str(d, "%Y-%m-%d") {
            Ok(x) => x,
            Err(_) => break,
        };
        if parsed == expect {
            streak += 1;
            expect = match expect.checked_sub_signed(Duration::days(1)) {
                Some(d) => d,
                None => break,
            };
        } else if parsed < expect {
            break;
        }
    }
    streak
}

fn find_by_hash(conn: &Connection, hash: &str) -> HistoryResult<Option<HistoryItem>> {
    let mut stmt = conn.prepare(
        "SELECT id, kind, text, preview, hash, image_path, image_width, image_height,
                copied_at, last_used_at, use_count, pinned, favorited, note
         FROM history WHERE hash = ?1 LIMIT 1",
    )?;
    let item = stmt
        .query_row(params![hash], |row| map_row(row))
        .optional()?;
    Ok(item)
}

fn touch_existing(conn: &Connection, mut item: HistoryItem) -> HistoryResult<HistoryItem> {
    let now = Utc::now();
    item.copied_at = now;
    item.last_used_at = now;
    item.use_count = item.use_count.saturating_add(1);
    conn.execute(
        "UPDATE history SET copied_at=?1, last_used_at=?2, use_count=?3 WHERE id=?4",
        params![
            item.copied_at.to_rfc3339(),
            item.last_used_at.to_rfc3339(),
            item.use_count,
            item.id
        ],
    )?;
    Ok(item)
}

fn insert_new(
    conn: &Connection,
    root: &Path,
    max_items: usize,
    id: String,
    kind: ItemKind,
    text: Option<String>,
    preview: String,
    hash: String,
    image_path: Option<String>,
    image_width: Option<u32>,
    image_height: Option<u32>,
) -> HistoryResult<HistoryItem> {
    let now = Utc::now();
    let item = HistoryItem {
        id: id.clone(),
        kind,
        text,
        preview,
        hash,
        image_path,
        image_width,
        image_height,
        copied_at: now,
        last_used_at: now,
        use_count: 1,
        pinned: false,
        favorited: false,
        note: None,
    };
    conn.execute(
        "INSERT INTO history (
            id, kind, text, preview, hash, image_path, image_width, image_height,
            copied_at, last_used_at, use_count, pinned, favorited, note
        ) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14)",
        params![
            item.id,
            item.kind.as_str(),
            item.text,
            item.preview,
            item.hash,
            item.image_path,
            item.image_width.map(|v| v as i64),
            item.image_height.map(|v| v as i64),
            item.copied_at.to_rfc3339(),
            item.last_used_at.to_rfc3339(),
            item.use_count,
            if item.pinned { 1 } else { 0 },
            if item.favorited { 1 } else { 0 },
            item.note,
        ],
    )?;
    trim(conn, root, max_items)?;
    Ok(item)
}

fn trim(conn: &Connection, root: &Path, max_items: usize) -> HistoryResult<()> {
    // Keep pinned and favorited items outside the max_items budget
    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM history WHERE pinned = 0 AND favorited = 0",
        [],
        |r| r.get(0),
    )?;
    let max = max_items as i64;
    if count <= max {
        return Ok(());
    }
    let overflow = count - max;
    let mut stmt = conn.prepare(
        "SELECT id, image_path FROM history
         WHERE pinned = 0 AND favorited = 0
         ORDER BY copied_at ASC
         LIMIT ?1",
    )?;
    let doomed: Vec<(String, Option<String>)> = stmt
        .query_map(params![overflow], |row| Ok((row.get(0)?, row.get(1)?)))?
        .filter_map(|r| r.ok())
        .collect();
    for (id, image_path) in doomed {
        conn.execute("DELETE FROM history WHERE id = ?1", params![id])?;
        if let Some(rel) = image_path {
            let _ = fs::remove_file(root.join(rel));
        }
    }
    Ok(())
}

fn map_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<HistoryItem> {
    let kind: String = row.get(1)?;
    let copied_at: String = row.get(8)?;
    let last_used_at: String = row.get(9)?;
    let pinned: i64 = row.get(11)?;
    // favorited / note may be missing on very old rows if migration failed
    let favorited: i64 = row.get(12).unwrap_or(0);
    let note: Option<String> = row.get(13).unwrap_or(None);
    let note = note.and_then(|s| {
        let t = s.trim().to_string();
        if t.is_empty() {
            None
        } else {
            Some(t)
        }
    });
    Ok(HistoryItem {
        id: row.get(0)?,
        kind: ItemKind::parse(&kind),
        text: row.get(2)?,
        preview: row.get(3)?,
        hash: row.get(4)?,
        image_path: row.get(5)?,
        image_width: row.get::<_, Option<i64>>(6)?.map(|v| v as u32),
        image_height: row.get::<_, Option<i64>>(7)?.map(|v| v as u32),
        copied_at: DateTime::parse_from_rfc3339(&copied_at)
            .map(|d| d.with_timezone(&Utc))
            .unwrap_or_else(|_| Utc::now()),
        last_used_at: DateTime::parse_from_rfc3339(&last_used_at)
            .map(|d| d.with_timezone(&Utc))
            .unwrap_or_else(|_| Utc::now()),
        use_count: row.get(10)?,
        pinned: pinned != 0,
        favorited: favorited != 0,
        note,
    })
}

fn hash_bytes(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hex::encode(hasher.finalize())
}

fn escape_like(input: &str) -> String {
    input
        .replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_")
}

fn resolve_export_json(package_dir: &Path) -> HistoryResult<PathBuf> {
    let direct = package_dir.join("export.json");
    if direct.is_file() {
        return Ok(direct);
    }
    if package_dir.is_file()
        && package_dir
            .file_name()
            .and_then(|s| s.to_str())
            .map(|s| s.eq_ignore_ascii_case("export.json"))
            .unwrap_or(false)
    {
        return Ok(package_dir.to_path_buf());
    }
    Err(HistoryError::Message(
        "未找到 export.json（请选择导出生成的 FunCV-saved-… 目录）".into(),
    ))
}

fn copy_dir_recursive(src: &Path, dst: &Path) -> HistoryResult<()> {
    fs::create_dir_all(dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let ty = entry.file_type()?;
        let from = entry.path();
        let to = dst.join(entry.file_name());
        if ty.is_dir() {
            copy_dir_recursive(&from, &to)?;
        } else if ty.is_file() {
            fs::copy(&from, &to).map_err(|e| {
                HistoryError::Message(format!("复制文件失败 {}: {e}", from.display()))
            })?;
        }
    }
    Ok(())
}

fn preview_text(text: &str) -> String {
    let flat: String = text.chars().take(120).collect();
    if text.chars().count() > 120 {
        format!("{flat}…")
    } else {
        flat
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stores_and_dedupes_text_and_stats() {
        let dir = std::env::temp_dir().join(format!("nfun-cv-test-{}", Uuid::new_v4()));
        let store = HistoryStore::open(&dir, 100).unwrap();
        let a = store.add_text("hello world").unwrap();
        assert!(!a.is_duplicate);
        let b = store.add_text("hello world").unwrap();
        assert!(b.is_duplicate);
        assert_eq!(a.item.id, b.item.id);
        assert_eq!(b.item.use_count, 2);
        let list = store.list("").unwrap();
        assert_eq!(list.len(), 1);
        let stats = store.fun_stats().unwrap();
        assert_eq!(stats.total_copies, 2);
        assert_eq!(stats.total_duplicates, 1);
        assert_eq!(stats.today_copies, 2);
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn stores_image() {
        let dir = std::env::temp_dir().join(format!("nfun-cv-img-{}", Uuid::new_v4()));
        let store = HistoryStore::open(&dir, 100).unwrap();
        let rgba = vec![255u8, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 255, 255, 0, 255];
        let item = store.add_image(&rgba, 2, 2).unwrap();
        assert_eq!(item.item.kind, ItemKind::Image);
        assert!(item.item.image_path.is_some());
        let abs = store.root().join(item.item.image_path.as_ref().unwrap());
        assert!(abs.exists());
        let _ = fs::remove_dir_all(dir);
    }
}
