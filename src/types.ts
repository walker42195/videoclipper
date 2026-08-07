// Mirrors src-tauri/src/model.rs. Kept in sync by hand for now; consider
// specta+tauri-specta later to generate this automatically.

export type AudioMode = "loop" | "pad";

export const AUDIO_MODE_LABELS: Record<AudioMode, string> = {
  loop: "Loopa",
  pad: "Tystnad i slutet",
};

export interface AudioOverride {
  sourcePath: string;
  mode: AudioMode;
  gainDb: number;
}

export const DEFAULT_CLIP_AUDIO_OVERRIDE: AudioOverride = {
  sourcePath: "",
  mode: "loop",
  gainDb: 0,
};

export interface Clip {
  id: string;
  sourcePath: string;
  /** Full duration of the source file - the upper bound trimOutSec can't exceed. */
  sourceDurationSec: number;
  trimInSec: number;
  trimOutSec: number;
  /** Whether the source file has an audio stream at all (from probeClip). */
  hasAudio: boolean;
  audioOverride: AudioOverride | null;
}

export type TransitionType =
  | "Fade"
  | "Dissolve"
  | "Fadeblack"
  | "Fadewhite"
  | "Wipeleft"
  | "Wiperight"
  | "Slideleft"
  | "Slideright"
  | "Circleopen"
  | "Smoothleft";

export const TRANSITION_LABELS: Record<TransitionType, string> = {
  Fade: "Tona",
  Dissolve: "Upplösning",
  Fadeblack: "Tona till svart",
  Fadewhite: "Tona till vitt",
  Wipeleft: "Svep vänster",
  Wiperight: "Svep höger",
  Slideleft: "Skjut vänster",
  Slideright: "Skjut höger",
  Circleopen: "Cirkelöppning",
  Smoothleft: "Mjuk vänster",
};

export interface Transition {
  fromClipId: string;
  toClipId: string;
  type: TransitionType;
  durationSec: number;
}

export type AudioBlendMode = "Replace" | "Mix";

export const AUDIO_BLEND_MODE_LABELS: Record<AudioBlendMode, string> = {
  Replace: "Ersätt allt ljud",
  Mix: "Mixa under befintligt ljud",
};

export interface MovieAudioOverride {
  sourcePath: string;
  blendMode: AudioBlendMode;
  fillMode: AudioMode;
  gainDb: number;
  fadeInSec: number;
  fadeOutSec: number;
}

export const DEFAULT_MOVIE_AUDIO: MovieAudioOverride = {
  sourcePath: "",
  blendMode: "Mix",
  fillMode: "loop",
  gainDb: -18,
  fadeInSec: 1.5,
  fadeOutSec: 2,
};

export interface ExportSettings {
  preset: string;
  container: string;
  videoCodec: string;
  crf: number;
  x264Preset: string;
  maxWidth: number;
  fps: number;
  audioCodec: string;
  audioBitrateKbps: number;
  faststart: boolean;
}

export const DEFAULT_EXPORT_SETTINGS: ExportSettings = {
  preset: "web-recommended",
  container: "mp4",
  videoCodec: "libx264",
  crf: 20,
  x264Preset: "medium",
  maxWidth: 1920,
  fps: 30,
  audioCodec: "aac",
  audioBitrateKbps: 192,
  faststart: true,
};

export interface ExportPreset {
  id: string;
  label: string;
  crf: number;
  x264Preset: string;
  audioBitrateKbps: number;
  maxWidth: number;
}

export const EXPORT_PRESETS: ExportPreset[] = [
  {
    id: "web-recommended",
    label: "Webb (rekommenderad)",
    crf: 20,
    x264Preset: "medium",
    audioBitrateKbps: 192,
    maxWidth: 1920,
  },
  {
    id: "web-fast",
    label: "Webb (mindre/snabbare)",
    crf: 26,
    x264Preset: "veryfast",
    audioBitrateKbps: 128,
    maxWidth: 1280,
  },
  {
    id: "web-hq",
    label: "Webb (hög kvalitet)",
    crf: 17,
    x264Preset: "slow",
    audioBitrateKbps: 256,
    maxWidth: 3840,
  },
];

export function applyExportPreset(settings: ExportSettings, presetId: string): ExportSettings {
  const preset = EXPORT_PRESETS.find((p) => p.id === presetId);
  if (!preset) return settings;
  return {
    ...settings,
    preset: preset.id,
    crf: preset.crf,
    x264Preset: preset.x264Preset,
    audioBitrateKbps: preset.audioBitrateKbps,
    maxWidth: preset.maxWidth,
  };
}

export interface Project {
  version: number;
  id: string;
  name: string;
  clips: Clip[];
  transitions: Transition[];
  movieAudioOverride: MovieAudioOverride | null;
  exportSettings: ExportSettings;
}

export interface ClipMeta {
  durationSec: number;
  width: number;
  height: number;
  fps: number;
  sampleRate: number;
  hasAudio: boolean;
}

export interface ExportProgress {
  pct: number;
  outTimeSec: number;
  speed: number | null;
  done: boolean;
}

export interface LoadProjectResultDto {
  project: Project;
  missingMedia: string[];
}
