import {
  DndContext,
  DragEndEvent,
  KeyboardSensor,
  PointerSensor,
  closestCenter,
  useSensor,
  useSensors,
} from "@dnd-kit/core";
import {
  SortableContext,
  horizontalListSortingStrategy,
  sortableKeyboardCoordinates,
} from "@dnd-kit/sortable";
import { useProjectStore } from "../../state/projectStore";
import { ClipCard } from "./ClipCard";
import { TransitionBadge } from "./TransitionBadge";
import { EdgeTransitionBadge } from "./EdgeTransitionBadge";

export function Timeline() {
  const {
    clips,
    transitions,
    removeClip,
    moveClip,
    updateClipTrim,
    introTransition,
    outroTransition,
    setIntroTransition,
    setOutroTransition,
  } = useProjectStore();

  const sensors = useSensors(
    useSensor(PointerSensor, { activationConstraint: { distance: 4 } }),
    useSensor(KeyboardSensor, { coordinateGetter: sortableKeyboardCoordinates }),
  );

  function handleDragEnd(event: DragEndEvent) {
    const { active, over } = event;
    if (!over || active.id === over.id) return;
    const fromIndex = clips.findIndex((c) => c.id === active.id);
    const toIndex = clips.findIndex((c) => c.id === over.id);
    if (fromIndex === -1 || toIndex === -1) return;
    moveClip(fromIndex, toIndex);
  }

  if (clips.length === 0) {
    return (
      <p className="empty-state">
        Inga klipp tillagda än. Klicka "Lägg till klipp" för att börja.
      </p>
    );
  }

  return (
    <DndContext sensors={sensors} collisionDetection={closestCenter} onDragEnd={handleDragEnd}>
      <SortableContext items={clips.map((c) => c.id)} strategy={horizontalListSortingStrategy}>
        <div className="timeline">
          <EdgeTransitionBadge
            label="Start"
            transition={introTransition}
            maxDurationSec={clips[0].trimOutSec - clips[0].trimInSec}
            onChange={setIntroTransition}
          />
          {clips.map((clip, index) => (
            <div className="timeline-item" key={clip.id}>
              <ClipCard clip={clip} index={index} onRemove={removeClip} onTrimChange={updateClipTrim} />
              {index < clips.length - 1 && (
                <TransitionBadge
                  fromClip={clip}
                  toClip={clips[index + 1]}
                  transition={transitions.find(
                    (t) => t.fromClipId === clip.id && t.toClipId === clips[index + 1].id,
                  )}
                />
              )}
            </div>
          ))}
          <EdgeTransitionBadge
            label="Slut"
            transition={outroTransition}
            maxDurationSec={clips[clips.length - 1].trimOutSec - clips[clips.length - 1].trimInSec}
            onChange={setOutroTransition}
          />
        </div>
      </SortableContext>
    </DndContext>
  );
}
