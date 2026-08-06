import { useEffect, useState } from "react";
import { open, save } from "@tauri-apps/plugin-dialog";
import { listen } from "@tauri-apps/api/event";
import { useProjectStore } from "./state/projectStore";
import { exportProject, probeClip } from "./lib/tauriCommands";
import { Clip, ExportProgress } from "./types";
import { Timeline } from "./components/Timeline/Timeline";
import "./App.css";

function fileNameFromPath(path: string): string {
  return path.split(/[/\\]/).pop() ?? path;
}

function formatDuration(sec: number): string {
  const m = Math.floor(sec / 60);
  const s = Math.round(sec % 60);
  return `${m}:${s.toString().padStart(2, "0")}`;
}

function App() {
  const { clips, addClip, exportSettings } = useProjectStore();
  const [busy, setBusy] = useState(false);
  const [statusMessage, setStatusMessage] = useState("");
  const [progress, setProgress] = useState<ExportProgress | null>(null);

  useEffect(() => {
    const unlisten = listen<ExportProgress>("export://progress", (event) => {
      setProgress(event.payload);
    });
    return () => {
      unlisten.then((f) => f());
    };
  }, []);

  async function handleAddClips() {
    const selected = await open({
      multiple: true,
      filters: [{ name: "Video", extensions: ["mp4", "mov", "mkv", "avi", "webm"] }],
    });
    if (!selected) return;
    const paths = Array.isArray(selected) ? selected : [selected];

    setStatusMessage(`Läser in ${paths.length} klipp...`);
    for (const path of paths) {
      try {
        const meta = await probeClip(path);
        const clip: Clip = {
          id: crypto.randomUUID(),
          sourcePath: path,
          sourceDurationSec: meta.durationSec,
          trimInSec: 0,
          trimOutSec: meta.durationSec,
          audioOverride: null,
        };
        addClip(clip);
      } catch (err) {
        setStatusMessage(`Kunde inte läsa in ${fileNameFromPath(path)}: ${err}`);
      }
    }
    setStatusMessage("");
  }

  async function handleExport() {
    if (clips.length === 0) {
      setStatusMessage("Lägg till minst ett klipp innan export.");
      return;
    }
    const outputPath = await save({
      defaultPath: "film.mp4",
      filters: [{ name: "MP4-video", extensions: ["mp4"] }],
    });
    if (!outputPath) return;

    setBusy(true);
    setProgress(null);
    setStatusMessage("Exporterar...");
    try {
      const result = await exportProject(clips, exportSettings, outputPath);
      setStatusMessage(`Klart! Sparad till ${result.outputPath}`);
    } catch (err) {
      setStatusMessage(`Export misslyckades: ${err}`);
    } finally {
      setBusy(false);
    }
  }

  const totalDuration = clips.reduce((sum, c) => sum + (c.trimOutSec - c.trimInSec), 0);

  return (
    <main className="container">
      <h1>VideoClipper</h1>

      <div className="toolbar">
        <button onClick={handleAddClips} disabled={busy}>
          + Lägg till klipp
        </button>
        <button onClick={handleExport} disabled={busy || clips.length === 0}>
          Exportera film
        </button>
        <span className="total-duration">Total längd: {formatDuration(totalDuration)}</span>
      </div>

      <Timeline />

      {progress && (
        <div className="progress-overlay">
          <div className="progress-bar-track">
            <div className="progress-bar-fill" style={{ width: `${progress.pct}%` }} />
          </div>
          <span>
            {progress.pct.toFixed(0)}% {progress.speed ? `(${progress.speed.toFixed(1)}x)` : ""}
          </span>
        </div>
      )}

      {statusMessage && <p className="status-message">{statusMessage}</p>}
    </main>
  );
}

export default App;
