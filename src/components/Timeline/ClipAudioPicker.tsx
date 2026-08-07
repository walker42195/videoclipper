import { open } from "@tauri-apps/plugin-dialog";
import { AUDIO_MODE_LABELS, AudioMode, AudioOverride, DEFAULT_CLIP_AUDIO_OVERRIDE } from "../../types";

function fileNameFromPath(path: string): string {
  return path.split(/[/\\]/).pop() ?? path;
}

interface ClipAudioPickerProps {
  current: AudioOverride | null;
  onChange: (override: AudioOverride | null) => void;
  onClose: () => void;
}

export function ClipAudioPicker({ current, onChange, onClose }: ClipAudioPickerProps) {
  async function handlePickFile() {
    const selected = await open({
      multiple: false,
      filters: [{ name: "Ljud", extensions: ["mp3", "m4a", "aac", "wav", "flac", "ogg"] }],
    });
    if (!selected || Array.isArray(selected)) return;
    onChange({ ...DEFAULT_CLIP_AUDIO_OVERRIDE, ...current, sourcePath: selected });
  }

  return (
    <div className="transition-picker clip-audio-picker" onPointerDown={(e) => e.stopPropagation()}>
      {!current ? (
        <button onClick={handlePickFile}>+ Ersätt klippets ljud</button>
      ) : (
        <>
          <div className="audio-file-name" title={current.sourcePath}>
            🎵 {fileNameFromPath(current.sourcePath)}
          </div>
          <button onClick={handlePickFile}>Byt fil</button>

          <label className="transition-duration-label">
            Volym: {current.gainDb.toFixed(0)} dB
            <input
              type="range"
              min={-40}
              max={6}
              step={1}
              value={current.gainDb}
              onChange={(e) => onChange({ ...current, gainDb: parseFloat(e.target.value) })}
            />
          </label>

          <div className="audio-blend-mode">
            {(Object.keys(AUDIO_MODE_LABELS) as AudioMode[]).map((mode) => (
              <span key={mode}>
                <input
                  type="radio"
                  name="clipAudioMode"
                  checked={current.mode === mode}
                  onChange={() => onChange({ ...current, mode })}
                />
                {AUDIO_MODE_LABELS[mode]}
              </span>
            ))}
          </div>
        </>
      )}

      <div className="transition-picker-actions">
        <button onClick={() => onChange(null)} disabled={!current}>
          Ta bort
        </button>
        <button onClick={onClose}>Stäng</button>
      </div>
    </div>
  );
}
