//! Native application menu. Tauri's default menu binds ⌘W to "close window", which we
//! want for closing tabs instead, so the menu is built by hand.

use tauri::menu::{Menu, MenuItem, PredefinedMenuItem, Submenu};
use tauri::{App, Emitter};

pub fn install(app: &mut App) -> tauri::Result<()> {
    let handle = app.handle();

    let new_note = MenuItem::with_id(handle, "new-note", "New Note", true, Some("CmdOrCtrl+N"))?;
    let close_tab = MenuItem::with_id(handle, "close-tab", "Close Tab", true, Some("CmdOrCtrl+W"))?;
    let save = MenuItem::with_id(handle, "save", "Save", true, Some("CmdOrCtrl+S"))?;
    let file = Submenu::with_items(
        handle,
        "File",
        true,
        &[
            &new_note,
            &close_tab,
            &PredefinedMenuItem::separator(handle)?,
            &save,
        ],
    )?;

    let edit = Submenu::with_items(
        handle,
        "Edit",
        true,
        &[
            &PredefinedMenuItem::undo(handle, None)?,
            &PredefinedMenuItem::redo(handle, None)?,
            &PredefinedMenuItem::separator(handle)?,
            &PredefinedMenuItem::cut(handle, None)?,
            &PredefinedMenuItem::copy(handle, None)?,
            &PredefinedMenuItem::paste(handle, None)?,
            &PredefinedMenuItem::select_all(handle, None)?,
        ],
    )?;

    let window = Submenu::with_items(
        handle,
        "Window",
        true,
        &[
            &PredefinedMenuItem::minimize(handle, None)?,
            &PredefinedMenuItem::maximize(handle, None)?,
        ],
    )?;

    #[cfg(target_os = "macos")]
    let menu = {
        let app_menu = Submenu::with_items(
            handle,
            "Math Note",
            true,
            &[
                &PredefinedMenuItem::about(handle, None, None)?,
                &PredefinedMenuItem::separator(handle)?,
                &PredefinedMenuItem::services(handle, None)?,
                &PredefinedMenuItem::separator(handle)?,
                &PredefinedMenuItem::hide(handle, None)?,
                &PredefinedMenuItem::hide_others(handle, None)?,
                &PredefinedMenuItem::show_all(handle, None)?,
                &PredefinedMenuItem::separator(handle)?,
                &PredefinedMenuItem::quit(handle, None)?,
            ],
        )?;
        Menu::with_items(handle, &[&app_menu, &file, &edit, &window])?
    };

    #[cfg(not(target_os = "macos"))]
    let menu = {
        file.append(&PredefinedMenuItem::separator(handle)?)?;
        file.append(&PredefinedMenuItem::quit(handle, None)?)?;
        Menu::with_items(handle, &[&file, &edit, &window])?
    };

    app.set_menu(menu)?;
    app.on_menu_event(|app, event| {
        let action = match event.id().0.as_str() {
            "new-note" => "new-note",
            "close-tab" => "close-tab",
            "save" => "save",
            _ => return,
        };
        let _ = app.emit("menu", action);
    });
    Ok(())
}
