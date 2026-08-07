import { RefObject, useRef } from "react";
import { useProjectStore } from "../../state/projectStore";
import { useBlobUrl } from "../../lib/useBlobUrl";
import { Clip, STILL_ITEM_MAX_DURATION_SEC } from "../../types";

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

interface DurationControlsProps {
  clip: Clip;
  onTrimChange: (id: string, trimInSec: number, trimOutSec: number) => void;
}

/** Duration editor for Image/TextCard clips - there's no source media to
 * trim within, just a display length to shorten or lengthen. */
function DurationControls({ clip, onTrimChange }: DurationControlsProps) {
  function adjust(deltaSec: number) {
    const next = Math.max(MIN_CLIP_DURATION_SEC, Math.min(STILL_ITEM_MAX_DURATION_SEC, clip.trimOutSec + deltaSec));
    onTrimChange(clip.id, 0, next);
  }

  return (
    <div className="trim-controls">
      <div className="trim-controls-row">
        <span className="trim-controls-label">Visningstid: {formatSeconds(clip.trimOutSec)}</span>
        <button onClick={() => adjust(-STEP_SEC)} disabled={clip.trimOutSec <= MIN_CLIP_DURATION_SEC}>
          −1s
        </button>
        <button onClick={() => adjust(STEP_SEC)} disabled={clip.trimOutSec >= STILL_ITEM_MAX_DURATION_SEC}>
          +1s
        </button>
      </div>
    </div>
  );
}

interface TextCardEditorProps {
  clip: Clip;
}

function TextCardEditor({ clip }: TextCardEditorProps) {
  const { updateClipSource } = useProjectStore();
  if (clip.source.kind !== "textCard") return null;
  const source = clip.source;

  return (
    <div
      className="text-card-preview"
      style={{ backgroundColor: source.backgroundColor, color: source.fontColor }}
    >
      <textarea
        className="text-card-textarea"
        style={{ color: source.fontColor }}
        value={source.text}
        placeholder="Skriv texten som ska visas..."
        onChange={(e) => updateClipSource(clip.id, { ...source, text: e.target.value })}
      />
      <div className="text-card-color-row">
        <label>
          Bakgrund
          <input
            type="color"
            value={source.backgroundColor}
            onChange={(e) => updateClipSource(clip.id, { ...source, backgroundColor: e.target.value })}
          />
        </label>
        <label>
          Text
          <input
            type="color"
            value={source.fontColor}
            onChange={(e) => updateClipSource(clip.id, { ...source, fontColor: e.target.value })}
          />
        </label>
      </div>
    </div>
  );
}

export function ClipPreviewPlayer() {
  const { clips, previewClipId, setPreviewClipId, updateClipTrim } = useProjectStore();
  const videoRef = useRef<HTMLVideoElement>(null);
  const clip = clips.find((c) => c.id === previewClipId);
  const isMediaFile = clip?.source.kind === "video" || clip?.source.kind === "image";
  const blobUrl = useBlobUrl(isMediaFile ? (clip?.sourcePath ?? null) : null);

  if (!clip) return null;

  const title = clip.source.kind === "textCard" ? "Textkort" : fileNameFromPath(clip.sourcePath);

  return (
    <div className="clip-preview">
      <div className="clip-preview-header">
        <span>Förhandsvisning: {title}</span>
        <button onClick={() => setPreviewClipId(null)}>Stäng</button>
      </div>

      {clip.source.kind === "video" && (
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
      )}
      {clip.source.kind === "image" && (blobUrl ? <img className="clip-preview-image" src={blobUrl} alt="" /> : null)}
      {clip.source.kind === "textCard" && <TextCardEditor clip={clip} />}

      {clip.source.kind === "video" ? (
        <TrimControls clip={clip} videoRef={videoRef} onTrimChange={updateClipTrim} />
      ) : (
        <DurationControls clip={clip} onTrimChange={updateClipTrim} />
      )}
    </div>
  );
}
