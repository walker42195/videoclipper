import { invoke } from "@tauri-apps/api/core";
import type { Clip, ClipMeta, ExportSettings, LoadProjectResultDto, Project, Transition } from "../types";

export function ffmpegVersion(): Promise<string> {
  return invoke("ffmpeg_version");
}

export function probeClip(path: string): Promise<ClipMeta> {
  return invoke("probe_clip", { path });
}

export function extractThumbnail(path: string, atSeconds: number): Promise<string> {
  return invoke("extract_thumbnail", { path, atSeconds });
}

export interface ExportResult {
  outputPath: string;
}

export function exportProject(
  clips: Clip[],
  transitions: Transition[],
  exportSettings: ExportSettings,
  outputPath: string,
): Promise<ExportResult> {
  return invoke("export_project", {
    request: { clips, transitions, exportSettings, outputPath },
  });
}

export function saveProject(path: string, project: Project): Promise<void> {
  return invoke("save_project", { path, project });
}

export function loadProject(path: string): Promise<LoadProjectResultDto> {
  return invoke("load_project", { path });
}
