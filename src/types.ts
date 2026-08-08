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

/** What a timeline entry's video comes from - mirrors Rust's internally
 * tagged ClipSource enum (`{kind: "video"}` etc.). Image/TextCard clips
 * reuse every other Clip field: trimInSec is always 0 and trimOutSec is the
 * on-timeline display duration (there's no source file to trim within). */
export type ClipSource =
  | { kind: "video" }
  | { kind: "image" }
  | { kind: "textCard"; text: string; backgroundColor: string; fontColor: string };

export const DEFAULT_TEXT_CARD_SOURCE: ClipSource = {
  kind: "textCard",
  text: "",
  backgroundColor: "#000000",
  fontColor: "#FFFFFF",
};

/** Practical UI-only duration cap for Image/TextCard clips (drag handles,
 * number inputs) - unlike Video, there's no real source file duration to
 * bound them by, and the Rust backend places no limit here at all. */
export const STILL_ITEM_MAX_DURATION_SEC = 60;
export const STILL_ITEM_DEFAULT_DURATION_SEC = 3;

export interface Clip {
  id: string;
  source: ClipSource;
  /** Video/Image: path to the source file. TextCard: unused (empty string). */
  sourcePath: string;
  /** Full duration of the source file - the upper bound trimOutSec can't exceed.
   * Not meaningful for Image/TextCard. */
  sourceDurationSec: number;
  trimInSec: number;
  trimOutSec: number;
  /** Whether the source file has an audio stream at all (from probeClip).
   * Always false for Image/TextCard. */
  hasAudio: boolean;
  audioOverride: AudioOverride | null;
}

// Mirrors src-tauri/src/model.rs's TransitionType exactly (byte-identical
// variant spelling, since serde serializes the Rust enum variant name as-is
// with no rename_all) - kept in sync with ffmpeg's xfade filter's built-in
// transition list (`ffmpeg -h filter=xfade`, indices 0-57).
export type TransitionType =
  | "Fade"
  | "Wipeleft"
  | "Wiperight"
  | "Wipeup"
  | "Wipedown"
  | "Slideleft"
  | "Slideright"
  | "Slideup"
  | "Slidedown"
  | "Circlecrop"
  | "Rectcrop"
  | "Distance"
  | "Fadeblack"
  | "Fadewhite"
  | "Radial"
  | "Smoothleft"
  | "Smoothright"
  | "Smoothup"
  | "Smoothdown"
  | "Circleopen"
  | "Circleclose"
  | "Vertopen"
  | "Vertclose"
  | "Horzopen"
  | "Horzclose"
  | "Dissolve"
  | "Pixelize"
  | "Diagtl"
  | "Diagtr"
  | "Diagbl"
  | "Diagbr"
  | "Hlslice"
  | "Hrslice"
  | "Vuslice"
  | "Vdslice"
  | "Hblur"
  | "Fadegrays"
  | "Wipetl"
  | "Wipetr"
  | "Wipebl"
  | "Wipebr"
  | "Squeezeh"
  | "Squeezev"
  | "Zoomin"
  | "Fadefast"
  | "Fadeslow"
  | "Hlwind"
  | "Hrwind"
  | "Vuwind"
  | "Vdwind"
  | "Coverleft"
  | "Coverright"
  | "Coverup"
  | "Coverdown"
  | "Revealleft"
  | "Revealright"
  | "Revealup"
  | "Revealdown";

export const TRANSITION_LABELS: Record<TransitionType, string> = {
  Fade: "Tona",
  Dissolve: "Upplösning",
  Fadeblack: "Tona till svart → in",
  Fadewhite: "Tona till vitt → in",
  Fadegrays: "Tona till gråskala",
  Fadefast: "Snabb tona",
  Fadeslow: "Långsam tona",
  Distance: "Avståndstona",
  Radial: "Radiell tona (urverk)",
  Hblur: "Suddig tona",
  Pixelize: "Pixlig tona",
  Wipeleft: "Svep vänster",
  Wiperight: "Svep höger",
  Wipeup: "Svep upp",
  Wipedown: "Svep ner",
  Wipetl: "Svep uppåt-vänster",
  Wipetr: "Svep uppåt-höger",
  Wipebl: "Svep nedåt-vänster",
  Wipebr: "Svep nedåt-höger",
  Slideleft: "Skjut vänster",
  Slideright: "Skjut höger",
  Slideup: "Skjut upp",
  Slidedown: "Skjut ner",
  Smoothleft: "Mjuk vänster",
  Smoothright: "Mjuk höger",
  Smoothup: "Mjuk upp",
  Smoothdown: "Mjuk ner",
  Circleopen: "Cirkelöppning",
  Circleclose: "Cirkelstängning",
  Circlecrop: "Cirkelbeskärning",
  Rectcrop: "Rektangelbeskärning",
  Vertopen: "Vertikal öppning",
  Vertclose: "Vertikal stängning",
  Horzopen: "Horisontell öppning",
  Horzclose: "Horisontell stängning",
  Diagtl: "Diagonal uppåt-vänster",
  Diagtr: "Diagonal uppåt-höger",
  Diagbl: "Diagonal nedåt-vänster",
  Diagbr: "Diagonal nedåt-höger",
  Hlslice: "Horisontella remsor (vänster)",
  Hrslice: "Horisontella remsor (höger)",
  Vuslice: "Vertikala remsor (upp)",
  Vdslice: "Vertikala remsor (ner)",
  Hlwind: "Vindremsor (vänster)",
  Hrwind: "Vindremsor (höger)",
  Vuwind: "Vindremsor (upp)",
  Vdwind: "Vindremsor (ner)",
  Squeezeh: "Klämma ihop horisontellt",
  Squeezev: "Klämma ihop vertikalt",
  Zoomin: "Zooma in",
  Coverleft: "Täck från vänster",
  Coverright: "Täck från höger",
  Coverup: "Täck uppifrån",
  Coverdown: "Täck nerifrån",
  Revealleft: "Avslöja åt vänster",
  Revealright: "Avslöja åt höger",
  Revealup: "Avslöja uppåt",
  Revealdown: "Avslöja nedåt",
};

/** Groups transitions into UI categories so the picker doesn't dump 58
 * options into one flat, unnavigable grid. */
export const TRANSITION_GROUPS: { label: string; types: TransitionType[] }[] = [
  { label: "Tona", types: ["Fade", "Dissolve", "Fadeblack", "Fadewhite", "Fadegrays", "Fadefast", "Fadeslow", "Distance", "Radial", "Hblur", "Pixelize"] },
  { label: "Svep", types: ["Wipeleft", "Wiperight", "Wipeup", "Wipedown", "Wipetl", "Wipetr", "Wipebl", "Wipebr"] },
  { label: "Skjut", types: ["Slideleft", "Slideright", "Slideup", "Slidedown"] },
  { label: "Mjuk övergång", types: ["Smoothleft", "Smoothright", "Smoothup", "Smoothdown"] },
  { label: "Iris / ram", types: ["Circleopen", "Circleclose", "Circlecrop", "Rectcrop", "Vertopen", "Vertclose", "Horzopen", "Horzclose"] },
  { label: "Diagonal", types: ["Diagtl", "Diagtr", "Diagbl", "Diagbr"] },
  { label: "Remsor", types: ["Hlslice", "Hrslice", "Vuslice", "Vdslice", "Hlwind", "Hrwind", "Vuwind", "Vdwind"] },
  { label: "Zoom / klämma", types: ["Squeezeh", "Squeezev", "Zoomin"] },
  { label: "Täck / avslöja (scenbyte)", types: ["Coverleft", "Coverright", "Coverup", "Coverdown", "Revealleft", "Revealright", "Revealup", "Revealdown"] },
];

export interface Transition {
  fromClipId: string;
  toClipId: string;
  type: TransitionType;
  durationSec: number;
}

/** A transition at the very start/end of the whole timeline, against a
 * synthesized black clip rather than a second real clip - mirrors Rust's
 * EdgeTransition. Unlike Transition, needs no fromClipId/toClipId since
 * there's only ever one "other side": the timeline's own edge. */
export interface EdgeTransition {
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
  introTransition: EdgeTransition | null;
  outroTransition: EdgeTransition | null;
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
