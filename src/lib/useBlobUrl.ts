import { useEffect, useState } from "react";
import { readFile } from "@tauri-apps/plugin-fs";

function guessMimeType(path: string): string {
  const ext = path.split(".").pop()?.toLowerCase();
  switch (ext) {
    case "mp4":
    case "m4v":
      return "video/mp4";
    case "mov":
      return "video/quicktime";
    case "webm":
      return "video/webm";
    case "mkv":
      return "video/x-matroska";
    case "avi":
      return "video/x-msvideo";
    default:
      return "video/mp4";
  }
}

/**
 * Reads a local file's bytes and exposes it as a `blob:` URL, revoked
 * automatically when the path changes or the component unmounts.
 *
 * Needed because WebKitGTK's video/audio elements go through GStreamer,
 * which doesn't recognize Tauri's custom `asset://` URI scheme the way
 * WebKit's own resource loader does for images - convertFileSrc() silently
 * fails to play video on Linux (upstream WebKitGTK/GStreamer limitation).
 * blob: URLs are handled entirely inside the webview, sidestepping that.
 */
export function useBlobUrl(path: string | null): string | null {
  const [url, setUrl] = useState<string | null>(null);

  useEffect(() => {
    if (!path) {
      setUrl(null);
      return;
    }
    let cancelled = false;
    let objectUrl: string | null = null;

    readFile(path).then((bytes) => {
      if (cancelled) return;
      objectUrl = URL.createObjectURL(new Blob([bytes], { type: guessMimeType(path) }));
      setUrl(objectUrl);
    });

    return () => {
      cancelled = true;
      if (objectUrl) URL.revokeObjectURL(objectUrl);
    };
  }, [path]);

  return url;
}
