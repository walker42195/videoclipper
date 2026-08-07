mod commands;
mod ffmpeg;
mod model;

use commands::export::{cancel_export, export_project, ExportHandle};
use commands::probe::{extract_thumbnail, ffmpeg_version, probe_clip};
use commands::project::{available_disk_space_bytes, load_project, save_project};

/// Works around a WebKitGTK/NVIDIA/Wayland crash on launch
/// ("Gdk-Message: Error 71 (Protocol error) dispatching to Wayland
/// display."), caused by the DMA-BUF renderer requesting buffer formats the
/// NVIDIA driver doesn't provide under explicit sync. Per Tauri's own Linux
/// graphics debugging guide this has no performance cost when unneeded, so
/// it's set unconditionally on Linux rather than trying to detect the GPU
/// vendor/compositor first. See https://v2.tauri.app/develop/debug/linux-graphics/
#[cfg(target_os = "linux")]
fn apply_linux_webkit_workarounds() {
    std::env::set_var("__NV_DISABLE_EXPLICIT_SYNC", "1");
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    #[cfg(target_os = "linux")]
    apply_linux_webkit_workarounds();

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .manage(ExportHandle::default())
        .invoke_handler(tauri::generate_handler![
            ffmpeg_version,
            probe_clip,
            extract_thumbnail,
            export_project,
            cancel_export,
            save_project,
            load_project,
            available_disk_space_bytes,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
