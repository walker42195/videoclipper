import { RefObject, useRef } from "react";
import { useProjectStore } from "../../state/projectStore";
import { useBlobUrl } from "../../lib/useBlobUrl";
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
  videoRef: RefObject<HTMLVideoElement | null>;
  onTrimChange: (id: string, trimInSec: number, trimOutSec: number) => void;
}

function TrimControls({ clip, videoRef, onTrimChange }: TrimControlsProps) {
  // Read the video's position live at click-time rather than trusting a
  // React state value kept in sync via onTimeUpdate - that event only
  // fires while the video is actively playing/seeking, so if autoplay
  // got silently blocked (WebKit blocks unmuted autoplay without a user
  // gesture) the state would stay stuck at 0 forever even though the
  // player itself knows its real position.
  function playheadSec(): number {
    return videoRef.current?.currentTime ?? 0;
  }

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
    onTrimChange(clip.id, Math.min(playheadSec(), clip.trimOutSec - MIN_CLIP_DURATION_SEC), clip.trimOutSec);
  }
  function setEndToPlayhead() {
    onTrimChange(clip.id, clip.trimInSec, Math.max(playheadSec(), clip.trimInSec + MIN_CLIP_DURATION_SEC));
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
  const clip = clips.find((c) => c.id === previewClipId);
  const blobUrl = useBlobUrl(clip?.sourcePath ?? null);

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
        src={blobUrl ?? undefined}
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
        }}
      />
      <TrimControls clip={clip} videoRef={videoRef} onTrimChange={updateClipTrim} />
    </div>
  );
}
