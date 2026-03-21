//! Minimal repo CRUD for the Tauri desktop GUI.
//!
//! Opens the same `~/.nomadterm/nomadterm.db` that the nomadterm daemon uses.
//! Only the `repos` table is managed here.

use anyhow::{Context, Result};
use rusqlite::{Connection, params};

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct RepoRow {
    pub id: String,
    pub path: String,
    pub name: String,
    pub token: String,
    pub added_at: i64,
    pub last_seen: Option<i64>,
    pub is_active: bool,
}

// ---------------------------------------------------------------------------
// Database handle
// ---------------------------------------------------------------------------

pub struct RepoDB {
    conn: Connection,
}

impl RepoDB {
    /// Open (or create) the nomadterm database at `~/.nomadterm/nomadterm.db`.
    pub fn open() -> Result<Self> {
        let path = hcom_db_path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("create nomadterm dir {:?}", parent))?;
        }
        let conn = Connection::open(&path)
            .with_context(|| format!("open db {:?}", path))?;
        let db = Self { conn };
        db.ensure_repos_table()?;
        Ok(db)
    }

    /// Create the repos table if it doesn't exist yet.
    fn ensure_repos_table(&self) -> Result<()> {
        self.conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS repos (
                id         TEXT PRIMARY KEY,
                path       TEXT NOT NULL UNIQUE,
                name       TEXT NOT NULL,
                token      TEXT NOT NULL,
                added_at   INTEGER NOT NULL,
                last_seen  INTEGER,
                is_active  INTEGER NOT NULL DEFAULT 0
            );",
        )?;
        Ok(())
    }

    // -----------------------------------------------------------------------
    // Repo CRUD
    // -----------------------------------------------------------------------

    /// Upsert a repo by path.  Returns the existing row if path already registered.
    pub fn upsert_repo(&self, path: &str) -> Result<RepoRow> {
        let name = std::path::Path::new(path)
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| path.to_string());

        let id = uuid::Uuid::new_v4().to_string();
        let token: String = (0..32)
            .map(|_| format!("{:02x}", rand::random::<u8>()))
            .collect();
        let added_at = epoch_now();

        self.conn.execute(
            "INSERT OR IGNORE INTO repos (id, path, name, token, added_at) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![id, path, name, token, added_at],
        )?;

        let row = self.conn.query_row(
            "SELECT id, path, name, token, added_at, last_seen, is_active \
             FROM repos WHERE path = ?1",
            params![path],
            row_from_query,
        )?;
        Ok(row)
    }

    /// List all repos ordered by most-recently-seen first.
    pub fn list_repos(&self) -> Result<Vec<RepoRow>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, path, name, token, added_at, last_seen, is_active \
             FROM repos ORDER BY last_seen DESC, added_at DESC",
        )?;
        let rows = stmt
            .query_map([], row_from_query)?
            .filter_map(|r| r.ok())
            .collect();
        Ok(rows)
    }

    /// Set the `is_active` flag.
    pub fn set_repo_active(&self, id: &str, active: bool) -> Result<()> {
        self.conn.execute(
            "UPDATE repos SET is_active = ?1 WHERE id = ?2",
            params![active as i64, id],
        )?;
        Ok(())
    }

    /// Delete a repo by id.
    pub fn remove_repo(&self, id: &str) -> Result<()> {
        self.conn
            .execute("DELETE FROM repos WHERE id = ?1", params![id])?;
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn hcom_db_path() -> std::path::PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join(".nomadterm")
        .join("nomadterm.db")
}

fn epoch_now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

fn row_from_query(row: &rusqlite::Row<'_>) -> rusqlite::Result<RepoRow> {
    Ok(RepoRow {
        id: row.get(0)?,
        path: row.get(1)?,
        name: row.get(2)?,
        token: row.get(3)?,
        added_at: row.get(4)?,
        last_seen: row.get(5)?,
        is_active: row.get::<_, i64>(6)? != 0,
    })
}
