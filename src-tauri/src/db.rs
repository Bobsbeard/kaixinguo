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
fn dict_file_uri(path: &std::path::Path) -> String {
    // Strip Windows verbatim-path prefixes: resource_dir() can return
    // "\\?\C:\..." (via current_exe), and a stray '?' would corrupt the
    // URI ("invalid uri authority: %3F").
    let raw = path.to_string_lossy().replace('\\', "/");
    let stripped = raw
        .strip_prefix("//?/UNC/")
        .map(|rest| format!("//{rest}"))
        .or_else(|| raw.strip_prefix("//?/").map(str::to_string))
        .unwrap_or(raw);
    let s = if stripped.starts_with('/') {
        stripped
    } else {
        format!("/{stripped}") // Windows drive path: C:/... -> /C:/...
    };
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

fn app_data_dir(app: &AppHandle) -> Result<PathBuf, AppError> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| AppError::Other(format!("cannot resolve app data dir: {e}")))?;
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}

/// Bump when shipping a rebuilt dictionary so existing installs re-copy.
const DICT_COPY_VERSION: &str = "1";

/// The bundled dictionary ships inside the install dir (e.g. Program
/// Files), which standard users may not write to — and a WAL-mode SQLite
/// file cannot even be *read* from there (it must create -shm/-wal
/// sidecars). So on first run we copy dictionary.db into the
/// user-writable app data dir and open that copy. This is also where the
/// in-app dictionary updater (FR-18) will drop new data.
fn ensure_dict_copy(
    app: &AppHandle,
    resource: &std::path::Path,
    user: &Connection,
) -> Result<PathBuf, AppError> {
    let target = app_data_dir(app)?.join("dictionary.db");
    let have_version: Option<String> = user
        .query_row(
            "SELECT value FROM settings WHERE key = 'dict_copy_version'",
            [],
            |r| r.get(0),
        )
        .ok();
    let up_to_date = target.exists()
        && have_version.as_deref() == Some(DICT_COPY_VERSION);
    if !up_to_date {
        std::fs::copy(resource, &target).map_err(|e| {
            AppError::Other(format!(
                "could not copy dictionary into the app data folder: {e}"
            ))
        })?;
        // Version is recorded only after a complete copy, so an
        // interrupted copy is redone on the next launch.
        user.execute(
            "INSERT INTO settings (key, value) VALUES ('dict_copy_version', ?1) \
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            rusqlite::params![DICT_COPY_VERSION],
        )?;
    }
    Ok(target)
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
    let resource_dict = dict_db_path(app)?;
    let user_path = app_data_dir(app)?.join("user.db");

    let user = Connection::open_with_flags(
        &user_path,
        OpenFlags::SQLITE_OPEN_READ_WRITE
            | OpenFlags::SQLITE_OPEN_CREATE
            | OpenFlags::SQLITE_OPEN_URI,
    )?;
    user.execute_batch(USER_SCHEMA)?;
    user.execute_batch("PRAGMA foreign_keys = ON;")?;

    // Work on a user-writable copy of the dictionary; the bundled file in
    // the install dir is not openable by standard users on Windows.
    let dict_path = ensure_dict_copy(app, &resource_dict, &user)?;
    let dict = Connection::open_with_flags(&dict_path, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    dict.execute_batch("PRAGMA query_only = ON;")?;

    // Attach the dictionary (read-only) so list queries can join `dict.entries`.
    user.execute(
        "ATTACH DATABASE ?1 AS dict",
        rusqlite::params![dict_file_uri(&dict_path)],
    )?;

    Ok(AppState {
        dict: Mutex::new(dict),
        user: Mutex::new(user),
    })
}
