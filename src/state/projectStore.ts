import { create } from "zustand";
import { Clip, DEFAULT_EXPORT_SETTINGS, ExportSettings } from "../types";

interface ProjectState {
  name: string;
  clips: Clip[];
  exportSettings: ExportSettings;
  addClip: (clip: Clip) => void;
  removeClip: (id: string) => void;
  moveClip: (fromIndex: number, toIndex: number) => void;
  updateClipTrim: (id: string, trimInSec: number, trimOutSec: number) => void;
  setExportSettings: (settings: ExportSettings) => void;
}

export const useProjectStore = create<ProjectState>((set) => ({
  name: "Namnlöst projekt",
  clips: [],
  exportSettings: DEFAULT_EXPORT_SETTINGS,
  addClip: (clip) => set((s) => ({ clips: [...s.clips, clip] })),
  removeClip: (id) => set((s) => ({ clips: s.clips.filter((c) => c.id !== id) })),
  moveClip: (fromIndex, toIndex) =>
    set((s) => {
      const next = [...s.clips];
      const [moved] = next.splice(fromIndex, 1);
      next.splice(toIndex, 0, moved);
      return { clips: next };
    }),
  updateClipTrim: (id, trimInSec, trimOutSec) =>
    set((s) => ({
      clips: s.clips.map((c) => (c.id === id ? { ...c, trimInSec, trimOutSec } : c)),
    })),
  setExportSettings: (exportSettings) => set({ exportSettings }),
}));
