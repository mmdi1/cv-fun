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
                pinned INTEGER NOT NULL DEFAULT 0
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
        let abs = self.root.join(&rel);
        let img = image::RgbaImage::from_raw(width, height, rgba.to_vec()).ok_or_else(|| {
            HistoryError::Message("invalid image buffer dimensions".into())
        })?;
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
        let conn = self.conn.lock();
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
        let conn = self.conn.lock();
        conn.execute("DELETE FROM history WHERE id = ?1", params![id])?;
        if let Some(rel) = item.image_path {
            let _ = fs::remove_file(self.root.join(rel));
        }
        Ok(())
    }

    pub fn clear(&self) -> HistoryResult<()> {
        let conn = self.conn.lock();
        conn.execute("DELETE FROM history", [])?;
        drop(conn);
        if self.images_dir.exists() {
            for entry in fs::read_dir(&self.images_dir)? {
                let entry = entry?;
                let _ = fs::remove_file(entry.path());
            }
        }
        Ok(())
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
                copied_at, last_used_at, use_count, pinned
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
    trim(conn, root, max_items)?;
    Ok(item)
}

fn trim(conn: &Connection, root: &Path, max_items: usize) -> HistoryResult<()> {
    let count: i64 =
        conn.query_row("SELECT COUNT(*) FROM history WHERE pinned = 0", [], |r| {
            r.get(0)
        })?;
    let max = max_items as i64;
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
