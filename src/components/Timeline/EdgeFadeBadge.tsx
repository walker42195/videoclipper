import { useState } from "react";
import { MAX_EDGE_FADE_SEC } from "../../types";

interface EdgeFadeBadgeProps {
  label: string;
  fadeSec: number;
  maxDurationSec: number;
  onChange: (sec: number) => void;
}

/** Fade-to-black control for the very start/end of the whole timeline -
 * unlike TransitionBadge (between two clips), there's no "other side" to
 * crossfade with here, so this only offers a duration, not a transition
 * type picker. */
export function EdgeFadeBadge({ label, fadeSec, maxDurationSec, onChange }: EdgeFadeBadgeProps) {
  const [open, setOpen] = useState(false);
  const cappedMax = Math.max(0.1, Math.min(MAX_EDGE_FADE_SEC, maxDurationSec));

  return (
    <div className="transition-badge-wrapper">
      <button
        className={`transition-badge ${fadeSec > 0 ? "transition-badge-active" : ""}`}
        onClick={() => setOpen((v) => !v)}
        title={fadeSec > 0 ? `${label}: tona till svart (${fadeSec.toFixed(1)}s)` : `Lägg till tona till svart i ${label.toLowerCase()}`}
      >
        {fadeSec > 0 ? `⬛ ${fadeSec.toFixed(1)}s` : "⬛ +"}
      </button>
      {open && (
        <div className="transition-picker edge-fade-picker" onPointerDown={(e) => e.stopPropagation()}>
          <label className="transition-duration-label">
            {label}: tona till svart, {fadeSec.toFixed(2)}s
            <input
              type="range"
              min={0}
              max={cappedMax}
              step={0.05}
              value={Math.min(fadeSec, cappedMax)}
              onChange={(e) => onChange(parseFloat(e.target.value))}
            />
          </label>
          <div className="transition-picker-actions">
            <button onClick={() => onChange(0)} disabled={fadeSec === 0}>
              Ingen tona
            </button>
            <button onClick={() => setOpen(false)}>Stäng</button>
          </div>
        </div>
      )}
    </div>
  );
}
