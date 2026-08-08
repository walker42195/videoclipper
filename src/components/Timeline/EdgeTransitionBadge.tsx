import { useState } from "react";
import { EdgeTransition, TRANSITION_LABELS, TransitionType } from "../../types";
import { TransitionPicker } from "./TransitionPicker";

interface EdgeTransitionBadgeProps {
  label: string;
  transition: EdgeTransition | null;
  maxDurationSec: number;
  onChange: (transition: EdgeTransition | null) => void;
}

/** Transition at the very start/end of the whole timeline - same picker UI
 * and full transition catalog as TransitionBadge (between two real clips),
 * but against a synthesized black clip on the backend rather than a second
 * real clip, since there's only one adjacent clip here, not two. */
export function EdgeTransitionBadge({ label, transition, maxDurationSec, onChange }: EdgeTransitionBadgeProps) {
  const [open, setOpen] = useState(false);

  function handleApply(type: TransitionType, durationSec: number) {
    onChange({ type, durationSec });
    setOpen(false);
  }

  function handleClear() {
    onChange(null);
    setOpen(false);
  }

  return (
    <div className="transition-badge-wrapper">
      <button
        className={`transition-badge ${transition ? "transition-badge-active" : ""}`}
        onClick={() => setOpen((v) => !v)}
        title={transition ? `${label}: ${TRANSITION_LABELS[transition.type]}` : `Lägg till övergång i ${label.toLowerCase()}`}
      >
        {transition ? TRANSITION_LABELS[transition.type] : "+"}
      </button>
      {open && (
        <TransitionPicker
          current={transition ?? undefined}
          maxDurationSec={maxDurationSec}
          onApply={handleApply}
          onClear={handleClear}
          onClose={() => setOpen(false)}
        />
      )}
    </div>
  );
}
