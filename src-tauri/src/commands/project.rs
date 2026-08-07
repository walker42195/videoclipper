use crate::model::Project;
use serde::Serialize;
use std::path::Path;

/// Free space, in bytes, on the filesystem holding `path`'s parent directory
/// (or `path` itself if it has no parent, e.g. a bare filename). Used as an
/// advisory pre-export check only - export sizes aren't known ahead of time,
/// so this can warn but never reliably block.
#[tauri::command]
pub fn available_disk_space_bytes(path: String) -> Result<u64, String> {
    let target = Path::new(&path);
    let dir = target.parent().filter(|p| !p.as_os_str().is_empty()).unwrap_or(target);
    fs4::available_space(dir).map_err(|e| format!("failed to read free space for '{path}': {e}"))
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LoadProjectResult {
    pub project: Project,
    /// sourcePaths referenced by the project that no longer exist on disk.
    pub missing_media: Vec<String>,
}

#[tauri::command]
pub fn save_project(path: String, project: Project) -> Result<(), String> {
    let json = serde_json::to_string_pretty(&project)
        .map_err(|e| format!("failed to serialize project: {e}"))?;
    std::fs::write(&path, json).map_err(|e| format!("failed to write '{path}': {e}"))
}

#[tauri::command]
pub fn load_project(path: String) -> Result<LoadProjectResult, String> {
    let json = std::fs::read_to_string(&path).map_err(|e| format!("failed to read '{path}': {e}"))?;
    let project: Project =
        serde_json::from_str(&json).map_err(|e| format!("failed to parse project JSON: {e}"))?;

    let missing_media = project
        .clips
        .iter()
        .filter(|c| !Path::new(&c.source_path).exists())
        .map(|c| c.source_path.clone())
        .collect();

    Ok(LoadProjectResult {
        project,
        missing_media,
    })
}
