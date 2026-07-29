//! Pistachio Dictionary (开心果词典) — offline Chinese-English dictionary
//! with ordered word lists and sync to Bingqilin.
//!
//! Spec: "Offline Chinese-English Dictionary — Spec Sheet v0.1"

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod db;
mod error;
mod lists;
mod search;
mod sync;

use tauri::{Manager, State};
use tauri_plugin_dialog::DialogExt;

use db::{lock, AppState};
use error::AppError;
use search::{EntrySummary, Segment};

#[tauri::command]
fn search_entries(
    state: State<'_, AppState>,
    query: String,
    limit: Option<usize>,
) -> Result<Vec<EntrySummary>, AppError> {
    let conn = lock(&state.dict)?;
    search::search(&conn, &query, limit.unwrap_or(50))
}

#[tauri::command]
fn get_entry(state: State<'_, AppState>, id: i64) -> Result<EntrySummary, AppError> {
    let conn = lock(&state.dict)?;
    search::get_entry(&conn, id)
}

#[tauri::command]
fn segment_text(state: State<'_, AppState>, text: String) -> Result<Vec<Segment>, AppError> {
    let conn = lock(&state.dict)?;
    search::segment_lookup(&conn, &text, 8)
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .setup(|app| {
            match db::init(app.handle()) {
                Ok(state) => {
                    app.manage(state);
                    Ok(())
                }
                Err(e) => {
                    // Release builds have no console: surface startup failures
                    // in a log file and a native dialog instead of exiting
                    // silently (the "window flashes and closes" symptom).
                    let detail = e.to_string();
                    let mut log_desc = String::from("the app data folder");
                    if let Ok(dir) = app.path().app_data_dir() {
                        let _ = std::fs::create_dir_all(&dir);
                        let log_path = dir.join("startup-error.log");
                        let _ = std::fs::write(
                            &log_path,
                            format!("Pistachio Dictionary failed to start:\n\n{detail}\n"),
                        );
                        log_desc = log_path.display().to_string();
                    }
                    app.dialog()
                        .message(format!(
                            "Pistachio Dictionary could not start.\n\n{detail}\n\nDetails: {log_desc}"
                        ))
                        .title("Pistachio Dictionary — startup error")
                        .blocking_show();
                    Err(e.into())
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            // dictionary
            search_entries,
            get_entry,
            segment_text,
            // word lists
            lists::get_lists,
            lists::create_list,
            lists::rename_list,
            lists::delete_list,
            lists::get_list_items,
            lists::add_to_list,
            lists::remove_item,
            lists::move_item,
            lists::export_list_tsv,
            // sync
            sync::engine::sync_now,
            sync::engine::get_sync_settings,
            sync::engine::set_sync_settings,
            sync::engine::get_sync_status,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Pistachio Dictionary");
}
