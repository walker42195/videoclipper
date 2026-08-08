import { useState } from "react";
import { TRANSITION_GROUPS, TRANSITION_LABELS, TransitionType } from "../../types";

const DEFAULT_DURATION_SEC = 1.0;
const MIN_DURATION_SEC = 0.1;
const MAX_DURATION_SEC = 3.0;

interface TransitionPickerProps {
  /** Only the type/duration are read - works for both a clip-to-clip
   * Transition and an edge-of-timeline EdgeTransition without needing a
   * fromClipId/toClipId that an edge transition doesn't have. */
  current: { type: TransitionType; durationSec: number } | undefined;
  maxDurationSec: number;
  onApply: (type: TransitionType, durationSec: number) => void;
  onClear: () => void;
  onClose: () => void;
}

export function TransitionPicker({ current, maxDurationSec, onApply, onClear, onClose }: TransitionPickerProps) {
  const [durationSec, setDurationSec] = useState(current?.durationSec ?? DEFAULT_DURATION_SEC);
  const cappedMax = Math.max(MIN_DURATION_SEC, Math.min(MAX_DURATION_SEC, maxDurationSec));

  return (
    <div className="transition-picker" onPointerDown={(e) => e.stopPropagation()}>
      <div className="transition-picker-groups">
        {TRANSITION_GROUPS.map((group) => (
          <div key={group.label} className="transition-picker-group">
            <div className="transition-picker-group-label">{group.label}</div>
            <div className="transition-picker-grid">
              {group.types.map((type) => (
                <button
                  key={type}
                  className={`transition-option ${current?.type === type ? "transition-option-active" : ""}`}
                  onClick={() => onApply(type, durationSec)}
                >
                  {TRANSITION_LABELS[type]}
                </button>
              ))}
            </div>
          </div>
        ))}
      </div>

      <label className="transition-duration-label">
        Längd: {durationSec.toFixed(2)}s
        <input
          type="range"
          min={MIN_DURATION_SEC}
          max={cappedMax}
          step={0.05}
          value={Math.min(durationSec, cappedMax)}
          onChange={(e) => setDurationSec(parseFloat(e.target.value))}
        />
      </label>

      <div className="transition-picker-actions">
        <button onClick={onClear} disabled={!current}>
          Ingen övergång
        </button>
        <button onClick={onClose}>Stäng</button>
      </div>
    </div>
  );
}
