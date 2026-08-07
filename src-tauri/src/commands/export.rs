use crate::ffmpeg::filtergraph::{
    build_background_music_chain, build_transition_graph, AudioReplacement, BackgroundMusicSpec,
    GraphClip, NormalizeTarget, TransitionJunction, CUT_DURATION_SEC,
};
use crate::ffmpeg::progress::ProgressParser;
use crate::model::{AudioBlendMode, Clip, ExportSettings, MovieAudioOverride, Transition, TransitionType};
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use tauri::{AppHandle, Emitter, State};
use tauri_plugin_shell::{process::CommandChild, process::CommandEvent, ShellExt};

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
fn build_ffmpeg_args(request: &ExportRequest) -> (Vec<String>, f64) {
    let target = NormalizeTarget {
        width: request.export_settings.max_width,
        height: request.export_settings.max_width * 9 / 16,
        fps: request.export_settings.fps,
        sample_rate: 48000,
    };

    let mut args: Vec<String> = vec!["-y".into()];
    for clip in &request.clips {
        args.push("-i".into());
        args.push(clip.source_path.clone());
    }
    let mut next_input_index = request.clips.len();

    let drop_clip_overrides = matches!(
        request.movie_audio_override.as_ref().map(|m| m.blend_mode),
        Some(AudioBlendMode::Replace)
    );

    let graph_clips: Vec<GraphClip> = request
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
            GraphClip {
                input_index: i,
                trim_in_sec: c.trim_in_sec,
                trim_out_sec: c.trim_out_sec,
                audio_replacement,
                has_audio: c.has_audio,
            }
        })
        .collect();

    let junctions = resolve_junctions(&request.clips, &request.transitions);
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

#[tauri::command]
pub async fn export_project(
    app: AppHandle,
    state: State<'_, ExportHandle>,
    request: ExportRequest,
) -> Result<ExportResult, String> {
    if request.clips.is_empty() {
        return Err("project has no clips".into());
    }
    state.cancelled.store(false, Ordering::SeqCst);

    let (mut args, total_duration_sec) = build_ffmpeg_args(&request);
    args.push("-progress".into());
    args.push("pipe:1".into());
    args.push("-nostats".into());
    args.push(request.output_path.clone());

    let shell = app.shell();
    let sidecar = shell
        .sidecar("ffmpeg")
        .map_err(|e| format!("failed to resolve ffmpeg sidecar: {e}"))?;
    let (mut rx, child) = sidecar
        .args(args)
        .spawn()
        .map_err(|e| format!("failed to spawn ffmpeg: {e}"))?;
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{AudioMode, AudioOverride};

    fn clip(id: &str, source_path: &str, has_audio: bool, audio_override: Option<AudioOverride>) -> Clip {
        Clip {
            id: id.into(),
            source_path: source_path.into(),
            source_duration_sec: 5.0,
            trim_in_sec: 0.0,
            trim_out_sec: 3.0,
            has_audio,
            audio_override,
        }
    }

    fn request(clips: Vec<Clip>, movie_audio_override: Option<MovieAudioOverride>) -> ExportRequest {
        ExportRequest {
            clips,
            transitions: vec![],
            movie_audio_override,
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
        let (args, _total) = build_ffmpeg_args(&request(clips, Some(movie_audio)));

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
        let (args, _total) = build_ffmpeg_args(&request(clips, Some(movie_audio)));

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

    /// End-to-end sanity check against the real vendored ffmpeg binary:
    /// generates tiny synthetic clips/audio with distinct tones, builds the
    /// exact production args for a per-clip-override + movie-audio-mix
    /// scenario, and confirms ffmpeg actually accepts and runs the graph.
    /// Ignored by default (needs the vendored sidecar binaries); run with
    /// `cargo test -- --ignored real_ffmpeg`.
    #[test]
    #[ignore]
    fn real_ffmpeg_accepts_per_clip_override_plus_movie_audio_mix_graph() {
        let dir = std::env::temp_dir().join("videoclipper_m6_verify");
        let _ = std::fs::create_dir_all(&dir);
        let ffmpeg = std::env::current_dir()
            .unwrap()
            .join("binaries/ffmpeg-x86_64-unknown-linux-gnu");
        assert!(ffmpeg.exists(), "run scripts/fetch-ffmpeg.sh linux first");

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
        let override0 = dir.join("override0.mp3");
        let music = dir.join("music.mp3");
        let out = dir.join("out.mp4");

        gen("clip0", 440, &clip0);
        gen("clip1", 220, &clip1);
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
        let (mut args, _total) = build_ffmpeg_args(&req);
        args.push("-nostats".into());
        args.push(req.output_path.clone());

        let output = std::process::Command::new(&ffmpeg).args(&args).output().unwrap();
        assert!(
            output.status.success(),
            "ffmpeg failed:\n{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(out.exists());
        assert!(std::fs::metadata(&out).unwrap().len() > 0);
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
