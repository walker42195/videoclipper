import { useEffect, useState } from "react";
import { open, save } from "@tauri-apps/plugin-dialog";
import { listen } from "@tauri-apps/api/event";
import { dirname } from "@tauri-apps/api/path";
import { useProjectStore } from "./state/projectStore";
import {
  availableDiskSpaceBytes,
  cancelExport,
  exportProject,
  loadProject,
  probeClip,
  renderPreview,
  saveProject,
} from "./lib/tauriCommands";
import { useBlobUrl } from "./lib/useBlobUrl";
import { Clip, DEFAULT_TEXT_CARD_SOURCE, ExportProgress, STILL_ITEM_DEFAULT_DURATION_SEC } from "./types";
import { Timeline } from "./components/Timeline/Timeline";
import { ClipPreviewPlayer } from "./components/Timeline/ClipPreviewPlayer";
import { MovieAudioPanel } from "./components/AudioPanel/MovieAudioPanel";
import { ExportPanel } from "./components/ExportPanel/ExportPanel";
import "./App.css";

const LAST_IMPORT_DIR_KEY = "videoclipper:lastImportDir";
const LAST_PROJECT_DIR_KEY = "videoclipper:lastProjectDir";
const LOW_DISK_WARNING_BYTES = 500 * 1024 * 1024;

function fileNameFromPath(path: string): string {
  return path.split(/[/\\]/).pop() ?? path;
}

function formatDuration(sec: number): string {
  const m = Math.floor(sec / 60);
  const s = Math.round(sec % 60);
  return `${m}:${s.toString().padStart(2, "0")}`;
}

function formatBytes(bytes: number): string {
  return `${(bytes / (1024 * 1024)).toFixed(0)} MB`;
}

function App() {
  const {
    clips,
    transitions,
    movieAudioOverride,
    introTransition,
    outroTransition,
    addClip,
    exportSettings,
    toProject,
    loadProject: hydrateProject,
  } = useProjectStore();
  const [busy, setBusy] = useState(false);
  const [statusMessage, setStatusMessage] = useState("");
  const [progress, setProgress] = useState<ExportProgress | null>(null);
  const [exportedMoviePath, setExportedMoviePath] = useState<string | null>(null);
  const [previewMoviePath, setPreviewMoviePath] = useState<string | null>(null);
  const [currentProjectPath, setCurrentProjectPath] = useState<string | null>(null);
  const exportedMovieUrl = useBlobUrl(exportedMoviePath);
  const previewMovieUrl = useBlobUrl(previewMoviePath);

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
      defaultPath: localStorage.getItem(LAST_IMPORT_DIR_KEY) ?? undefined,
      filters: [{ name: "Video", extensions: ["mp4", "mov", "mkv", "avi", "webm"] }],
    });
    if (!selected) return;
    const paths = Array.isArray(selected) ? selected : [selected];

    localStorage.setItem(LAST_IMPORT_DIR_KEY, await dirname(paths[0]));

    setStatusMessage(`Läser in ${paths.length} klipp...`);
    for (const path of paths) {
      try {
        const meta = await probeClip(path);
        const clip: Clip = {
          id: crypto.randomUUID(),
          source: { kind: "video" },
          sourcePath: path,
          sourceDurationSec: meta.durationSec,
          trimInSec: 0,
          trimOutSec: meta.durationSec,
          hasAudio: meta.hasAudio,
          audioOverride: null,
        };
        addClip(clip);
      } catch (err) {
        setStatusMessage(`Kunde inte läsa in ${fileNameFromPath(path)}: ${err}`);
      }
    }
    setStatusMessage("");
  }

  async function handleAddImage() {
    const selected = await open({
      multiple: true,
      defaultPath: localStorage.getItem(LAST_IMPORT_DIR_KEY) ?? undefined,
      filters: [{ name: "Bild", extensions: ["jpg", "jpeg", "png", "webp", "bmp"] }],
    });
    if (!selected) return;
    const paths = Array.isArray(selected) ? selected : [selected];
    localStorage.setItem(LAST_IMPORT_DIR_KEY, await dirname(paths[0]));

    for (const path of paths) {
      const clip: Clip = {
        id: crypto.randomUUID(),
        source: { kind: "image" },
        sourcePath: path,
        sourceDurationSec: STILL_ITEM_DEFAULT_DURATION_SEC,
        trimInSec: 0,
        trimOutSec: STILL_ITEM_DEFAULT_DURATION_SEC,
        hasAudio: false,
        audioOverride: null,
      };
      addClip(clip);
    }
  }

  function handleAddTextCard() {
    const clip: Clip = {
      id: crypto.randomUUID(),
      source: DEFAULT_TEXT_CARD_SOURCE,
      sourcePath: "",
      sourceDurationSec: STILL_ITEM_DEFAULT_DURATION_SEC,
      trimInSec: 0,
      trimOutSec: STILL_ITEM_DEFAULT_DURATION_SEC,
      hasAudio: false,
      audioOverride: null,
    };
    addClip(clip);
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
    setExportedMoviePath(null);
    setStatusMessage("Exporterar...");

    try {
      const freeBytes = await availableDiskSpaceBytes(outputPath);
      if (freeBytes < LOW_DISK_WARNING_BYTES) {
        setStatusMessage(`Varning: bara ${formatBytes(freeBytes)} ledigt diskutrymme. Fortsätter ändå...`);
      }
    } catch {
      // Advisory only - if the check itself fails, just proceed with export.
    }

    try {
      const result = await exportProject(
        clips,
        transitions,
        movieAudioOverride,
        introTransition,
        outroTransition,
        exportSettings,
        outputPath,
      );
      setStatusMessage(`Klart! Sparad till ${result.outputPath}`);
      setExportedMoviePath(result.outputPath);
    } catch (err) {
      if (String(err) === "cancelled") {
        setStatusMessage("Export avbruten.");
      } else {
        setStatusMessage(`Export misslyckades:\n${err}`);
      }
    } finally {
      setBusy(false);
      setProgress(null);
    }
  }

  async function handleRenderPreview() {
    if (clips.length === 0) {
      setStatusMessage("Lägg till minst ett klipp innan förhandsgranskning.");
      return;
    }
    setBusy(true);
    setProgress(null);
    setStatusMessage("Renderar förhandsvisning...");

    try {
      const result = await renderPreview(clips, transitions, movieAudioOverride, introTransition, outroTransition);
      setPreviewMoviePath(result.outputPath);
      setStatusMessage("Förhandsvisning klar.");
    } catch (err) {
      if (String(err) === "cancelled") {
        setStatusMessage("Förhandsvisning avbruten.");
      } else {
        setStatusMessage(`Förhandsvisning misslyckades:\n${err}`);
      }
    } finally {
      setBusy(false);
      setProgress(null);
    }
  }

  async function handleCancelExport() {
    setStatusMessage("Avbryter...");
    try {
      await cancelExport();
    } catch (err) {
      setStatusMessage(`Kunde inte avbryta: ${err}`);
    }
  }

  async function handleSaveProject(forcePrompt: boolean) {
    let path = forcePrompt ? null : currentProjectPath;
    if (!path) {
      path = await save({
        defaultPath: localStorage.getItem(LAST_PROJECT_DIR_KEY)
          ? `${localStorage.getItem(LAST_PROJECT_DIR_KEY)}/projekt.vcproj.json`
          : "projekt.vcproj.json",
        filters: [{ name: "VideoClipper-projekt", extensions: ["json"] }],
      });
      if (!path) return;
    }
    try {
      await saveProject(path, toProject());
      setCurrentProjectPath(path);
      localStorage.setItem(LAST_PROJECT_DIR_KEY, await dirname(path));
      setStatusMessage(`Projekt sparat till ${path}`);
    } catch (err) {
      setStatusMessage(`Kunde inte spara projekt: ${err}`);
    }
  }

  async function handleOpenProject() {
    const selected = await open({
      multiple: false,
      defaultPath: localStorage.getItem(LAST_PROJECT_DIR_KEY) ?? undefined,
      filters: [{ name: "VideoClipper-projekt", extensions: ["json"] }],
    });
    if (!selected || Array.isArray(selected)) return;

    try {
      const result = await loadProject(selected);
      hydrateProject(result.project);
      setCurrentProjectPath(selected);
      localStorage.setItem(LAST_PROJECT_DIR_KEY, await dirname(selected));
      setExportedMoviePath(null);
      setPreviewMoviePath(null);
      if (result.missingMedia.length > 0) {
        setStatusMessage(
          `Projekt öppnat, men ${result.missingMedia.length} klipp saknas på disk: ${result.missingMedia
            .map(fileNameFromPath)
            .join(", ")}`,
        );
      } else {
        setStatusMessage(`Projekt öppnat: ${result.project.name}`);
      }
    } catch (err) {
      setStatusMessage(`Kunde inte öppna projekt: ${err}`);
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
        <button onClick={handleAddImage} disabled={busy}>
          + Stillbild
        </button>
        <button onClick={handleAddTextCard} disabled={busy}>
          + Textkort
        </button>
        {!busy ? (
          <>
            <button onClick={handleRenderPreview} disabled={clips.length === 0}>
              Förhandsgranska film
            </button>
            <button onClick={handleExport} disabled={clips.length === 0}>
              Exportera film
            </button>
          </>
        ) : (
          <button onClick={handleCancelExport}>Avbryt</button>
        )}
        <button onClick={() => handleSaveProject(false)} disabled={busy}>
          Spara projekt
        </button>
        <button onClick={() => handleSaveProject(true)} disabled={busy}>
          Spara som...
        </button>
        <button onClick={handleOpenProject} disabled={busy}>
          Öppna projekt
        </button>
        <span className="total-duration">Total längd: {formatDuration(totalDuration)}</span>
      </div>

      <ClipPreviewPlayer />

      <Timeline />

      <MovieAudioPanel />

      <ExportPanel />

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

      {statusMessage &&
        (statusMessage.includes("\n") ? (
          <pre className="status-message-pre">{statusMessage}</pre>
        ) : (
          <p className="status-message">{statusMessage}</p>
        ))}

      {previewMovieUrl && (
        <div className="movie-player">
          <p className="movie-player-label">Förhandsgranskning (lågkvalitet, med övergångar)</p>
          <video controls autoPlay src={previewMovieUrl} />
        </div>
      )}

      {exportedMovieUrl && (
        <div className="movie-player">
          <p className="movie-player-label">Exporterad film</p>
          <video controls src={exportedMovieUrl} />
        </div>
      )}
    </main>
  );
}

export default App;
