//! Thin helpers around `tauri_plugin_shell`'s sidecar API for spawning the
//! bundled `ffmpeg`/`ffprobe` binaries and collecting their output.

use tauri::AppHandle;
use tauri_plugin_shell::{process::CommandEvent, ShellExt};

/// Run a sidecar to completion and return (stdout, stderr, success).
pub async fn run_sidecar_capture(
    app: &AppHandle,
    name: &str,
    args: Vec<String>,
) -> Result<(String, String, bool), String> {
    let shell = app.shell();
    let sidecar = shell
        .sidecar(name)
        .map_err(|e| format!("failed to resolve sidecar '{name}': {e}"))?;
    let (mut rx, _child) = sidecar
        .args(args)
        .spawn()
        .map_err(|e| format!("failed to spawn sidecar '{name}': {e}"))?;

    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let mut success = false;

    while let Some(event) = rx.recv().await {
        match event {
            CommandEvent::Stdout(bytes) => stdout.extend_from_slice(&bytes),
            CommandEvent::Stderr(bytes) => stderr.extend_from_slice(&bytes),
            CommandEvent::Terminated(payload) => {
                success = payload.code == Some(0);
            }
            _ => {}
        }
    }

    Ok((
        String::from_utf8_lossy(&stdout).to_string(),
        String::from_utf8_lossy(&stderr).to_string(),
        success,
    ))
}
