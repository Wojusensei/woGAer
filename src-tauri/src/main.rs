#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

mod db;

use tauri::Manager;
use std::fs::create_dir_all;

fn main() {
    tauri::Builder::default()
        .setup(|app| {
            let app_handle = app.handle();
            let app_data_dir = app_handle.path().app_data_dir().unwrap();
            create_dir_all(&app_data_dir).unwrap();
            let db = db::Database::new(app_data_dir).unwrap();
            app.manage(db);
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}