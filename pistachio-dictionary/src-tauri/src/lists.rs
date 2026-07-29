//! Word lists (FR-6..FR-11): named, explicitly ordered lists stored in the
//! user database. Every mutation is recorded in the sync outbox inside the
//! same transaction, so offline changes are never lost (FR-14).

use rusqlite::{Connection, OptionalExtension};
use serde::Serialize;
use tauri::State;
use uuid::Uuid;

use crate::db::{lock, now, AppState};
use crate::error::AppError;
use crate::search::EntrySummary;
use crate::sync::queue_op;

#[derive(Debug, Serialize)]
pub struct WordList {
    pub id: String,
    pub name: String,
    pub item_count: i64,
    pub updated_at: String,
    pub sync_state: String,
}

#[derive(Debug, Serialize)]
pub struct ListItemView {
    pub item_id: String,
    pub position: f64,
    pub sync_state: String,
    pub entry: EntrySummary,
}

fn touch_list(conn: &Connection, list_id: &str) -> Result<(), AppError> {
    conn.execute(
        "UPDATE lists SET updated_at = ?2, sync_state = 'pending' WHERE id = ?1",
        rusqlite::params![list_id, now()],
    )?;
    Ok(())
}

fn mark_item_pending(conn: &Connection, item_id: &str) -> Result<(), AppError> {
    conn.execute(
        "UPDATE list_items SET updated_at = ?2, sync_state = 'pending' WHERE id = ?1",
        rusqlite::params![item_id, now()],
    )?;
    Ok(())
}

#[tauri::command]
pub fn get_lists(state: State<'_, AppState>) -> Result<Vec<WordList>, AppError> {
    let conn = lock(&state.user)?;
    let mut stmt = conn.prepare(
        "SELECT l.id, l.name, l.updated_at, l.sync_state, \
         (SELECT COUNT(*) FROM list_items i WHERE i.list_id = l.id AND i.deleted_at IS NULL) \
         FROM lists l WHERE l.deleted_at IS NULL ORDER BY l.updated_at DESC",
    )?;
    let rows = stmt.query_map([], |row| {
        Ok(WordList {
            id: row.get(0)?,
            name: row.get(1)?,
            updated_at: row.get(2)?,
            sync_state: row.get(3)?,
            item_count: row.get(4)?,
        })
    })?;
    Ok(rows.collect::<Result<Vec<_>, _>>()?)
}

#[tauri::command]
pub fn create_list(state: State<'_, AppState>, name: String) -> Result<WordList, AppError> {
    let name = name.trim().to_string();
    if name.is_empty() {
        return Err(AppError::Invalid("list name cannot be empty".into()));
    }
    let conn = lock(&state.user)?;
    let id = Uuid::new_v4().to_string();
    let ts = now();
    let tx = conn.unchecked_transaction()?;
    tx.execute(
        "INSERT INTO lists (id, name, created_at, updated_at) VALUES (?1, ?2, ?3, ?3)",
        rusqlite::params![id, name, ts],
    )?;
    queue_op(&tx, "list", &id, "create", serde_json::json!({ "id": id, "name": name }))?;
    tx.commit()?;
    Ok(WordList {
        id,
        name,
        item_count: 0,
        updated_at: ts,
        sync_state: "pending".into(),
    })
}

#[tauri::command]
pub fn rename_list(state: State<'_, AppState>, id: String, name: String) -> Result<(), AppError> {
    let name = name.trim().to_string();
    if name.is_empty() {
        return Err(AppError::Invalid("list name cannot be empty".into()));
    }
    let conn = lock(&state.user)?;
    let tx = conn.unchecked_transaction()?;
    let changed = tx.execute(
        "UPDATE lists SET name = ?2, updated_at = ?3, sync_state = 'pending' \
         WHERE id = ?1 AND deleted_at IS NULL",
        rusqlite::params![id, name, now()],
    )?;
    if changed == 0 {
        return Err(AppError::NotFound(format!("list {id}")));
    }
    queue_op(&tx, "list", &id, "update", serde_json::json!({ "id": id, "name": name }))?;
    tx.commit()?;
    Ok(())
}

#[tauri::command]
pub fn delete_list(state: State<'_, AppState>, id: String) -> Result<(), AppError> {
    let conn = lock(&state.user)?;
    let tx = conn.unchecked_transaction()?;
    let ts = now();
    let remote_id: Option<String> = tx
        .query_row(
            "SELECT remote_id FROM lists WHERE id = ?1 AND deleted_at IS NULL",
            rusqlite::params![id],
            |r| r.get(0),
        )
        .optional()?
        .ok_or_else(|| AppError::NotFound(format!("list {id}")))?;
    tx.execute(
        "UPDATE lists SET deleted_at = ?2, sync_state = 'pending' WHERE id = ?1",
        rusqlite::params![id, ts],
    )?;
    // Soft-delete the items too, so deletions propagate on sync.
    tx.execute(
        "UPDATE list_items SET deleted_at = ?2, sync_state = 'pending' \
         WHERE list_id = ?1 AND deleted_at IS NULL",
        rusqlite::params![id, ts],
    )?;
    queue_op(&tx, "list", &id, "delete", serde_json::json!({ "id": id, "remote_id": remote_id }))?;
    tx.commit()?;
    Ok(())
}

#[tauri::command]
pub fn get_list_items(state: State<'_, AppState>, list_id: String) -> Result<Vec<ListItemView>, AppError> {
    let conn = lock(&state.user)?;
    let mut stmt = conn.prepare(
        "SELECT i.id, i.position, i.sync_state, \
         e.id, e.traditional, e.simplified, e.pinyin_marks, e.definitions \
         FROM list_items i JOIN dict.entries e ON e.id = i.entry_id \
         WHERE i.list_id = ?1 AND i.deleted_at IS NULL \
         ORDER BY i.position, i.added_at",
    )?;
    let rows = stmt.query_map(rusqlite::params![list_id], |row| {
        Ok(ListItemView {
            item_id: row.get(0)?,
            position: row.get(1)?,
            sync_state: row.get(2)?,
            entry: EntrySummary {
                id: row.get(3)?,
                traditional: row.get(4)?,
                simplified: row.get(5)?,
                pinyin_marks: row.get(6)?,
                definitions: row.get(7)?,
            },
        })
    })?;
    Ok(rows.collect::<Result<Vec<_>, _>>()?)
}

/// FR-7 + FR-9: add an entry to a list; duplicates are rejected so the UI
/// can prompt instead of silently duplicating.
#[tauri::command]
pub fn add_to_list(
    state: State<'_, AppState>,
    list_id: String,
    entry_id: i64,
) -> Result<ListItemView, AppError> {
    let conn = lock(&state.user)?;
    let tx = conn.unchecked_transaction()?;

    // Verify the list exists and the entry exists (in the attached dict DB).
    let list_name: Option<String> = tx
        .query_row(
            "SELECT name FROM lists WHERE id = ?1 AND deleted_at IS NULL",
            rusqlite::params![list_id],
            |r| r.get(0),
        )
        .optional()?;
    let list_name = list_name.ok_or_else(|| AppError::NotFound(format!("list {list_id}")))?;

    let entry = tx
        .query_row(
            "SELECT id, traditional, simplified, pinyin_marks, definitions \
             FROM dict.entries WHERE id = ?1",
            rusqlite::params![entry_id],
            |row| {
                Ok(EntrySummary {
                    id: row.get(0)?,
                    traditional: row.get(1)?,
                    simplified: row.get(2)?,
                    pinyin_marks: row.get(3)?,
                    definitions: row.get(4)?,
                })
            },
        )
        .optional()?
        .ok_or_else(|| AppError::NotFound(format!("entry {entry_id}")))?;

    let dupe: Option<String> = tx
        .query_row(
            "SELECT id FROM list_items \
             WHERE list_id = ?1 AND entry_id = ?2 AND deleted_at IS NULL",
            rusqlite::params![list_id, entry_id],
            |r| r.get(0),
        )
        .optional()?;
    if dupe.is_some() {
        return Err(AppError::Duplicate(format!(
            "'{}' is already in list '{}'",
            entry.simplified, list_name
        )));
    }

    let position: f64 = tx
        .query_row(
            "SELECT COALESCE(MAX(position), 0.0) + 1.0 FROM list_items \
             WHERE list_id = ?1 AND deleted_at IS NULL",
            rusqlite::params![list_id],
            |r| r.get(0),
        )?;
    let item_id = Uuid::new_v4().to_string();
    let ts = now();
    tx.execute(
        "INSERT INTO list_items (id, list_id, entry_id, position, added_at, updated_at) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?5)",
        rusqlite::params![item_id, list_id, entry_id, position, ts],
    )?;
    touch_list(&tx, &list_id)?;
    // The payload embeds an entry snapshot so Bingqilin can store the word
    // without access to the dictionary database.
    queue_op(
        &tx,
        "item",
        &item_id,
        "create",
        serde_json::json!({
            "id": item_id,
            "list_id": list_id,
            "position": position,
            "entry": {
                "traditional": entry.traditional,
                "simplified": entry.simplified,
                "pinyin": entry.pinyin_marks,
                "definitions": entry.definitions,
            }
        }),
    )?;
    tx.commit()?;
    Ok(ListItemView {
        item_id,
        position,
        sync_state: "pending".into(),
        entry,
    })
}

#[tauri::command]
pub fn remove_item(state: State<'_, AppState>, item_id: String) -> Result<(), AppError> {
    let conn = lock(&state.user)?;
    let tx = conn.unchecked_transaction()?;
    let row: Option<(String, Option<String>)> = tx
        .query_row(
            "SELECT list_id, remote_id FROM list_items WHERE id = ?1 AND deleted_at IS NULL",
            rusqlite::params![item_id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()?;
    let (list_id, remote_id) = row.ok_or_else(|| AppError::NotFound(format!("item {item_id}")))?;
    tx.execute(
        "UPDATE list_items SET deleted_at = ?2, sync_state = 'pending' WHERE id = ?1",
        rusqlite::params![item_id, now()],
    )?;
    touch_list(&tx, &list_id)?;
    queue_op(
        &tx,
        "item",
        &item_id,
        "delete",
        serde_json::json!({ "id": item_id, "list_id": list_id, "remote_id": remote_id }),
    )?;
    tx.commit()?;
    Ok(())
}

/// FR-8: explicit user-controlled ordering. `new_index` is the target index
/// within the list's current order; positions are fractional so a move only
/// rewrites one row.
#[tauri::command]
pub fn move_item(
    state: State<'_, AppState>,
    item_id: String,
    new_index: usize,
) -> Result<(), AppError> {
    let conn = lock(&state.user)?;
    let tx = conn.unchecked_transaction()?;

    let list_id: Option<String> = tx
        .query_row(
            "SELECT list_id FROM list_items WHERE id = ?1 AND deleted_at IS NULL",
            rusqlite::params![item_id],
            |r| r.get(0),
        )
        .optional()?;
    let list_id = list_id.ok_or_else(|| AppError::NotFound(format!("item {item_id}")))?;

    let mut stmt = tx.prepare(
        "SELECT id, position FROM list_items \
         WHERE list_id = ?1 AND deleted_at IS NULL AND id != ?2 \
         ORDER BY position, added_at",
    )?;
    let others: Vec<(String, f64)> = stmt
        .query_map(rusqlite::params![list_id, item_id], |r| Ok((r.get(0)?, r.get(1)?)))?
        .collect::<Result<Vec<_>, _>>()?;
    drop(stmt);

    let idx = new_index.min(others.len());
    let new_position = if others.is_empty() {
        1.0
    } else if idx == 0 {
        others[0].1 - 1.0
    } else if idx >= others.len() {
        others[others.len() - 1].1 + 1.0
    } else {
        (others[idx - 1].1 + others[idx].1) / 2.0
    };

    tx.execute(
        "UPDATE list_items SET position = ?2 WHERE id = ?1",
        rusqlite::params![item_id, new_position],
    )?;
    mark_item_pending(&tx, &item_id)?;
    touch_list(&tx, &list_id)?;
    queue_op(
        &tx,
        "item",
        &item_id,
        "update",
        serde_json::json!({ "id": item_id, "list_id": list_id, "position": new_position }),
    )?;
    tx.commit()?;
    Ok(())
}

/// FR-11: export a list in order as TSV (Simplified, Traditional, Pinyin, English).
/// The path comes from the frontend's save-file dialog.
#[tauri::command]
pub fn export_list_tsv(
    state: State<'_, AppState>,
    list_id: String,
    path: String,
) -> Result<(), AppError> {
    let items = get_list_items(state.clone(), list_id)?;
    let mut out = String::from("Simplified\tTraditional\tPinyin\tEnglish\n");
    for item in items {
        let english = item.entry.definitions.replace(['\t', '\n'], " ");
        out.push_str(&format!(
            "{}\t{}\t{}\t{}\n",
            item.entry.simplified, item.entry.traditional, item.entry.pinyin_marks, english
        ));
    }
    std::fs::write(&path, out)?;
    Ok(())
}
