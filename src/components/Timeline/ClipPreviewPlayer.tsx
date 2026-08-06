import { useRef, useState } from "react";
import { convertFileSrc } from "@tauri-apps/api/core";
import { useProjectStore } from "../../state/projectStore";
import { Clip } from "../../types";

const STEP_SEC = 1;
const MIN_CLIP_DURATION_SEC = 0.1;

function fileNameFromPath(path: string): string {
  return path.split(/[/\\]/).pop() ?? path;
}

function formatSeconds(sec: number): string {
  return sec.toFixed(1) + "s";
}

interface TrimControlsProps {
  clip: Clip;
  currentTime: number;
  onTrimChange: (id: string, trimInSec: number, trimOutSec: number) => void;
}

function TrimControls({ clip, currentTime, onTrimChange }: TrimControlsProps) {
  function adjustStart(deltaSec: number) {
    const next = Math.min(Math.max(0, clip.trimInSec + deltaSec), clip.trimOutSec - MIN_CLIP_DURATION_SEC);
    onTrimChange(clip.id, next, clip.trimOutSec);
  }
  function adjustEnd(deltaSec: number) {
    const next = Math.max(
      Math.min(clip.sourceDurationSec, clip.trimOutSec + deltaSec),
      clip.trimInSec + MIN_CLIP_DURATION_SEC,
    );
    onTrimChange(clip.id, clip.trimInSec, next);
  }
  function setStartToPlayhead() {
    onTrimChange(clip.id, Math.min(currentTime, clip.trimOutSec - MIN_CLIP_DURATION_SEC), clip.trimOutSec);
  }
  function setEndToPlayhead() {
    onTrimChange(clip.id, clip.trimInSec, Math.max(currentTime, clip.trimInSec + MIN_CLIP_DURATION_SEC));
  }

  return (
    <div className="trim-controls">
      <div className="trim-controls-row">
        <span className="trim-controls-label">Start: {formatSeconds(clip.trimInSec)}</span>
        <button onClick={() => adjustStart(-STEP_SEC)} disabled={clip.trimInSec <= 0}>
          −1s
        </button>
        <button onClick={() => adjustStart(STEP_SEC)}>+1s</button>
        <button onClick={setStartToPlayhead} title="Klipp bort allt före spelarens nuvarande position">
          Klipp här
        </button>
      </div>
      <div className="trim-controls-row">
        <span className="trim-controls-label">Slut: {formatSeconds(clip.trimOutSec)}</span>
        <button onClick={() => adjustEnd(-STEP_SEC)}>−1s</button>
        <button onClick={() => adjustEnd(STEP_SEC)} disabled={clip.trimOutSec >= clip.sourceDurationSec}>
          +1s
        </button>
        <button onClick={setEndToPlayhead} title="Klipp bort allt efter spelarens nuvarande position">
          Klipp här
        </button>
      </div>
    </div>
  );
}

export function ClipPreviewPlayer() {
  const { clips, previewClipId, setPreviewClipId, updateClipTrim } = useProjectStore();
  const videoRef = useRef<HTMLVideoElement>(null);
  const [currentTime, setCurrentTime] = useState(0);
  const clip = clips.find((c) => c.id === previewClipId);

  if (!clip) return null;

  return (
    <div className="clip-preview">
      <div className="clip-preview-header">
        <span>Förhandsvisning: {fileNameFromPath(clip.sourcePath)}</span>
        <button onClick={() => setPreviewClipId(null)}>Stäng</button>
      </div>
      <video
        key={clip.id}
        ref={videoRef}
        src={convertFileSrc(clip.sourcePath)}
        controls
        autoPlay
        onLoadedMetadata={() => {
          if (videoRef.current) videoRef.current.currentTime = clip.trimInSec;
        }}
        onTimeUpdate={(e) => {
          const video = e.currentTarget;
          if (video.currentTime >= clip.trimOutSec) {
            video.currentTime = clip.trimInSec;
          }
          setCurrentTime(video.currentTime);
        }}
      />
      <TrimControls clip={clip} currentTime={currentTime} onTrimChange={updateClipTrim} />
    </div>
  );
}
