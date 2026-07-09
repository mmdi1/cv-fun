use chrono::{DateTime, Utc};
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
    /// Relative path under data root for image files.
    pub image_path: Option<String>,
    pub image_width: Option<u32>,
    pub image_height: Option<u32>,
    pub copied_at: DateTime<Utc>,
    pub last_used_at: DateTime<Utc>,
    pub use_count: i64,
    pub pinned: bool,
}

pub struct HistoryStore {
    root: PathBuf,
    db_path: PathBuf,
    images_dir: PathBuf,
    max_items: usize,
}

impl HistoryStore {
    pub fn open(root: impl Into<PathBuf>, max_items: usize) -> HistoryResult<Self> {
        let root = root.into();
        let images_dir = root.join("images");
        let db_path = root.join("history.db");
        fs::create_dir_all(&images_dir)?;
        let store = Self {
            root,
            db_path,
            images_dir,
            max_items: if max_items == 0 {
                DEFAULT_MAX_ITEMS
            } else {
                max_items
            },
        };
        store.init_schema()?;
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

    fn connect(&self) -> HistoryResult<Connection> {
        let conn = Connection::open(&self.db_path)?;
        conn.execute_batch(
            "PRAGMA journal_mode=WAL;
             PRAGMA synchronous=NORMAL;
             PRAGMA foreign_keys=ON;",
        )?;
        Ok(conn)
    }

    fn init_schema(&self) -> HistoryResult<()> {
        let conn = self.connect()?;
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
                pinned INTEGER NOT NULL DEFAULT 0
            );
            CREATE INDEX IF NOT EXISTS idx_history_copied_at ON history(copied_at DESC);
            CREATE INDEX IF NOT EXISTS idx_history_hash ON history(hash);
            CREATE INDEX IF NOT EXISTS idx_history_text ON history(text);",
        )?;
        Ok(())
    }

    pub fn add_text(&self, text: &str) -> HistoryResult<HistoryItem> {
        let text = text.trim();
        if text.is_empty() {
            return Err(HistoryError::Message("clipboard text is empty".into()));
        }
        let hash = hash_bytes(text.as_bytes());
        let preview = preview_text(text);
        self.upsert(
            ItemKind::Text,
            Some(text.to_string()),
            preview,
            hash,
            None,
            None,
            None,
        )
    }

    pub fn add_image(
        &self,
        rgba: &[u8],
        width: u32,
        height: u32,
    ) -> HistoryResult<HistoryItem> {
        if width == 0 || height == 0 || rgba.is_empty() {
            return Err(HistoryError::Message("clipboard image is empty".into()));
        }
        let hash = hash_bytes(rgba);
        let conn = self.connect()?;
        if let Some(existing) = self.find_by_hash(&conn, &hash)? {
            return self.touch_existing(&conn, existing);
        }

        let id = Uuid::new_v4().simple().to_string();
        let rel = format!("images/{id}.png");
        let abs = self.root.join(&rel);
        let img = image::RgbaImage::from_raw(width, height, rgba.to_vec()).ok_or_else(|| {
            HistoryError::Message("invalid image buffer dimensions".into())
        })?;
        img.save(&abs)
            .map_err(|e| HistoryError::Message(format!("save image: {e}")))?;

        let preview = format!("图片 {width}×{height}");
        self.insert_new(
            &conn,
            id,
            ItemKind::Image,
            None,
            preview,
            hash,
            Some(rel),
            Some(width),
            Some(height),
        )
    }

    fn upsert(
        &self,
        kind: ItemKind,
        text: Option<String>,
        preview: String,
        hash: String,
        image_path: Option<String>,
        image_width: Option<u32>,
        image_height: Option<u32>,
    ) -> HistoryResult<HistoryItem> {
        let conn = self.connect()?;
        if let Some(existing) = self.find_by_hash(&conn, &hash)? {
            return self.touch_existing(&conn, existing);
        }
        let id = Uuid::new_v4().simple().to_string();
        self.insert_new(
            &conn,
            id,
            kind,
            text,
            preview,
            hash,
            image_path,
            image_width,
            image_height,
        )
    }

    fn touch_existing(
        &self,
        conn: &Connection,
        mut item: HistoryItem,
    ) -> HistoryResult<HistoryItem> {
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
        &self,
        conn: &Connection,
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
        };
        conn.execute(
            "INSERT INTO history (
                id, kind, text, preview, hash, image_path, image_width, image_height,
                copied_at, last_used_at, use_count, pinned
            ) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12)",
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
            ],
        )?;
        self.trim(conn)?;
        Ok(item)
    }

    fn find_by_hash(&self, conn: &Connection, hash: &str) -> HistoryResult<Option<HistoryItem>> {
        let mut stmt = conn.prepare(
            "SELECT id, kind, text, preview, hash, image_path, image_width, image_height,
                    copied_at, last_used_at, use_count, pinned
             FROM history WHERE hash = ?1 LIMIT 1",
        )?;
        let item = stmt
            .query_row(params![hash], |row| map_row(row))
            .optional()?;
        Ok(item)
    }

    /// List history summaries for the sidebar.
    ///
    /// Full `text` bodies are intentionally omitted (always `None`) so large clipboard
    /// payloads do not block startup IPC / UI. Fetch full content via [`Self::get`].
    pub fn list(&self, query: &str) -> HistoryResult<Vec<HistoryItem>> {
        let conn = self.connect()?;
        let q = query.trim();
        let mut items = Vec::with_capacity(64);

        if q.is_empty() {
            let mut stmt = conn.prepare(
                "SELECT id, kind, NULL, preview, hash, image_path, image_width, image_height,
                        copied_at, last_used_at, use_count, pinned
                 FROM history
                 ORDER BY pinned DESC, copied_at DESC
                 LIMIT 500",
            )?;
            for row in stmt.query_map([], |row| map_row(row))? {
                items.push(row?);
            }
        } else {
            let pattern = format!("%{}%", escape_like(q));
            let mut stmt = conn.prepare(
                "SELECT id, kind, NULL, preview, hash, image_path, image_width, image_height,
                        copied_at, last_used_at, use_count, pinned
                 FROM history
                 WHERE preview LIKE ?1 ESCAPE '\\'
                    OR IFNULL(text, '') LIKE ?1 ESCAPE '\\'
                 ORDER BY pinned DESC, copied_at DESC
                 LIMIT 500",
            )?;
            for row in stmt.query_map(params![pattern], |row| map_row(row))? {
                items.push(row?);
            }
        }

        Ok(items)
    }

    pub fn get(&self, id: &str) -> HistoryResult<HistoryItem> {
        let conn = self.connect()?;
        let mut stmt = conn.prepare(
            "SELECT id, kind, text, preview, hash, image_path, image_width, image_height,
                    copied_at, last_used_at, use_count, pinned
             FROM history WHERE id = ?1",
        )?;
        stmt.query_row(params![id], |row| map_row(row))
            .map_err(|_| HistoryError::Message(format!("history item {id} not found")))
    }

    pub fn delete(&self, id: &str) -> HistoryResult<()> {
        let item = self.get(id)?;
        let conn = self.connect()?;
        conn.execute("DELETE FROM history WHERE id = ?1", params![id])?;
        if let Some(rel) = item.image_path {
            let path = self.root.join(rel);
            let _ = fs::remove_file(path);
        }
        Ok(())
    }

    pub fn clear(&self) -> HistoryResult<()> {
        let conn = self.connect()?;
        conn.execute("DELETE FROM history", [])?;
        if self.images_dir.exists() {
            for entry in fs::read_dir(&self.images_dir)? {
                let entry = entry?;
                let _ = fs::remove_file(entry.path());
            }
        }
        Ok(())
    }

    pub fn mark_used(&self, id: &str) -> HistoryResult<()> {
        let conn = self.connect()?;
        let now = Utc::now().to_rfc3339();
        conn.execute(
            "UPDATE history SET last_used_at=?1, use_count=use_count+1 WHERE id=?2",
            params![now, id],
        )?;
        Ok(())
    }

    fn trim(&self, conn: &Connection) -> HistoryResult<()> {
        let count: i64 =
            conn.query_row("SELECT COUNT(*) FROM history WHERE pinned = 0", [], |r| {
                r.get(0)
            })?;
        let max = self.max_items as i64;
        if count <= max {
            return Ok(());
        }
        let overflow = count - max;
        let mut stmt = conn.prepare(
            "SELECT id, image_path FROM history
             WHERE pinned = 0
             ORDER BY copied_at ASC
             LIMIT ?1",
        )?;
        let doomed: Vec<(String, Option<String>)> = stmt
            .query_map(params![overflow], |row| {
                Ok((row.get(0)?, row.get(1)?))
            })?
            .filter_map(|r| r.ok())
            .collect();
        for (id, image_path) in doomed {
            conn.execute("DELETE FROM history WHERE id = ?1", params![id])?;
            if let Some(rel) = image_path {
                let _ = fs::remove_file(self.root.join(rel));
            }
        }
        Ok(())
    }
}

fn map_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<HistoryItem> {
    let kind: String = row.get(1)?;
    let copied_at: String = row.get(8)?;
    let last_used_at: String = row.get(9)?;
    let pinned: i64 = row.get(11)?;
    Ok(HistoryItem {
        id: row.get(0)?,
        kind: ItemKind::parse(&kind),
        text: row.get(2)?,
        preview: row.get(3)?,
        hash: row.get(4)?,
        image_path: row.get(5)?,
        image_width: row
            .get::<_, Option<i64>>(6)?
            .map(|v| v as u32),
        image_height: row
            .get::<_, Option<i64>>(7)?
            .map(|v| v as u32),
        copied_at: DateTime::parse_from_rfc3339(&copied_at)
            .map(|d| d.with_timezone(&Utc))
            .unwrap_or_else(|_| Utc::now()),
        last_used_at: DateTime::parse_from_rfc3339(&last_used_at)
            .map(|d| d.with_timezone(&Utc))
            .unwrap_or_else(|_| Utc::now()),
        use_count: row.get(10)?,
        pinned: pinned != 0,
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
    fn stores_and_dedupes_text() {
        let dir = std::env::temp_dir().join(format!("nfun-cv-test-{}", Uuid::new_v4()));
        let store = HistoryStore::open(&dir, 100).unwrap();
        let a = store.add_text("hello world").unwrap();
        let b = store.add_text("hello world").unwrap();
        assert_eq!(a.id, b.id);
        assert_eq!(b.use_count, 2);
        let list = store.list("").unwrap();
        assert_eq!(list.len(), 1);
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn stores_image() {
        let dir = std::env::temp_dir().join(format!("nfun-cv-img-{}", Uuid::new_v4()));
        let store = HistoryStore::open(&dir, 100).unwrap();
        let rgba = vec![255u8, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 255, 255, 0, 255];
        let item = store.add_image(&rgba, 2, 2).unwrap();
        assert_eq!(item.kind, ItemKind::Image);
        assert!(item.image_path.is_some());
        let abs = store.root().join(item.image_path.as_ref().unwrap());
        assert!(abs.exists());
        let _ = fs::remove_dir_all(dir);
    }
}
