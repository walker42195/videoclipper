# VideoClipper

A desktop video editor for stitching together your own video clips into a movie — built with [Tauri](https://tauri.app) (Rust backend + React/TypeScript frontend), using [ffmpeg](https://ffmpeg.org) under the hood for all encoding and compositing.

## Features

- **Combine clips into a movie** — import video clips, drag to reorder, trim start/end per clip.
- **Transitions** — the full set of [ffmpeg `xfade`](https://ffmpeg.org/ffmpeg-filters.html#xfade) transitions (fades, wipes, slides, zooms, cover/reveal, and more) between any two clips, plus a transition at the very start and/or end of the movie (fading in from / out to black — or any other transition type).
- **Still images & text cards** — drop a photo or a title card (custom text, background color, font color) anywhere in the timeline; both flow through the same trim/transition pipeline as video clips.
- **Audio replacement** — replace a single clip's audio, or the whole movie's audio (mixed under the existing sound, or replacing it entirely), with independent fade in/out and gain control.
- **Preview before exporting** — play back the full composited timeline (transitions, text, audio) at a fast/low-res preset before committing to a full export.
- **Export to a web-friendly format** — H.264/AAC MP4 with `faststart`, at a choice of quality presets, with live progress and cancel support.
- **Save/load projects** as JSON.
- **Cross-platform** — runs on Linux and Windows; built and cross-compiled from Linux.

## Development

```bash
pnpm install
scripts/fetch-ffmpeg.sh linux   # vendors ffmpeg/ffprobe sidecar binaries (gitignored)
pnpm tauri dev
```

## Building

```bash
pnpm tauri build --bundles deb        # Linux (.deb)
pnpm tauri build --bundles appimage   # Linux (AppImage)
pnpm tauri build --target x86_64-pc-windows-gnu --bundles nsis   # Windows, cross-compiled from Linux
```

## Tech stack

- **Frontend**: React, TypeScript, [zustand](https://github.com/pmndrs/zustand) for state, [@dnd-kit](https://dndkit.com) for the drag-and-drop timeline.
- **Backend**: Rust, [Tauri v2](https://tauri.app), ffmpeg run as a bundled sidecar process.
- All video composition (normalization, transitions, text overlays, audio mixing) is expressed as a single `ffmpeg -filter_complex` graph built from the project's clip/transition/audio data.
