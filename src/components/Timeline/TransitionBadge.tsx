import { useState } from "react";
import { Clip, TRANSITION_LABELS, Transition, TransitionType } from "../../types";
import { useProjectStore } from "../../state/projectStore";
import { TransitionPicker } from "./TransitionPicker";

interface TransitionBadgeProps {
  fromClip: Clip;
  toClip: Clip;
  transition: Transition | undefined;
}

export function TransitionBadge({ fromClip, toClip, transition }: TransitionBadgeProps) {
  const [open, setOpen] = useState(false);
  const { setTransition, clearTransition } = useProjectStore();

  const fromDuration = fromClip.trimOutSec - fromClip.trimInSec;
  const toDuration = toClip.trimOutSec - toClip.trimInSec;
  const maxDurationSec = Math.min(fromDuration, toDuration);

  function handleApply(type: TransitionType, durationSec: number) {
    setTransition(fromClip.id, toClip.id, type, durationSec);
    setOpen(false);
  }

  function handleClear() {
    clearTransition(fromClip.id, toClip.id);
    setOpen(false);
  }

  return (
    <div className="transition-badge-wrapper">
      <button
        className={`transition-badge ${transition ? "transition-badge-active" : ""}`}
        onClick={() => setOpen((v) => !v)}
        title={transition ? TRANSITION_LABELS[transition.type] : "Lägg till övergång"}
      >
        {transition ? TRANSITION_LABELS[transition.type] : "+"}
      </button>
      {open && (
        <TransitionPicker
          current={transition}
          maxDurationSec={maxDurationSec}
          onApply={handleApply}
          onClear={handleClear}
          onClose={() => setOpen(false)}
        />
      )}
    </div>
  );
}
