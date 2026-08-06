mod commands;
mod ffmpeg;
mod model;

use commands::export::export_project;
use commands::probe::{ffmpeg_version, probe_clip};
use commands::project::{load_project, save_project};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .invoke_handler(tauri::generate_handler![
            ffmpeg_version,
            probe_clip,
            export_project,
            save_project,
            load_project,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
