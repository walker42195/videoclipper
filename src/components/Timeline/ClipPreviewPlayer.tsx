import { useRef } from "react";
import { convertFileSrc } from "@tauri-apps/api/core";
import { useProjectStore } from "../../state/projectStore";

function fileNameFromPath(path: string): string {
  return path.split(/[/\\]/).pop() ?? path;
}

export function ClipPreviewPlayer() {
  const { clips, previewClipId, setPreviewClipId } = useProjectStore();
  const videoRef = useRef<HTMLVideoElement>(null);
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
        onTimeUpdate={() => {
          const video = videoRef.current;
          if (video && video.currentTime >= clip.trimOutSec) {
            video.currentTime = clip.trimInSec;
          }
        }}
      />
    </div>
  );
}
