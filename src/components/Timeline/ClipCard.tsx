import { useEffect, useRef, useState } from "react";
import { useSortable } from "@dnd-kit/sortable";
import { CSS } from "@dnd-kit/utilities";
import { Clip } from "../../types";
import { extractThumbnail } from "../../lib/tauriCommands";
import { useProjectStore } from "../../state/projectStore";

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
  const { previewClipId, setPreviewClipId } = useProjectStore();

  const [thumbnail, setThumbnail] = useState<string | null>(null);
  const dragState = useRef<{
    handle: TrimHandle;
    startX: number;
    startTrimIn: number;
    startTrimOut: number;
    pxPerSec: number;
  } | null>(null);

  // Fetch a fresh thumbnail once on mount and whenever the committed
  // trim-in point changes (not on every pointermove while dragging).
  useEffect(() => {
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
  }, [clip.sourcePath, clip.trimInSec]);

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
        Math.min(clip.sourceDurationSec, state.startTrimOut + deltaSec),
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
  const canTrimOut = clip.trimOutSec < clip.sourceDurationSec;

  const isPreviewing = clip.id === previewClipId;

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
          {thumbnail ? <img src={thumbnail} alt="" /> : <div className="clip-thumbnail-placeholder" />}
        </div>
        <div className="clip-info">
          <span className="clip-name">{fileNameFromPath(clip.sourcePath)}</span>
          <span className="clip-duration">{formatDuration(clip.trimOutSec - clip.trimInSec)}</span>
        </div>
      </div>

      <div
        className={`trim-handle trim-handle-in ${canTrimIn ? "" : "trim-handle-disabled"}`}
        onPointerDown={(e) => beginTrimDrag("in", e)}
        onPointerMove={onTrimPointerMove}
        onPointerUp={endTrimDrag}
        title="Dra för att trimma klippets början"
      />
      <div
        className={`trim-handle trim-handle-out ${canTrimOut ? "" : "trim-handle-disabled"}`}
        onPointerDown={(e) => beginTrimDrag("out", e)}
        onPointerMove={onTrimPointerMove}
        onPointerUp={endTrimDrag}
        title="Dra för att trimma klippets slut"
      />

      <button className="remove-btn" onClick={() => onRemove(clip.id)}>
        Ta bort
      </button>
    </div>
  );
}
