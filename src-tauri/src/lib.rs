mod db;
pub mod math;
mod menu;

use std::time::Duration;

use db::{Db, Note, NoteMeta};
use tauri::{AppHandle, Emitter, Manager, State};

#[tauri::command]
fn evaluate(content: String) -> math::NoteEvaluation {
    math::evaluate_note(&content)
}

#[tauri::command]
fn list_notes(db: State<'_, Db>) -> Result<Vec<NoteMeta>, String> {
    db.list().map_err(|e| e.to_string())
}

#[tauri::command]
fn create_note(db: State<'_, Db>) -> Result<Note, String> {
    db.create().map_err(|e| e.to_string())
}

#[tauri::command]
fn get_note(db: State<'_, Db>, id: i64) -> Result<Note, String> {
    db.get(id).map_err(|e| e.to_string())
}

#[tauri::command]
fn save_note(
    db: State<'_, Db>,
    id: i64,
    title: String,
    content: String,
) -> Result<NoteMeta, String> {
    db.save(id, &title, &content).map_err(|e| e.to_string())
}

#[tauri::command]
fn delete_note(db: State<'_, Db>, id: i64) -> Result<(), String> {
    db.delete(id).map_err(|e| e.to_string())
}

/// Called by the frontend once unsaved notes have been flushed after a close request.
#[tauri::command]
fn finish_close(app: AppHandle) {
    app.exit(0);
}

/// Asks the frontend to save everything, then exits. If the frontend never answers
/// (for example because the webview is gone), exit anyway after a short grace period.
fn request_graceful_exit(app: &AppHandle) {
    let _ = app.emit("app-close-requested", ());
    let handle = app.clone();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_secs(3));
        handle.exit(0);
    });
}

pub fn run() {
    let app = tauri::Builder::default()
        .setup(|app| {
            let data_dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&data_dir)?;
            let db = Db::open(data_dir.join("notes.db"))?;
            app.manage(db);
            menu::install(app)?;
            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                request_graceful_exit(window.app_handle());
            }
        })
        .invoke_handler(tauri::generate_handler![
            evaluate,
            list_notes,
            create_note,
            get_note,
            save_note,
            delete_note,
            finish_close
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application");

    app.run(|app, event| {
        // `code` is Some when we called `app.exit` ourselves (after saving), None when
        // the OS or the Quit menu item asked us to leave.
        if let tauri::RunEvent::ExitRequested { api, code, .. } = event {
            if code.is_none() {
                api.prevent_exit();
                request_graceful_exit(app);
            }
        }
    });
}
