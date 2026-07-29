//! Sync engine (FR-13..FR-16): replays the durable outbox through the
//! configured connector, applies per-op results, and records sync status.
//! Never blocks dictionary use; failures are recorded, not raised as dialogs.

use rusqlite::Connection;
use serde::Serialize;
use tauri::State;

use super::connector::{Connector, OpResult};
use super::SyncOp;
use crate::db::{lock, now, AppState};
use crate::error::AppError;

const BATCH_SIZE: usize = 100;
const MAX_ATTEMPTS: i64 = 5;

#[derive(Debug, Serialize)]
pub struct SyncReport {
    pub pushed: usize,
    pub failed: usize,
    pub pending: usize,
    pub message: String,
    pub synced_at: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct SyncSettingsView {
    pub server_url: String,
    pub has_token: bool,
    pub pending_ops: i64,
    pub last_sync_at: Option<String>,
    pub last_error: Option<String>,
}

fn get_setting(conn: &Connection, key: &str) -> Option<String> {
    conn.query_row(
        "SELECT value FROM settings WHERE key = ?1",
        rusqlite::params![key],
        |r| r.get(0),
    )
    .ok()
}

fn set_setting(conn: &Connection, key: &str, value: &str) -> Result<(), AppError> {
    conn.execute(
        "INSERT INTO settings (key, value) VALUES (?1, ?2) \
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        rusqlite::params![key, value],
    )?;
    Ok(())
}

fn set_state(conn: &Connection, key: &str, value: &str) -> Result<(), AppError> {
    conn.execute(
        "INSERT INTO sync_state (key, value) VALUES (?1, ?2) \
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        rusqlite::params![key, value],
    )?;
    Ok(())
}

fn pending_count(conn: &Connection) -> i64 {
    conn.query_row("SELECT COUNT(*) FROM sync_outbox", [], |r| r.get(0))
        .unwrap_or(0)
}

fn pending_ops(conn: &Connection, limit: usize) -> Result<Vec<SyncOp>, AppError> {
    let mut stmt = conn.prepare(
        "SELECT id, entity, entity_id, op, payload, queued_at \
         FROM sync_outbox ORDER BY queued_at, id LIMIT ?1",
    )?;
    let rows = stmt.query_map(rusqlite::params![limit as i64], |row| {
        let payload_str: String = row.get(4)?;
        Ok(SyncOp {
            id: row.get(0)?,
            entity: row.get(1)?,
            entity_id: row.get(2)?,
            op: row.get(3)?,
            payload: serde_json::from_str(&payload_str)
                .unwrap_or(serde_json::Value::Null),
            queued_at: row.get(5)?,
        })
    })?;
    Ok(rows.collect::<Result<Vec<_>, _>>()?)
}

fn apply_result(conn: &Connection, op: &SyncOp, result: &OpResult) -> Result<(), AppError> {
    let table = match op.entity.as_str() {
        "list" => "lists",
        "item" => "list_items",
        _ => return Ok(()),
    };
    match &result.outcome {
        Ok(remote_id) => {
            conn.execute("DELETE FROM sync_outbox WHERE id = ?1", rusqlite::params![result.op_id])?;
            // For deletes the row is already tombstoned; still mark it synced
            // so it can be garbage-collected later.
            conn.execute(
                &format!(
                    "UPDATE {table} SET sync_state = 'synced', \
                     remote_id = COALESCE(?2, remote_id) WHERE id = ?1"
                ),
                rusqlite::params![op.entity_id, remote_id],
            )?;
        }
        Err(_) => {
            conn.execute(
                "UPDATE sync_outbox SET attempts = attempts + 1 WHERE id = ?1",
                rusqlite::params![result.op_id],
            )?;
            let attempts: i64 = conn.query_row(
                "SELECT attempts FROM sync_outbox WHERE id = ?1",
                rusqlite::params![result.op_id],
                |r| r.get(0),
            )?;
            if attempts >= MAX_ATTEMPTS {
                conn.execute(
                    &format!("UPDATE {table} SET sync_state = 'error' WHERE id = ?1"),
                    rusqlite::params![op.entity_id],
                )?;
            }
        }
    }
    Ok(())
}

pub async fn run_sync(state: &AppState) -> Result<SyncReport, AppError> {
    let (ops, url, token) = {
        let conn = lock(&state.user)?;
        let url = get_setting(&conn, "server_url").unwrap_or_default();
        let token = get_setting(&conn, "auth_token").unwrap_or_default();
        (pending_ops(&conn, BATCH_SIZE)?, url, token)
    };

    if ops.is_empty() {
        let pending = {
            let conn = lock(&state.user)?;
            pending_count(&conn)
        };
        return Ok(SyncReport {
            pushed: 0,
            failed: 0,
            pending: pending as usize,
            message: "Nothing to sync".into(),
            synced_at: Some(now()),
        });
    }
    if url.trim().is_empty() {
        return Err(AppError::SyncNotConfigured);
    }

    let connector = Connector::from_settings(&url, &token);
    let results = match connector.push(&ops).await {
        Ok(r) => r,
        Err(e) => {
            // Whole-batch failure (offline, server down): leave the outbox
            // untouched so everything retries later (FR-6 user story).
            let conn = lock(&state.user)?;
            set_state(&conn, "last_error", &e.to_string())?;
            return Err(e);
        }
    };

    let mut pushed = 0usize;
    let mut failed = 0usize;
    let last_error;
    let pending;
    {
        let conn = lock(&state.user)?;
        // Op id -> op lookup so results can be matched back to entities.
        for r in &results {
            if let Some(op) = ops.iter().find(|o| o.id == r.op_id) {
                apply_result(&conn, op, r)?;
                match &r.outcome {
                    Ok(_) => pushed += 1,
                    Err(_) => failed += 1,
                }
            }
        }
        last_error = results
            .iter()
            .filter_map(|r| r.outcome.as_ref().err().cloned())
            .next();
        match &last_error {
            Some(e) => set_state(&conn, "last_error", e)?,
            None => set_state(&conn, "last_error", "")?,
        }
        let ts = now();
        set_state(&conn, "last_sync_at", &ts)?;
        pending = pending_count(&conn);
    }

    Ok(SyncReport {
        pushed,
        failed,
        pending: pending as usize,
        message: match last_error {
            Some(e) => format!("Synced {pushed}, {failed} failed: {e}"),
            None => format!("Synced {pushed} change(s)"),
        },
        synced_at: Some(now()),
    })
}

// ----- Tauri commands -----

#[tauri::command]
pub async fn sync_now(state: State<'_, AppState>) -> Result<SyncReport, AppError> {
    run_sync(state.inner()).await
}

#[tauri::command]
pub fn get_sync_settings(state: State<'_, AppState>) -> Result<SyncSettingsView, AppError> {
    let conn = lock(&state.user)?;
    Ok(SyncSettingsView {
        server_url: get_setting(&conn, "server_url").unwrap_or_default(),
        has_token: get_setting(&conn, "auth_token")
            .map(|t| !t.is_empty())
            .unwrap_or(false),
        pending_ops: pending_count(&conn),
        last_sync_at: get_setting(&conn, "last_sync_at").filter(|s| !s.is_empty()),
        last_error: get_setting(&conn, "last_error").filter(|s| !s.is_empty()),
    })
}

/// `auth_token = None` or empty keeps the stored token (FR-12).
/// NOTE: tokens are stored in the local settings table for v0.1; moving to
/// the OS keychain (e.g. the `keyring` crate) is tracked as a hardening task.
#[tauri::command]
pub fn set_sync_settings(
    state: State<'_, AppState>,
    server_url: String,
    auth_token: Option<String>,
) -> Result<(), AppError> {
    let conn = lock(&state.user)?;
    set_setting(&conn, "server_url", server_url.trim())?;
    if let Some(t) = auth_token {
        if !t.is_empty() {
            set_setting(&conn, "auth_token", &t)?;
        }
    }
    Ok(())
}

#[tauri::command]
pub fn get_sync_status(state: State<'_, AppState>) -> Result<SyncSettingsView, AppError> {
    get_sync_settings(state)
}
