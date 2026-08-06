import { open } from "@tauri-apps/plugin-dialog";
import { useProjectStore } from "../../state/projectStore";
import { AUDIO_BLEND_MODE_LABELS, AudioBlendMode, DEFAULT_MOVIE_AUDIO } from "../../types";

function fileNameFromPath(path: string): string {
  return path.split(/[/\\]/).pop() ?? path;
}

export function MovieAudioPanel() {
  const { movieAudioOverride, setMovieAudioOverride } = useProjectStore();

  async function handlePickMusic() {
    const selected = await open({
      multiple: false,
      filters: [{ name: "Ljud", extensions: ["mp3", "m4a", "aac", "wav", "flac", "ogg"] }],
    });
    if (!selected || Array.isArray(selected)) return;
    setMovieAudioOverride({ ...DEFAULT_MOVIE_AUDIO, sourcePath: selected });
  }

  if (!movieAudioOverride) {
    return (
      <div className="audio-panel">
        <button onClick={handlePickMusic}>+ Lägg till bakgrundsmusik</button>
      </div>
    );
  }

  return (
    <div className="audio-panel">
      <div className="audio-panel-header">
        <span className="audio-file-name">🎵 {fileNameFromPath(movieAudioOverride.sourcePath)}</span>
        <button onClick={handlePickMusic}>Byt fil</button>
        <button onClick={() => setMovieAudioOverride(null)}>Ta bort</button>
      </div>

      <div className="audio-panel-controls">
        <label className="audio-blend-mode">
          {(Object.keys(AUDIO_BLEND_MODE_LABELS) as AudioBlendMode[]).map((mode) => (
            <span key={mode}>
              <input
                type="radio"
                name="blendMode"
                checked={movieAudioOverride.blendMode === mode}
                onChange={() => setMovieAudioOverride({ ...movieAudioOverride, blendMode: mode })}
              />
              {AUDIO_BLEND_MODE_LABELS[mode]}
            </span>
          ))}
        </label>

        <label className="audio-slider-label">
          Volym: {movieAudioOverride.gainDb.toFixed(0)} dB
          <input
            type="range"
            min={-40}
            max={6}
            step={1}
            value={movieAudioOverride.gainDb}
            onChange={(e) =>
              setMovieAudioOverride({ ...movieAudioOverride, gainDb: parseFloat(e.target.value) })
            }
          />
        </label>

        <div className="audio-fade-inputs">
          <label>
            Tona in (s)
            <input
              type="number"
              min={0}
              step={0.5}
              value={movieAudioOverride.fadeInSec}
              onChange={(e) =>
                setMovieAudioOverride({ ...movieAudioOverride, fadeInSec: parseFloat(e.target.value) || 0 })
              }
            />
          </label>
          <label>
            Tona ut (s)
            <input
              type="number"
              min={0}
              step={0.5}
              value={movieAudioOverride.fadeOutSec}
              onChange={(e) =>
                setMovieAudioOverride({ ...movieAudioOverride, fadeOutSec: parseFloat(e.target.value) || 0 })
              }
            />
          </label>
        </div>
      </div>
    </div>
  );
}
