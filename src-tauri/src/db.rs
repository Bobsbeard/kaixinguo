//! Database setup: the read-only CC-CEDICT dictionary database and the
//! writable user database (lists, items, sync outbox, settings).
//!
//! The dictionary DB is ATTACHed to the user connection as `dict` so list
//! queries can join against entries in a single statement.

use std::path::PathBuf;
use std::sync::{Mutex, MutexGuard};

use rusqlite::{Connection, OpenFlags};
use tauri::{AppHandle, Manager};

/// Build a SQLite `file:` URI that opens the dictionary read-only
/// (`mode=ro`). Percent-encodes everything outside a small safe set so
/// Windows backslashes, spaces and non-ASCII profile names survive.
/// Read-only matters: installed into "Program Files", a standard user
/// cannot open the DB read-write, and SQLite would refuse to open it.
fn dict_file_uri(path: &std::path::Path) -> String {
    let mut s = path.to_string_lossy().replace('\\', "/");
    if !s.starts_with('/') {
        s = format!("/{s}"); // Windows drive path: C:/... -> /C:/...
    }
    let mut out = String::from("file:");
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'/' | b'.' | b'-' | b'_' | b':' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    format!("{out}?mode=ro")
}

use crate::error::AppError;

pub struct AppState {
    /// Read-only dictionary database (bundled CC-CEDICT build).
    pub dict: Mutex<Connection>,
    /// Writable user database. The dictionary DB is attached to this
    /// connection under the schema name `dict`.
    pub user: Mutex<Connection>,
}

pub fn now() -> String {
    chrono::Utc::now().to_rfc3339()
}

pub fn lock(conn: &Mutex<Connection>) -> Result<MutexGuard<'_, Connection>, AppError> {
    conn.lock()
        .map_err(|_| AppError::Other("database lock poisoned".into()))
}

fn dict_db_path(app: &AppHandle) -> Result<PathBuf, AppError> {
    // Packaged layout: <resource_dir>/resources/dictionary.db
    if let Ok(dir) = app.path().resource_dir() {
        for candidate in [
            dir.join("resources").join("dictionary.db"),
            dir.join("dictionary.db"),
        ] {
            if candidate.exists() {
                return Ok(candidate);
            }
        }
    }
    // Development layout: <src-tauri>/resources/dictionary.db
    let dev = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("resources")
        .join("dictionary.db");
    if dev.exists() {
        return Ok(dev);
    }
    Err(AppError::Other(
        "dictionary.db not found — run `python3 tools/import_cedict.py` first".into(),
    ))
}

fn user_db_path(app: &AppHandle) -> Result<PathBuf, AppError> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| AppError::Other(format!("cannot resolve app data dir: {e}")))?;
    std::fs::create_dir_all(&dir)?;
    Ok(dir.join("user.db"))
}

const USER_SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS lists (
    id          TEXT PRIMARY KEY,
    name        TEXT NOT NULL,
    created_at  TEXT NOT NULL,
    updated_at  TEXT NOT NULL,
    deleted_at  TEXT,
    remote_id   TEXT,
    sync_state  TEXT NOT NULL DEFAULT 'pending'
);
CREATE TABLE IF NOT EXISTS list_items (
    id          TEXT PRIMARY KEY,
    list_id     TEXT NOT NULL REFERENCES lists(id),
    entry_id    INTEGER NOT NULL,
    position    REAL NOT NULL,
    added_at    TEXT NOT NULL,
    updated_at  TEXT NOT NULL,
    deleted_at  TEXT,
    remote_id   TEXT,
    sync_state  TEXT NOT NULL DEFAULT 'pending'
);
CREATE INDEX IF NOT EXISTS idx_items_list ON list_items(list_id, position);
CREATE TABLE IF NOT EXISTS sync_outbox (
    id          TEXT PRIMARY KEY,
    entity      TEXT NOT NULL,   -- 'list' | 'item'
    entity_id   TEXT NOT NULL,
    op          TEXT NOT NULL,   -- 'create' | 'update' | 'delete'
    payload     TEXT NOT NULL,   -- JSON
    queued_at   TEXT NOT NULL,
    attempts    INTEGER NOT NULL DEFAULT 0
);
CREATE TABLE IF NOT EXISTS sync_state (
    key   TEXT PRIMARY KEY,
    value TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS settings (
    key   TEXT PRIMARY KEY,
    value TEXT NOT NULL
);
";

pub fn init(app: &AppHandle) -> Result<AppState, AppError> {
    let dict_path = dict_db_path(app)?;
    let user_path = user_db_path(app)?;

    // The bundled dictionary is opened strictly read-only: it lives under
    // the install dir (e.g. Program Files), where standard users may not
    // write, and SQLite refuses a read-write open it cannot perform.
    let dict = Connection::open_with_flags(&dict_path, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    dict.execute_batch("PRAGMA query_only = ON;")?;

    let user = Connection::open_with_flags(
        &user_path,
        OpenFlags::SQLITE_OPEN_READ_WRITE
            | OpenFlags::SQLITE_OPEN_CREATE
            | OpenFlags::SQLITE_OPEN_URI,
    )?;
    user.execute_batch(USER_SCHEMA)?;
    // Attach the dictionary (read-only) so list queries can join `dict.entries`.
    user.execute(
        "ATTACH DATABASE ?1 AS dict",
        rusqlite::params![dict_file_uri(&dict_path)],
    )?;
    user.execute_batch("PRAGMA foreign_keys = ON;")?;

    Ok(AppState {
        dict: Mutex::new(dict),
        user: Mutex::new(user),
    })
}
