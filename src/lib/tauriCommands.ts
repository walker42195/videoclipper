import { invoke } from "@tauri-apps/api/core";
import type {
  Clip,
  ClipMeta,
  ExportSettings,
  LoadProjectResultDto,
  MovieAudioOverride,
  Project,
  Transition,
} from "../types";

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
  movieAudioOverride: MovieAudioOverride | null,
  introFadeSec: number,
  outroFadeSec: number,
  exportSettings: ExportSettings,
  outputPath: string,
): Promise<ExportResult> {
  return invoke("export_project", {
    request: { clips, transitions, movieAudioOverride, introFadeSec, outroFadeSec, exportSettings, outputPath },
  });
}

/** Renders the whole timeline (clips + transitions + edge fades + movie
 * audio) at a small/fast preset to a temp file, so it can be played back
 * in-app before committing to a full-quality export. */
export function renderPreview(
  clips: Clip[],
  transitions: Transition[],
  movieAudioOverride: MovieAudioOverride | null,
  introFadeSec: number,
  outroFadeSec: number,
): Promise<ExportResult> {
  return invoke("render_preview", {
    request: { clips, transitions, movieAudioOverride, introFadeSec, outroFadeSec },
  });
}

export function cancelExport(): Promise<void> {
  return invoke("cancel_export");
}

export function saveProject(path: string, project: Project): Promise<void> {
  return invoke("save_project", { path, project });
}

export function loadProject(path: string): Promise<LoadProjectResultDto> {
  return invoke("load_project", { path });
}

export function availableDiskSpaceBytes(path: string): Promise<number> {
  return invoke("available_disk_space_bytes", { path });
}
