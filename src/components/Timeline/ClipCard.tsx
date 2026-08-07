import { useEffect, useRef, useState } from "react";
import { useSortable } from "@dnd-kit/sortable";
import { CSS } from "@dnd-kit/utilities";
import { Clip, STILL_ITEM_MAX_DURATION_SEC } from "../../types";
import { extractThumbnail } from "../../lib/tauriCommands";
import { useProjectStore } from "../../state/projectStore";
import { useBlobUrl } from "../../lib/useBlobUrl";
import { ClipAudioPicker } from "./ClipAudioPicker";

const MIN_CLIP_DURATION_SEC = 0.2;

function formatDuration(sec: number): string {
  const m = Math.floor(sec / 60);
  const s = (sec % 60).toFixed(1);
  return `${m}:${s.padStart(4, "0")}`;
}

function fileNameFromPath(path: string): string {
  return path.split(/[/\\]/).pop() ?? path;
}

type TrimHandle = "in" | "out";

interface ClipCardProps {
  clip: Clip;
  index: number;
  onRemove: (id: string) => void;
  onTrimChange: (id: string, trimInSec: number, trimOutSec: number) => void;
}

export function ClipCard({ clip, index, onRemove, onTrimChange }: ClipCardProps) {
  const { attributes, listeners, setNodeRef, transform, transition, isDragging } = useSortable({
    id: clip.id,
  });
  const { previewClipId, setPreviewClipId, movieAudioOverride, updateClipAudioOverride } = useProjectStore();
  const [audioPickerOpen, setAudioPickerOpen] = useState(false);
  // A whole-movie override in Replace mode drops the timeline's own audio
  // entirely, so per-clip replacements have no effect - gray the control out
  // rather than let it silently do nothing. Mix mode still layers per-clip
  // audio underneath the music, so it stays active there.
  const clipAudioDisabled = movieAudioOverride?.blendMode === "Replace";

  // Image/TextCard have no source file to trim within - trimInSec is always
  // 0 (no start handle) and trimOutSec is just "how long this stays on
  // screen", capped by an arbitrary UI limit rather than a real duration.
  const isVideo = clip.source.kind === "video";
  const maxDuration = isVideo ? clip.sourceDurationSec : STILL_ITEM_MAX_DURATION_SEC;

  const [thumbnail, setThumbnail] = useState<string | null>(null);
  const imageBlobUrl = useBlobUrl(clip.source.kind === "image" ? clip.sourcePath : null);
  const dragState = useRef<{
    handle: TrimHandle;
    startX: number;
    startTrimIn: number;
    startTrimOut: number;
    pxPerSec: number;
  } | null>(null);

  // Fetch a fresh thumbnail once on mount and whenever the committed
  // trim-in point changes (not on every pointermove while dragging). Only
  // video clips need this - image thumbnails are just the image itself.
  useEffect(() => {
    if (clip.source.kind !== "video") return;
    let cancelled = false;
    extractThumbnail(clip.sourcePath, clip.trimInSec)
      .then((dataUrl) => {
        if (!cancelled) setThumbnail(dataUrl);
      })
      .catch(() => {
        if (!cancelled) setThumbnail(null);
      });
    return () => {
      cancelled = true;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [clip.source.kind, clip.sourcePath, clip.trimInSec]);

  function beginTrimDrag(handle: TrimHandle, e: React.PointerEvent<HTMLDivElement>) {
    e.stopPropagation();
    e.preventDefault();
    (e.target as Element).setPointerCapture(e.pointerId);
    // 4px per second of clip is enough resolution for coarse trim dragging
    // at typical clip-card widths without needing the card's live layout.
    dragState.current = {
      handle,
      startX: e.clientX,
      startTrimIn: clip.trimInSec,
      startTrimOut: clip.trimOutSec,
      pxPerSec: 4,
    };
  }

  function onTrimPointerMove(e: React.PointerEvent<HTMLDivElement>) {
    const state = dragState.current;
    if (!state) return;
    const deltaSec = (e.clientX - state.startX) / state.pxPerSec;

    if (state.handle === "in") {
      const nextIn = Math.min(
        Math.max(0, state.startTrimIn + deltaSec),
        clip.trimOutSec - MIN_CLIP_DURATION_SEC,
      );
      onTrimChange(clip.id, nextIn, clip.trimOutSec);
    } else {
      const nextOut = Math.max(
        Math.min(maxDuration, state.startTrimOut + deltaSec),
        clip.trimInSec + MIN_CLIP_DURATION_SEC,
      );
      onTrimChange(clip.id, clip.trimInSec, nextOut);
    }
  }

  function endTrimDrag(e: React.PointerEvent<HTMLDivElement>) {
    if (!dragState.current) return;
    (e.target as Element).releasePointerCapture(e.pointerId);
    dragState.current = null;
  }

  const style: React.CSSProperties = {
    transform: CSS.Transform.toString(transform),
    transition,
    opacity: isDragging ? 0.5 : 1,
  };

  const canTrimIn = clip.trimInSec > 0;
  const canTrimOut = clip.trimOutSec < maxDuration;

  const isPreviewing = clip.id === previewClipId;

  const displayName =
    clip.source.kind === "textCard"
      ? clip.source.text.trim() || "Textkort"
      : fileNameFromPath(clip.sourcePath);

  return (
    <div ref={setNodeRef} style={style} className={`clip-card ${isPreviewing ? "clip-card-previewing" : ""}`}>
      <div
        className="clip-card-drag-handle"
        {...attributes}
        {...listeners}
        onClick={() => setPreviewClipId(clip.id)}
        title="Klicka för att förhandsgranska"
      >
        <span className="clip-index">{index + 1}</span>
        <div className="clip-thumbnail">
          {clip.source.kind === "video" && (thumbnail ? <img src={thumbnail} alt="" /> : <div className="clip-thumbnail-placeholder" />)}
          {clip.source.kind === "image" && (imageBlobUrl ? <img src={imageBlobUrl} alt="" /> : <div className="clip-thumbnail-placeholder" />)}
          {clip.source.kind === "textCard" && (
            <div
              className="clip-thumbnail-textcard"
              style={{ backgroundColor: clip.source.backgroundColor, color: clip.source.fontColor }}
            >
              T
            </div>
          )}
        </div>
        <div className="clip-info">
          <span className="clip-name">{displayName}</span>
          <span className="clip-duration">{formatDuration(clip.trimOutSec - clip.trimInSec)}</span>
        </div>
      </div>

      {isVideo && (
        <div
          className={`trim-handle trim-handle-in ${canTrimIn ? "" : "trim-handle-disabled"}`}
          onPointerDown={(e) => beginTrimDrag("in", e)}
          onPointerMove={onTrimPointerMove}
          onPointerUp={endTrimDrag}
          title="Dra för att trimma klippets början"
        />
      )}
      <div
        className={`trim-handle trim-handle-out ${canTrimOut ? "" : "trim-handle-disabled"}`}
        onPointerDown={(e) => beginTrimDrag("out", e)}
        onPointerMove={onTrimPointerMove}
        onPointerUp={endTrimDrag}
        title={isVideo ? "Dra för att trimma klippets slut" : "Dra för att ändra visningstid"}
      />

      <div className="clip-audio-btn-wrapper">
        <button
          className={`clip-audio-btn ${clip.audioOverride ? "clip-audio-btn-active" : ""}`}
          onClick={() => setAudioPickerOpen((v) => !v)}
          disabled={clipAudioDisabled}
          title={
            clipAudioDisabled
              ? "Inaktiv - filmens ljud ersätts helt av bakgrundsmusiken"
              : clip.audioOverride
                ? "Klippets ljud är ersatt"
                : "Ersätt klippets ljud"
          }
        >
          🎵
        </button>
        {audioPickerOpen && !clipAudioDisabled && (
          <ClipAudioPicker
            current={clip.audioOverride}
            onChange={(override) => updateClipAudioOverride(clip.id, override)}
            onClose={() => setAudioPickerOpen(false)}
          />
        )}
      </div>

      <button className="remove-btn" onClick={() => onRemove(clip.id)}>
        Ta bort
      </button>
    </div>
  );
}
