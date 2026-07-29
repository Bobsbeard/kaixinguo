//! Sync subsystem (FR-12..FR-17): a durable outbox plus a pluggable
//! connector. All Bingqilin-specific logic lives in `connector.rs` behind
//! the `Connector` enum, so the app is fully functional offline and the
//! service contract can be finalized independently (spec §7, §9).

pub mod connector;
pub mod engine;

use rusqlite::Connection;
use serde::Serialize;
use uuid::Uuid;

use crate::db::now;
use crate::error::AppError;

/// One queued local change. Serialized into the outbox payload column and
/// sent verbatim to the connector.
#[derive(Debug, Clone, Serialize, serde::Deserialize)]
pub struct SyncOp {
    pub id: String,
    pub entity: String, // "list" | "item"
    pub entity_id: String,
    pub op: String, // "create" | "update" | "delete"
    pub payload: serde_json::Value,
    pub queued_at: String,
}

/// Record a local mutation in the outbox. Callers must invoke this inside
/// the same transaction as the mutation itself (FR-14).
pub fn queue_op(
    conn: &Connection,
    entity: &str,
    entity_id: &str,
    op: &str,
    payload: serde_json::Value,
) -> Result<(), AppError> {
    conn.execute(
        "INSERT INTO sync_outbox (id, entity, entity_id, op, payload, queued_at) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        rusqlite::params![
            Uuid::new_v4().to_string(),
            entity,
            entity_id,
            op,
            payload.to_string(),
            now()
        ],
    )?;
    Ok(())
}
