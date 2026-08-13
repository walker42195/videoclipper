use crate::ffmpeg::filtergraph::{
    build_background_music_chain, build_transition_graph, AudioReplacement, BackgroundMusicSpec,
    GraphClip, NormalizeTarget, TextOverlaySpec, TransitionJunction, CUT_DURATION_SEC,
};
use crate::ffmpeg::progress::ProgressParser;
use crate::model::{
    AudioBlendMode, Clip, ClipSource, EdgeTransition, ExportSettings, MovieAudioOverride, Transition,
    TransitionType,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use tauri::path::BaseDirectory;
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_shell::{process::CommandChild, process::CommandEvent, ShellExt};

/// Validates a color is `#RRGGBB` before it ever reaches an ffmpeg filter
/// string; anything else (which shouldn't happen from the color-picker-only
/// UI, but this is a value that crossed the IPC boundary) falls back to a
/// safe default rather than being passed through.
fn sanitize_hex_color(input: &str, fallback: &str) -> String {
    let valid = input.len() == 7 && input.starts_with('#') && input[1..].chars().all(|c| c.is_ascii_hexdigit());
    if valid { input.to_string() } else { fallback.to_string() }
}

/// Writes each text card's text to its own temp file (ffmpeg's drawtext
/// reads text from disk via `textfile=` rather than inline, see
/// `TextOverlaySpec`'s doc comment for why). Returns a map from clip id to
/// that clip's temp file path; callers must remove these files themselves
/// once ffmpeg no longer needs them.
fn write_text_card_files(clips: &[Clip]) -> Result<HashMap<String, PathBuf>, String> {
    let mut paths = HashMap::new();
    for clip in clips {
        if let ClipSource::TextCard { text, .. } = &clip.source {
            let path = std::env::temp_dir().join(format!("videoclipper_textcard_{}.txt", clip.id));
            std::fs::write(&path, text).map_err(|e| format!("failed to write text card file: {e}"))?;
            paths.insert(clip.id.clone(), path);
        }
    }
    Ok(paths)
}

/// Tracks the currently-running export's ffmpeg child so `cancel_export` can
/// kill it, and whether the last termination was a deliberate cancel (vs. a
/// real ffmpeg failure) so `export_project` can report it accordingly.
#[derive(Default)]
pub struct ExportHandle {
    child: Mutex<Option<CommandChild>>,
    cancelled: AtomicBool,
}

#[tauri::command]
pub fn cancel_export(state: State<'_, ExportHandle>) -> Result<(), String> {
    state.cancelled.store(true, Ordering::SeqCst);
    let child = state.child.lock().unwrap().take();
    if let Some(child) = child {
        child.kill().map_err(|e| format!("failed to kill ffmpeg: {e}"))?;
    }
    Ok(())
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportRequest {
    pub clips: Vec<Clip>,
    /// Sparse: only junctions the user put an explicit transition on need an
    /// entry here (matched by fromClipId/toClipId). Missing junctions render
    /// as a hard cut (see [`CUT_DURATION_SEC`]).
    pub transitions: Vec<Transition>,
    pub movie_audio_override: Option<MovieAudioOverride>,
    #[serde(default)]
    pub intro_transition: Option<EdgeTransition>,
    #[serde(default)]
    pub outro_transition: Option<EdgeTransition>,
    pub export_settings: ExportSettings,
    pub output_path: String,
}

/// Build the ordered, gap-filled per-junction transition list
/// `build_transition_graph` needs from the project's sparse transitions.
fn resolve_junctions(clips: &[Clip], transitions: &[Transition]) -> Vec<TransitionJunction> {
    clips
        .windows(2)
        .map(|pair| {
            let (from, to) = (&pair[0].id, &pair[1].id);
            transitions
                .iter()
                .find(|t| &t.from_clip_id == from && &t.to_clip_id == to)
                .map(|t| TransitionJunction {
                    transition_type: t.transition_type,
                    duration_sec: t.duration_sec,
                })
                .unwrap_or(TransitionJunction {
                    transition_type: TransitionType::Fade,
                    duration_sec: CUT_DURATION_SEC,
                })
        })
        .collect()
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ExportResult {
    pub output_path: String,
}

/// Builds the full ffmpeg argument list (everything up to but not including
/// `-progress`/output path) plus the computed total output duration, from an
/// [`ExportRequest`]. Kept free of Tauri/async so it's directly unit
/// testable - this is where a per-clip-override input's index or the
/// movie-audio input's index (offset by however many per-clip override
/// inputs preceded it) would silently drift if the bookkeeping were wrong.
///
/// Priority rule: a whole-movie audio override in `Replace` mode drops the
/// timeline's audio entirely (including any per-clip replacements) since
/// nothing from the timeline would be audible anyway. In `Mix` mode,
/// per-clip replacements stay active and get layered under the music - only
/// `Replace` makes them moot. The UI's per-clip audio panel must gray out
/// under the exact same condition, or the two would silently disagree.
fn build_ffmpeg_args(
    request: &ExportRequest,
    text_card_paths: &HashMap<String, PathBuf>,
    font_path: &str,
) -> (Vec<String>, f64) {
    let target = NormalizeTarget {
        width: request.export_settings.max_width,
        height: request.export_settings.max_width * 9 / 16,
        fps: request.export_settings.fps,
        sample_rate: 48000,
    };

    let mut args: Vec<String> = vec!["-y".into()];
    for clip in &request.clips {
        match &clip.source {
            ClipSource::Video => {
                args.push("-i".into());
                args.push(clip.source_path.clone());
            }
            ClipSource::Image => {
                // -loop makes a still image an infinite-duration video
                // stream; the same trim=start=0:end=<duration> filter every
                // other clip gets (via normalize_video_chain) caps it, so no
                // other part of the pipeline needs to know this isn't video.
                args.push("-loop".into());
                args.push("1".into());
                args.push("-i".into());
                args.push(clip.source_path.clone());
            }
            ClipSource::TextCard { background_color, .. } => {
                // A generated solid-color source stands in for a "file";
                // drawtext (added below via text_overlay) paints the text
                // onto it. Still exactly one -i per clip, like Video/Image.
                let duration = (clip.trim_out_sec - clip.trim_in_sec).max(0.1);
                args.push("-f".into());
                args.push("lavfi".into());
                args.push("-i".into());
                args.push(format!(
                    "color=c={color}:s={w}x{h}:d={duration}",
                    color = sanitize_hex_color(background_color, "#000000"),
                    w = target.width,
                    h = target.height,
                ));
            }
        }
    }
    let mut next_input_index = request.clips.len();

    let drop_clip_overrides = matches!(
        request.movie_audio_override.as_ref().map(|m| m.blend_mode),
        Some(AudioBlendMode::Replace)
    );

    let mut graph_clips: Vec<GraphClip> = request
        .clips
        .iter()
        .enumerate()
        .map(|(i, c)| {
            let audio_replacement = if drop_clip_overrides {
                None
            } else {
                c.audio_override.as_ref().map(|ov| {
                    let idx = next_input_index;
                    next_input_index += 1;
                    args.push("-i".into());
                    args.push(ov.source_path.clone());
                    AudioReplacement {
                        input_index: idx,
                        fill_mode: ov.mode,
                        gain_db: ov.gain_db,
                    }
                })
            };
            let text_overlay = match &c.source {
                ClipSource::TextCard { font_color, .. } => text_card_paths.get(&c.id).map(|path| TextOverlaySpec {
                    fontfile_path: font_path.to_string(),
                    textfile_path: path.to_string_lossy().to_string(),
                    font_color: sanitize_hex_color(font_color, "#FFFFFF"),
                }),
                _ => None,
            };
            GraphClip {
                input_index: i,
                trim_in_sec: c.trim_in_sec,
                trim_out_sec: c.trim_out_sec,
                audio_replacement,
                has_audio: c.has_audio,
                text_overlay,
            }
        })
        .collect();

    let mut junctions = resolve_junctions(&request.clips, &request.transitions);

    // Intro/outro edge transitions are folded straight into the same
    // xfade/acrossfade chain every clip-to-clip transition uses, against a
    // synthesized black clip standing in for "the frame before clip 1" or
    // "the frame after the last clip" - the same trick TextCard already uses
    // for a generated input. This gives edge transitions the full 58-type
    // catalog for free, rather than needing a separate fade-only code path.
    // No `d=` (duration) on the color source: like the `-loop 1` image
    // pattern, the source is left effectively infinite and
    // normalize_video_chain's trim=end=<duration> caps it - an explicit d=
    // here risks landing a frame short of the requested duration after fps
    // rounding, which would starve xfade of frames right at the tail.
    if let Some(intro) = &request.intro_transition {
        let idx = next_input_index;
        next_input_index += 1;
        args.push("-f".into());
        args.push("lavfi".into());
        args.push("-i".into());
        args.push(format!("color=c=black:s={}x{}", target.width, target.height));
        graph_clips.insert(
            0,
            GraphClip {
                input_index: idx,
                trim_in_sec: 0.0,
                trim_out_sec: intro.duration_sec.max(CUT_DURATION_SEC),
                audio_replacement: None,
                has_audio: false,
                text_overlay: None,
            },
        );
        junctions.insert(
            0,
            TransitionJunction {
                transition_type: intro.transition_type,
                duration_sec: intro.duration_sec.max(CUT_DURATION_SEC),
            },
        );
    }
    if let Some(outro) = &request.outro_transition {
        let idx = next_input_index;
        next_input_index += 1;
        args.push("-f".into());
        args.push("lavfi".into());
        args.push("-i".into());
        args.push(format!("color=c=black:s={}x{}", target.width, target.height));
        graph_clips.push(GraphClip {
            input_index: idx,
            trim_in_sec: 0.0,
            trim_out_sec: outro.duration_sec.max(CUT_DURATION_SEC),
            audio_replacement: None,
            has_audio: false,
            text_overlay: None,
        });
        junctions.push(TransitionJunction {
            transition_type: outro.transition_type,
            duration_sec: outro.duration_sec.max(CUT_DURATION_SEC),
        });
    }

    let (mut filter_complex, mut maps, total_duration_sec) =
        build_transition_graph(&graph_clips, target, &junctions);

    if let Some(movie_audio) = &request.movie_audio_override {
        let music_input_index = next_input_index;
        args.push("-i".into());
        args.push(movie_audio.source_path.clone());

        // If the timeline had audio, its "-map" "[aout]" pair is replaced by
        // the mixed/replaced track below rather than mapped twice.
        let base_audio_label = maps.iter().position(|m| m == "[aout]").map(|pos| {
            maps.drain(pos - 1..=pos);
            "aout".to_string()
        });

        let spec = BackgroundMusicSpec {
            input_index: music_input_index,
            blend_mode: movie_audio.blend_mode,
            fill_mode: movie_audio.fill_mode,
            gain_db: movie_audio.gain_db,
            fade_in_sec: movie_audio.fade_in_sec,
            fade_out_sec: movie_audio.fade_out_sec,
        };
        filter_complex.push_str(&build_background_music_chain(
            &spec,
            base_audio_label.as_deref(),
            total_duration_sec,
            48000,
            "finalaudio",
        ));
        maps.push("-map".into());
        maps.push("[finalaudio]".into());
    }

    args.push("-filter_complex".into());
    args.push(filter_complex);
    args.extend(maps);
    args.push("-c:v".into());
    args.push(request.export_settings.video_codec.clone());
    args.push("-preset".into());
    args.push(request.export_settings.x264_preset.clone());
    args.push("-crf".into());
    args.push(request.export_settings.crf.to_string());
    args.push("-pix_fmt".into());
    args.push("yuv420p".into());
    args.push("-c:a".into());
    args.push(request.export_settings.audio_codec.clone());
    args.push("-b:a".into());
    args.push(format!("{}k", request.export_settings.audio_bitrate_kbps));
    if request.export_settings.faststart {
        args.push("-movflags".into());
        args.push("+faststart".into());
    }

    (args, total_duration_sec)
}

/// Shared by `export_project` and `render_preview` - the two commands differ
/// only in what `ExportRequest` they build (real output path + user's chosen
/// quality settings, vs. a temp cache-dir path + fixed fast/low-quality
/// settings), everything downstream (spawn, progress events, cancel,
/// cleanup) is identical.
async fn run_export(
    app: &AppHandle,
    state: &State<'_, ExportHandle>,
    request: ExportRequest,
) -> Result<ExportResult, String> {
    if request.clips.is_empty() {
        return Err("project has no clips".into());
    }
    state.cancelled.store(false, Ordering::SeqCst);

    let font_path = app
        .path()
        .resolve("resources/NotoSans-Regular.ttf", BaseDirectory::Resource)
        .map_err(|e| format!("failed to resolve bundled font: {e}"))?;
    let text_card_paths = write_text_card_files(&request.clips)?;
    let cleanup_text_cards = || {
        for path in text_card_paths.values() {
            let _ = std::fs::remove_file(path);
        }
    };

    let (mut args, total_duration_sec) =
        build_ffmpeg_args(&request, &text_card_paths, &font_path.to_string_lossy());
    args.push("-progress".into());
    args.push("pipe:1".into());
    args.push("-nostats".into());
    args.push(request.output_path.clone());

    let shell = app.shell();
    let sidecar = match shell.sidecar("ffmpeg").or_else(|_| shell.sidecar("binaries/ffmpeg")) {
        Ok(s) => s,
        Err(e) => {
            cleanup_text_cards();
            return Err(format!("failed to resolve ffmpeg sidecar: {e}"));
        }
    };
    let (mut rx, child) = match sidecar.args(args).spawn() {
        Ok(v) => v,
        Err(e) => {
            cleanup_text_cards();
            return Err(format!("failed to spawn ffmpeg: {e}"));
        }
    };
    *state.child.lock().unwrap() = Some(child);

    let mut parser = ProgressParser::new(total_duration_sec);
    let mut stderr_tail = String::new();
    let mut success = false;

    while let Some(event) = rx.recv().await {
        match event {
            CommandEvent::Stdout(bytes) => {
                let text = String::from_utf8_lossy(&bytes);
                for line in text.lines() {
                    if let Some(progress) = parser.feed_line(line) {
                        let _ = app.emit("export://progress", &progress);
                    }
                }
            }
            CommandEvent::Stderr(bytes) => {
                stderr_tail.push_str(&String::from_utf8_lossy(&bytes));
            }
            CommandEvent::Terminated(payload) => {
                success = payload.code == Some(0);
            }
            _ => {}
        }
    }
    *state.child.lock().unwrap() = None;
    cleanup_text_cards();

    if !success {
        if state.cancelled.swap(false, Ordering::SeqCst) {
            let _ = std::fs::remove_file(&request.output_path);
            return Err("cancelled".into());
        }
        let tail: String = stderr_tail
            .lines()
            .rev()
            .take(20)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect::<Vec<_>>()
            .join("\n");
        return Err(format!("ffmpeg export failed:\n{tail}"));
    }

    Ok(ExportResult {
        output_path: request.output_path,
    })
}

#[tauri::command]
pub async fn export_project(
    app: AppHandle,
    state: State<'_, ExportHandle>,
    request: ExportRequest,
) -> Result<ExportResult, String> {
    run_export(&app, &state, request).await
}

/// Same timeline data as [`ExportRequest`], minus the fields the preview
/// pipeline fixes itself (output path, quality settings) - the whole point
/// is the caller can't accidentally ask for a slow, full-resolution preview.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PreviewRequest {
    pub clips: Vec<Clip>,
    pub transitions: Vec<Transition>,
    pub movie_audio_override: Option<MovieAudioOverride>,
    #[serde(default)]
    pub intro_transition: Option<EdgeTransition>,
    #[serde(default)]
    pub outro_transition: Option<EdgeTransition>,
}

const PREVIEW_FILE_PREFIX: &str = "videoclipper_preview_";

/// Renders the whole timeline (clips + transitions + edge transitions +
/// movie audio) at a small/fast preset to a file in the app's cache dir, so
/// the user can watch the actual composited result - including transitions -
/// before committing to a full-quality export. This is the "Rendera
/// förhandsvisning" fallback the original plan scoped v1's preview down to,
/// rather than a real-time in-browser compositor.
///
/// The cache dir (not a bare temp dir) matters: the frontend reads the
/// result back via the fs plugin. `std::env::temp_dir()` (`/tmp` on Linux)
/// falls outside every capability scope this app declares, while the app's
/// cache dir has its own explicit `$APPCACHE/**/*` entry (needed because
/// `tauri-plugin-fs`'s `read_file` defaults `require_literal_leading_dot` to
/// `true` on Unix, unlike core's own fs scope - the broader `$HOME/**/*`
/// entry does NOT match a dot-prefixed path segment like `.cache` there,
/// confirmed empirically, not just from docs). Each render gets a fresh
/// unique filename (old ones are swept first) rather than a fixed name, so a
/// second preview after an edit is guaranteed to be read back fresh instead
/// of the frontend's blob-URL cache reusing the previous render for an
/// unchanged path.
#[tauri::command]
pub async fn render_preview(
    app: AppHandle,
    state: State<'_, ExportHandle>,
    request: PreviewRequest,
) -> Result<ExportResult, String> {
    if request.clips.is_empty() {
        return Err("project has no clips".into());
    }

    let cache_dir = app
        .path()
        .app_cache_dir()
        .map_err(|e| format!("failed to resolve cache dir: {e}"))?;
    std::fs::create_dir_all(&cache_dir).map_err(|e| format!("failed to create cache dir: {e}"))?;

    if let Ok(entries) = std::fs::read_dir(&cache_dir) {
        for entry in entries.flatten() {
            if entry.file_name().to_string_lossy().starts_with(PREVIEW_FILE_PREFIX) {
                let _ = std::fs::remove_file(entry.path());
            }
        }
    }
    let output_path = cache_dir.join(format!("{PREVIEW_FILE_PREFIX}{}.mp4", uuid::Uuid::new_v4()));

    let export_request = ExportRequest {
        clips: request.clips,
        transitions: request.transitions,
        movie_audio_override: request.movie_audio_override,
        intro_transition: request.intro_transition,
        outro_transition: request.outro_transition,
        export_settings: ExportSettings {
            preset: "preview".into(),
            container: "mp4".into(),
            video_codec: "libx264".into(),
            crf: 30,
            x264_preset: "ultrafast".into(),
            max_width: 640,
            fps: 24,
            audio_codec: "aac".into(),
            audio_bitrate_kbps: 96,
            faststart: true,
        },
        output_path: output_path.to_string_lossy().to_string(),
    };

    run_export(&app, &state, export_request).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{AudioMode, AudioOverride, ClipSource};

    fn clip(id: &str, source_path: &str, has_audio: bool, audio_override: Option<AudioOverride>) -> Clip {
        Clip {
            id: id.into(),
            source: ClipSource::Video,
            source_path: source_path.into(),
            source_duration_sec: 5.0,
            trim_in_sec: 0.0,
            trim_out_sec: 3.0,
            has_audio,
            audio_override,
        }
    }

    fn image_clip(id: &str, source_path: &str, duration_sec: f64) -> Clip {
        Clip {
            id: id.into(),
            source: ClipSource::Image,
            source_path: source_path.into(),
            source_duration_sec: duration_sec,
            trim_in_sec: 0.0,
            trim_out_sec: duration_sec,
            has_audio: false,
            audio_override: None,
        }
    }

    fn text_card_clip(id: &str, text: &str, bg: &str, fg: &str, duration_sec: f64) -> Clip {
        Clip {
            id: id.into(),
            source: ClipSource::TextCard {
                text: text.into(),
                background_color: bg.into(),
                font_color: fg.into(),
            },
            source_path: String::new(),
            source_duration_sec: duration_sec,
            trim_in_sec: 0.0,
            trim_out_sec: duration_sec,
            has_audio: false,
            audio_override: None,
        }
    }

    fn request(clips: Vec<Clip>, movie_audio_override: Option<MovieAudioOverride>) -> ExportRequest {
        ExportRequest {
            clips,
            transitions: vec![],
            movie_audio_override,
            intro_transition: None,
            outro_transition: None,
            export_settings: ExportSettings::default(),
            output_path: "out.mp4".into(),
        }
    }

    #[test]
    fn per_clip_override_and_movie_audio_inputs_are_indexed_in_declaration_order() {
        // clip0 (index 0), clip1 (index 1, no override), clip0's override
        // file (index 2, the only extra input), movie music (index 3).
        let clips = vec![
            clip(
                "c0",
                "clip0.mp4",
                true,
                Some(AudioOverride { source_path: "override0.mp3".into(), mode: AudioMode::Loop, gain_db: -3.0 }),
            ),
            clip("c1", "clip1.mp4", true, None),
        ];
        let movie_audio = MovieAudioOverride {
            source_path: "music.mp3".into(),
            blend_mode: AudioBlendMode::Mix,
            fill_mode: AudioMode::Loop,
            gain_db: -18.0,
            fade_in_sec: 0.5,
            fade_out_sec: 0.5,
        };
        let (args, _total) = build_ffmpeg_args(&request(clips, Some(movie_audio)), &HashMap::new(), "/fake/font.ttf");

        let inputs: Vec<&str> = args
            .windows(2)
            .filter(|w| w[0] == "-i")
            .map(|w| w[1].as_str())
            .collect();
        assert_eq!(inputs, vec!["clip0.mp4", "clip1.mp4", "override0.mp3", "music.mp3"]);

        let filter_complex = args
            .iter()
            .position(|a| a == "-filter_complex")
            .map(|i| args[i + 1].as_str())
            .unwrap();
        assert!(filter_complex.contains("[2:a]")); // clip0's replacement audio
        assert!(filter_complex.contains("[3:a]")); // movie music input
        assert!(filter_complex.contains("amix")); // Mix blend, not Replace
    }

    #[test]
    fn movie_audio_replace_mode_drops_per_clip_overrides_and_their_inputs() {
        let clips = vec![
            clip(
                "c0",
                "clip0.mp4",
                true,
                Some(AudioOverride { source_path: "override0.mp3".into(), mode: AudioMode::Loop, gain_db: 0.0 }),
            ),
            clip("c1", "clip1.mp4", true, None),
        ];
        let movie_audio = MovieAudioOverride {
            source_path: "music.mp3".into(),
            blend_mode: AudioBlendMode::Replace,
            fill_mode: AudioMode::Pad,
            gain_db: 0.0,
            fade_in_sec: 0.0,
            fade_out_sec: 0.0,
        };
        let (args, _total) = build_ffmpeg_args(&request(clips, Some(movie_audio)), &HashMap::new(), "/fake/font.ttf");

        // override0.mp3 must never be added as an input - Replace mode makes
        // the per-clip override moot, so it shouldn't even reach ffmpeg.
        let inputs: Vec<&str> = args
            .windows(2)
            .filter(|w| w[0] == "-i")
            .map(|w| w[1].as_str())
            .collect();
        assert_eq!(inputs, vec!["clip0.mp4", "clip1.mp4", "music.mp3"]);

        let filter_complex = args
            .iter()
            .position(|a| a == "-filter_complex")
            .map(|i| args[i + 1].as_str())
            .unwrap();
        assert!(filter_complex.contains("[2:a]")); // music is input index 2 now, not 3
        assert!(!filter_complex.contains("amix")); // Replace, not Mix
    }

    #[test]
    fn mixed_timeline_video_image_textcard_indexes_one_input_per_clip() {
        // clip0 = video (index 0), clip1 = image (index 1, -loop 1), clip2 =
        // text card (index 2, -f lavfi color=). No per-clip overrides or
        // movie audio here, so the input list must be exactly these three,
        // one -i group per clip, in declaration order.
        let clips = vec![
            clip("c0", "clip0.mp4", true, None),
            image_clip("c1", "photo.jpg", 2.5),
            text_card_clip("c2", "Kapitel 1", "#000000", "#FFFFFF", 3.0),
        ];
        let mut text_card_paths = HashMap::new();
        text_card_paths.insert("c2".to_string(), PathBuf::from("/tmp/videoclipper_textcard_c2.txt"));

        let (args, _total) =
            build_ffmpeg_args(&request(clips, None), &text_card_paths, "/opt/videoclipper/NotoSans-Regular.ttf");

        // One -i per clip, in declaration order: clip0's real file, then
        // photo.jpg (preceded by -loop 1), then clip2's generated lavfi
        // color= source (preceded by -f lavfi) standing in for a file.
        let inputs: Vec<&str> = args
            .windows(2)
            .filter(|w| w[0] == "-i")
            .map(|w| w[1].as_str())
            .collect();
        assert_eq!(inputs.len(), 3);
        assert_eq!(inputs[0], "clip0.mp4");
        assert_eq!(inputs[1], "photo.jpg");
        assert!(inputs[2].starts_with("color=c=#000000:s="));
        assert!(args.windows(3).any(|w| w == ["-loop", "1", "-i"]));
        assert!(args.windows(2).any(|w| w == ["-f", "lavfi"]));

        // clip2 (text card) is input index 2 - its own drawtext must
        // reference [2:v], and no other clip should have drawtext at all.
        let filter_complex = args
            .iter()
            .position(|a| a == "-filter_complex")
            .map(|i| args[i + 1].as_str())
            .unwrap();
        assert!(filter_complex.contains("[2:v]trim=start=0:end=3,setpts=PTS-STARTPTS,drawtext="));
        assert!(filter_complex.contains("textfile='/tmp/videoclipper_textcard_c2.txt'"));
        assert!(filter_complex.contains("fontfile='/opt/videoclipper/NotoSans-Regular.ttf'"));
        assert!(filter_complex.contains("fontcolor=#FFFFFF"));
        assert_eq!(filter_complex.matches("drawtext=").count(), 1);
    }

    #[test]
    fn intro_transition_prepends_black_clip_wired_with_the_chosen_xfade_type() {
        let mut req = request(
            vec![clip("c0", "clip0.mp4", true, None), clip("c1", "clip1.mp4", true, None)],
            None,
        );
        req.intro_transition = Some(EdgeTransition { transition_type: TransitionType::Wipeleft, duration_sec: 1.0 });
        let (args, total_with_intro) = build_ffmpeg_args(&req, &HashMap::new(), "/fake/font.ttf");

        // An extra generated black-color input, with no explicit `d=` (left
        // infinite, capped by the normalize chain's own trim= like -loop 1
        // images are) - not counted among the two real clip inputs.
        let inputs: Vec<&str> = args
            .windows(2)
            .filter(|w| w[0] == "-i")
            .map(|w| w[1].as_str())
            .collect();
        assert_eq!(inputs.len(), 3);
        assert!(inputs[2].starts_with("color=c=black:s="));
        assert!(!inputs[2].contains(":d="));

        let filter_complex = args
            .iter()
            .position(|a| a == "-filter_complex")
            .map(|i| args[i + 1].as_str())
            .unwrap();
        // Black clip becomes v0 (input index 2, pushed last but positioned
        // first in the graph), real clip0 becomes v1: the chosen type must
        // wire the boundary junction, not just fall back to a plain fade.
        assert!(filter_complex.contains("[v0][v1]xfade=transition=wipeleft"));

        // Duration is unaffected by the intro - it overlays into clip0's
        // own runtime rather than adding time, same invariant as the old
        // fade-only implementation had.
        let req_no_intro = request(
            vec![clip("c0", "clip0.mp4", true, None), clip("c1", "clip1.mp4", true, None)],
            None,
        );
        let (_args, total_without_intro) = build_ffmpeg_args(&req_no_intro, &HashMap::new(), "/fake/font.ttf");
        assert!((total_with_intro - total_without_intro).abs() < 1e-9);
    }

    #[test]
    fn outro_transition_appends_black_clip_wired_with_the_chosen_xfade_type() {
        let mut req = request(vec![clip("c0", "clip0.mp4", true, None)], None);
        req.outro_transition = Some(EdgeTransition { transition_type: TransitionType::Slideup, duration_sec: 0.8 });
        let (args, _total) = build_ffmpeg_args(&req, &HashMap::new(), "/fake/font.ttf");

        let inputs: Vec<&str> = args
            .windows(2)
            .filter(|w| w[0] == "-i")
            .map(|w| w[1].as_str())
            .collect();
        assert_eq!(inputs.len(), 2);
        assert!(inputs[1].starts_with("color=c=black:s="));

        let filter_complex = args
            .iter()
            .position(|a| a == "-filter_complex")
            .map(|i| args[i + 1].as_str())
            .unwrap();
        // Real clip0 is v0, the appended black clip is v1.
        assert!(filter_complex.contains("[v0][v1]xfade=transition=slideup"));
    }

    #[test]
    fn no_edge_transitions_leaves_maps_as_plain_vout_aout() {
        let req = request(vec![clip("c0", "clip0.mp4", true, None)], None);
        let (args, _total) = build_ffmpeg_args(&req, &HashMap::new(), "/fake/font.ttf");
        let map_args: Vec<&str> = args
            .windows(2)
            .filter(|w| w[0] == "-map")
            .map(|w| w[1].as_str())
            .collect();
        assert_eq!(map_args, vec!["[vout]", "[aout]"]);
    }

    /// End-to-end sanity check against the real vendored ffmpeg binary:
    /// generates tiny synthetic clips/audio with distinct tones, builds the
    /// exact production args for a per-clip-override + movie-audio-mix
    /// scenario, and confirms ffmpeg actually accepts and runs the graph.
    /// Also covers M10's image/text-card clips in the same timeline, since
    /// that's exactly the kind of interaction (shared input-index
    /// bookkeeping, drawtext alongside per-clip audio replacement) unit
    /// tests can assert the *shape* of but not that ffmpeg actually accepts
    /// it. Ignored by default (needs the vendored sidecar binaries + bundled
    /// font); run with `cargo test -- --ignored real_ffmpeg`.
    #[test]
    #[ignore]
    fn real_ffmpeg_accepts_mixed_video_image_textcard_with_overrides_and_movie_audio() {
        let dir = std::env::temp_dir().join("videoclipper_m6_m10_verify");
        let _ = std::fs::create_dir_all(&dir);
        let ffmpeg = std::env::current_dir()
            .unwrap()
            .join("binaries/ffmpeg-x86_64-unknown-linux-gnu");
        assert!(ffmpeg.exists(), "run scripts/fetch-ffmpeg.sh linux first");
        let font = std::env::current_dir().unwrap().join("resources/NotoSans-Regular.ttf");
        assert!(font.exists(), "expected bundled font at src-tauri/resources/NotoSans-Regular.ttf");

        let gen = |name: &str, tone_hz: u32, out: &std::path::Path| {
            let status = std::process::Command::new(&ffmpeg)
                .args([
                    "-y",
                    "-f",
                    "lavfi",
                    "-i",
                    "testsrc=size=320x240:rate=25:duration=3",
                    "-f",
                    "lavfi",
                    "-i",
                    &format!("sine=frequency={tone_hz}:duration=3"),
                    "-shortest",
                    out.to_str().unwrap(),
                ])
                .status()
                .unwrap();
            assert!(status.success(), "failed to generate {name}");
        };

        let clip0 = dir.join("clip0.mp4");
        let clip1 = dir.join("clip1.mp4");
        let photo = dir.join("photo.jpg");
        let override0 = dir.join("override0.mp3");
        let music = dir.join("music.mp3");
        let out = dir.join("out.mp4");

        gen("clip0", 440, &clip0);
        gen("clip1", 220, &clip1);
        let status = std::process::Command::new(&ffmpeg)
            .args(["-y", "-f", "lavfi", "-i", "testsrc=size=320x240", "-frames:v", "1", photo.to_str().unwrap()])
            .status()
            .unwrap();
        assert!(status.success(), "failed to generate photo.jpg");
        let status = std::process::Command::new(&ffmpeg)
            .args(["-y", "-f", "lavfi", "-i", "sine=frequency=880:duration=1", override0.to_str().unwrap()])
            .status()
            .unwrap();
        assert!(status.success());
        let status = std::process::Command::new(&ffmpeg)
            .args(["-y", "-f", "lavfi", "-i", "sine=frequency=110:duration=2", music.to_str().unwrap()])
            .status()
            .unwrap();
        assert!(status.success());

        let clips = vec![
            clip(
                "c0",
                clip0.to_str().unwrap(),
                true,
                Some(AudioOverride { source_path: override0.to_str().unwrap().into(), mode: AudioMode::Loop, gain_db: -3.0 }),
            ),
            clip("c1", clip1.to_str().unwrap(), true, None),
            image_clip("c2", photo.to_str().unwrap(), 1.5),
            text_card_clip("c3", "Slut", "#000000", "#FFFFFF", 1.5),
        ];
        let movie_audio = MovieAudioOverride {
            source_path: music.to_str().unwrap().into(),
            blend_mode: AudioBlendMode::Mix,
            fill_mode: AudioMode::Loop,
            gain_db: -12.0,
            fade_in_sec: 0.2,
            fade_out_sec: 0.2,
        };
        let mut req = request(clips, Some(movie_audio));
        req.output_path = out.to_str().unwrap().into();
        let text_card_paths = write_text_card_files(&req.clips).unwrap();
        let (mut args, _total) = build_ffmpeg_args(&req, &text_card_paths, &font.to_string_lossy());
        args.push("-nostats".into());
        args.push(req.output_path.clone());

        let output = std::process::Command::new(&ffmpeg).args(&args).output().unwrap();
        for path in text_card_paths.values() {
            let _ = std::fs::remove_file(path);
        }
        assert!(
            output.status.success(),
            "ffmpeg failed:\n{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(out.exists());
        assert!(std::fs::metadata(&out).unwrap().len() > 0);
    }

    /// Regression test: replacing all audio (movie audio override in
    /// `Replace` mode) while the timeline's own clips have real audio used
    /// to make ffmpeg hard-fail with "Filter 'anull:default' has output 1
    /// (aout) unconnected" / "Error binding filtergraph inputs/outputs:
    /// Invalid argument" - the timeline's `[aout]` label was a genuine
    /// filter output that nothing ever consumed once Replace mode discarded
    /// it. A pure filtergraph-string test can assert the fixed graph
    /// *contains* `anullsink`, but only real ffmpeg proves the graph is
    /// actually valid. Also includes an intro edge transition since that's
    /// the exact combination that surfaced the bug. Ignored by default; run
    /// with `cargo test -- --ignored real_ffmpeg`.
    #[test]
    #[ignore]
    fn real_ffmpeg_accepts_replace_mode_movie_audio_with_real_timeline_audio_present() {
        let dir = std::env::temp_dir().join("videoclipper_replace_audio_verify");
        let _ = std::fs::create_dir_all(&dir);
        let ffmpeg = std::env::current_dir().unwrap().join("binaries/ffmpeg-x86_64-unknown-linux-gnu");
        assert!(ffmpeg.exists(), "run scripts/fetch-ffmpeg.sh linux first");

        let gen = |tone_hz: u32, out: &std::path::Path| {
            let status = std::process::Command::new(&ffmpeg)
                .args([
                    "-y", "-f", "lavfi", "-i", "testsrc=size=320x240:rate=25:duration=3",
                    "-f", "lavfi", "-i", &format!("sine=frequency={tone_hz}:duration=3"),
                    "-shortest", out.to_str().unwrap(),
                ])
                .status()
                .unwrap();
            assert!(status.success());
        };

        let clip0 = dir.join("clip0.mp4");
        let clip1 = dir.join("clip1.mp4");
        let music = dir.join("music.mp3");
        let out = dir.join("out.mp4");
        gen(440, &clip0);
        gen(220, &clip1);
        let status = std::process::Command::new(&ffmpeg)
            .args(["-y", "-f", "lavfi", "-i", "sine=frequency=110:duration=2", music.to_str().unwrap()])
            .status()
            .unwrap();
        assert!(status.success());

        let clips = vec![
            clip("c0", clip0.to_str().unwrap(), true, None),
            clip("c1", clip1.to_str().unwrap(), true, None),
        ];
        let movie_audio = MovieAudioOverride {
            source_path: music.to_str().unwrap().into(),
            blend_mode: AudioBlendMode::Replace,
            fill_mode: AudioMode::Loop,
            gain_db: 0.0,
            fade_in_sec: 0.0,
            fade_out_sec: 0.0,
        };
        let mut req = request(clips, Some(movie_audio));
        req.intro_transition = Some(EdgeTransition { transition_type: TransitionType::Fade, duration_sec: 0.5 });
        req.output_path = out.to_str().unwrap().into();

        let (mut args, _total) = build_ffmpeg_args(&req, &HashMap::new(), "/fake/font.ttf");
        args.push("-nostats".into());
        args.push(req.output_path.clone());

        let output = std::process::Command::new(&ffmpeg).args(&args).output().unwrap();
        assert!(output.status.success(), "ffmpeg failed:\n{}", String::from_utf8_lossy(&output.stderr));
        assert!(out.exists());
        assert!(std::fs::metadata(&out).unwrap().len() > 0);
    }

    /// End-to-end sanity check for intro/outro edge transitions against the
    /// real vendored ffmpeg binary: confirms (a) they don't change the
    /// output's total duration (the synthetic black clip is consumed
    /// entirely by the crossfade rather than adding time) and (b) frames
    /// actually go near-black during the transition windows but not in the
    /// middle of the timeline - a pure filtergraph-string test can assert
    /// the `xfade=` filter text is present but not that ffmpeg actually
    /// renders black pixels from it. Ignored by default; run with
    /// `cargo test -- --ignored real_ffmpeg`.
    #[test]
    #[ignore]
    fn real_ffmpeg_edge_transitions_produce_near_black_frames_without_changing_duration() {
        let dir = std::env::temp_dir().join("videoclipper_edge_fade_verify");
        let _ = std::fs::create_dir_all(&dir);
        let ffmpeg = std::env::current_dir().unwrap().join("binaries/ffmpeg-x86_64-unknown-linux-gnu");
        let ffprobe = std::env::current_dir().unwrap().join("binaries/ffprobe-x86_64-unknown-linux-gnu");
        assert!(ffmpeg.exists(), "run scripts/fetch-ffmpeg.sh linux first");
        assert!(ffprobe.exists(), "run scripts/fetch-ffmpeg.sh linux first");

        let gen = |tone_hz: u32, out: &std::path::Path| {
            let status = std::process::Command::new(&ffmpeg)
                .args([
                    "-y", "-f", "lavfi", "-i", "testsrc=size=320x240:rate=25:duration=3",
                    "-f", "lavfi", "-i", &format!("sine=frequency={tone_hz}:duration=3"),
                    "-shortest", out.to_str().unwrap(),
                ])
                .status()
                .unwrap();
            assert!(status.success());
        };

        let clip0 = dir.join("clip0.mp4");
        let clip1 = dir.join("clip1.mp4");
        let out = dir.join("out.mp4");
        gen(440, &clip0);
        gen(220, &clip1);

        let clips = vec![
            clip("c0", clip0.to_str().unwrap(), true, None),
            clip("c1", clip1.to_str().unwrap(), true, None),
        ];
        let mut req = request(clips, None);
        // Kept on the plain `Fade` type (rather than e.g. a wipe) so the
        // near-black brightness assertions below stay valid - a non-fade
        // type wouldn't necessarily be uniformly dark at any given instant.
        req.intro_transition = Some(EdgeTransition { transition_type: TransitionType::Fade, duration_sec: 0.5 });
        req.outro_transition = Some(EdgeTransition { transition_type: TransitionType::Fade, duration_sec: 0.5 });
        req.output_path = out.to_str().unwrap().into();

        let (mut args, total_duration_sec) = build_ffmpeg_args(&req, &HashMap::new(), "/fake/font.ttf");
        args.push("-nostats".into());
        args.push(req.output_path.clone());

        let output = std::process::Command::new(&ffmpeg).args(&args).output().unwrap();
        assert!(output.status.success(), "ffmpeg failed:\n{}", String::from_utf8_lossy(&output.stderr));

        // (a) duration unchanged: the fades overlay the existing video/audio,
        // they don't extend the timeline.
        let probe = std::process::Command::new(&ffprobe)
            .args(["-v", "quiet", "-show_entries", "format=duration", "-of", "csv=p=0", out.to_str().unwrap()])
            .output()
            .unwrap();
        let actual_duration: f64 = String::from_utf8_lossy(&probe.stdout).trim().parse().unwrap();
        assert!(
            (actual_duration - total_duration_sec).abs() < 0.3,
            "expected output duration ~{total_duration_sec}s, got {actual_duration}s"
        );

        // (b) grab a grayscale frame at a given timestamp and return its mean
        // brightness (0 = black, 255 = white).
        let mean_brightness_at = |timestamp_sec: f64| -> f64 {
            let frame = std::process::Command::new(&ffmpeg)
                .args([
                    "-ss", &timestamp_sec.to_string(), "-i", out.to_str().unwrap(),
                    "-vframes", "1", "-f", "rawvideo", "-pix_fmt", "gray", "pipe:1",
                ])
                .output()
                .unwrap();
            assert!(!frame.stdout.is_empty(), "no frame decoded at t={timestamp_sec}");
            let sum: u64 = frame.stdout.iter().map(|&b| b as u64).sum();
            sum as f64 / frame.stdout.len() as f64
        };

        let near_start = mean_brightness_at(0.05);
        let near_end = mean_brightness_at((total_duration_sec - 0.05).max(0.0));
        let middle = mean_brightness_at(total_duration_sec / 2.0);

        assert!(near_start < 30.0, "expected near-black frame during intro fade, got mean brightness {near_start}");
        assert!(near_end < 30.0, "expected near-black frame during outro fade, got mean brightness {near_end}");
        assert!(middle > 60.0, "expected a normal (non-faded) frame in the middle, got mean brightness {middle}");
    }

    /// Exercises the OS-level mechanics `cancel_export` relies on:
    /// `CommandChild::kill()` (via the `shared_child` crate) is a thin
    /// wrapper around `std::process::Child::kill()`, so killing a real,
    /// actively-encoding ffmpeg process here is a direct proxy for what
    /// happens when a user clicks "Avbryt" mid-export. Confirms the process
    /// actually dies promptly (not just eventually) and that the partial
    /// output file it leaves behind - exactly what `export_project`'s
    /// cancelled branch deletes - exists and is removable.
    #[test]
    #[ignore]
    fn killing_a_running_ffmpeg_process_terminates_it_and_leaves_a_removable_partial_file() {
        use std::process::{Command, Stdio};
        use std::time::{Duration, Instant};

        let dir = std::env::temp_dir().join("videoclipper_cancel_verify");
        let _ = std::fs::create_dir_all(&dir);
        let ffmpeg = std::env::current_dir()
            .unwrap()
            .join("binaries/ffmpeg-x86_64-unknown-linux-gnu");
        assert!(ffmpeg.exists(), "run scripts/fetch-ffmpeg.sh linux first");
        let out = dir.join("partial.mp4");
        let _ = std::fs::remove_file(&out);

        // Long enough (slow preset, 30s of synthetic video) that a kill sent
        // shortly after spawn lands well before natural completion.
        let mut child = Command::new(&ffmpeg)
            .args([
                "-y",
                "-f",
                "lavfi",
                "-i",
                "testsrc=size=1280x720:rate=30:duration=30",
                "-c:v",
                "libx264",
                "-preset",
                "veryslow",
                out.to_str().unwrap(),
            ])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("failed to spawn ffmpeg");

        std::thread::sleep(Duration::from_millis(400));
        child.kill().expect("kill() should succeed on a running process");

        let deadline = Instant::now() + Duration::from_secs(5);
        let status = loop {
            if let Some(status) = child.try_wait().expect("try_wait failed") {
                break status;
            }
            assert!(Instant::now() < deadline, "process did not die within 5s of kill()");
            std::thread::sleep(Duration::from_millis(50));
        };
        assert!(!status.success(), "a killed process must not report success");

        assert!(out.exists(), "expected a partial output file to have been written before the kill");
        assert!(std::fs::metadata(&out).unwrap().len() > 0);
        std::fs::remove_file(&out).expect("partial output file must be removable, as export_project's cancel path does");

        let _ = std::fs::remove_dir(&dir);
    }
}
