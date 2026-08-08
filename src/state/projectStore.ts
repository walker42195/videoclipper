import { create } from "zustand";
import {
  AudioOverride,
  Clip,
  ClipSource,
  DEFAULT_EXPORT_SETTINGS,
  EdgeTransition,
  ExportSettings,
  MovieAudioOverride,
  Project,
  Transition,
  TransitionType,
} from "../types";

/** Drop transitions whose two clip ids are no longer adjacent in `clips` -
 * call this after every reorder/removal so stale junctions don't linger. */
function pruneInvalidTransitions(clips: Clip[], transitions: Transition[]): Transition[] {
  const adjacentPairs = new Set<string>();
  for (let i = 0; i < clips.length - 1; i++) {
    adjacentPairs.add(`${clips[i].id}->${clips[i + 1].id}`);
  }
  return transitions.filter((t) => adjacentPairs.has(`${t.fromClipId}->${t.toClipId}`));
}

interface ProjectState {
  id: string;
  name: string;
  clips: Clip[];
  transitions: Transition[];
  movieAudioOverride: MovieAudioOverride | null;
  exportSettings: ExportSettings;
  introTransition: EdgeTransition | null;
  outroTransition: EdgeTransition | null;
  /** Which clip is currently shown in the preview player. */
  previewClipId: string | null;
  addClip: (clip: Clip) => void;
  removeClip: (id: string) => void;
  moveClip: (fromIndex: number, toIndex: number) => void;
  updateClipTrim: (id: string, trimInSec: number, trimOutSec: number) => void;
  updateClipAudioOverride: (id: string, override: AudioOverride | null) => void;
  updateClipSource: (id: string, source: ClipSource) => void;
  setTransition: (fromClipId: string, toClipId: string, type: TransitionType, durationSec: number) => void;
  clearTransition: (fromClipId: string, toClipId: string) => void;
  setMovieAudioOverride: (override: MovieAudioOverride | null) => void;
  setExportSettings: (settings: ExportSettings) => void;
  setIntroTransition: (transition: EdgeTransition | null) => void;
  setOutroTransition: (transition: EdgeTransition | null) => void;
  setPreviewClipId: (id: string | null) => void;
  setName: (name: string) => void;
  /** Replaces the whole project state at once (used by "Öppna projekt"). */
  loadProject: (project: Project) => void;
  /** Snapshots the current store into a serializable Project (used by "Spara projekt"). */
  toProject: () => Project;
}

export const useProjectStore = create<ProjectState>((set, get) => ({
  id: crypto.randomUUID(),
  name: "Namnlöst projekt",
  clips: [],
  transitions: [],
  movieAudioOverride: null,
  exportSettings: DEFAULT_EXPORT_SETTINGS,
  introTransition: null,
  outroTransition: null,
  previewClipId: null,
  // Preview the clip you just added so you can immediately check it looks right.
  addClip: (clip) => set((s) => ({ clips: [...s.clips, clip], previewClipId: clip.id })),
  removeClip: (id) =>
    set((s) => {
      const clips = s.clips.filter((c) => c.id !== id);
      return {
        clips,
        transitions: pruneInvalidTransitions(clips, s.transitions),
        previewClipId: s.previewClipId === id ? null : s.previewClipId,
      };
    }),
  moveClip: (fromIndex, toIndex) =>
    set((s) => {
      const clips = [...s.clips];
      const [moved] = clips.splice(fromIndex, 1);
      clips.splice(toIndex, 0, moved);
      return { clips, transitions: pruneInvalidTransitions(clips, s.transitions) };
    }),
  updateClipTrim: (id, trimInSec, trimOutSec) =>
    set((s) => ({
      clips: s.clips.map((c) => (c.id === id ? { ...c, trimInSec, trimOutSec } : c)),
    })),
  updateClipAudioOverride: (id, audioOverride) =>
    set((s) => ({
      clips: s.clips.map((c) => (c.id === id ? { ...c, audioOverride } : c)),
    })),
  updateClipSource: (id, source) =>
    set((s) => ({
      clips: s.clips.map((c) => (c.id === id ? { ...c, source } : c)),
    })),
  setTransition: (fromClipId, toClipId, type, durationSec) =>
    set((s) => ({
      transitions: [
        ...s.transitions.filter((t) => !(t.fromClipId === fromClipId && t.toClipId === toClipId)),
        { fromClipId, toClipId, type, durationSec },
      ],
    })),
  clearTransition: (fromClipId, toClipId) =>
    set((s) => ({
      transitions: s.transitions.filter((t) => !(t.fromClipId === fromClipId && t.toClipId === toClipId)),
    })),
  setMovieAudioOverride: (movieAudioOverride) => set({ movieAudioOverride }),
  setExportSettings: (exportSettings) => set({ exportSettings }),
  setIntroTransition: (introTransition) => set({ introTransition }),
  setOutroTransition: (outroTransition) => set({ outroTransition }),
  setPreviewClipId: (previewClipId) => set({ previewClipId }),
  setName: (name) => set({ name }),
  loadProject: (project) =>
    set({
      id: project.id,
      name: project.name,
      clips: project.clips,
      transitions: project.transitions,
      movieAudioOverride: project.movieAudioOverride,
      exportSettings: project.exportSettings,
      introTransition: project.introTransition,
      outroTransition: project.outroTransition,
      previewClipId: null,
    }),
  toProject: () => {
    const s = get();
    return {
      version: 1,
      id: s.id,
      name: s.name,
      clips: s.clips,
      transitions: s.transitions,
      movieAudioOverride: s.movieAudioOverride,
      exportSettings: s.exportSettings,
      introTransition: s.introTransition,
      outroTransition: s.outroTransition,
    };
  },
}));
