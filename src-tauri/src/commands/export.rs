use crate::ffmpeg::filtergraph::{
    build_transition_graph, GraphClip, NormalizeTarget, TransitionJunction, CUT_DURATION_SEC,
};
use crate::ffmpeg::progress::ProgressParser;
use crate::model::{Clip, ExportSettings, Transition, TransitionType};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter};
use tauri_plugin_shell::{process::CommandEvent, ShellExt};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportRequest {
    pub clips: Vec<Clip>,
    /// Sparse: only junctions the user put an explicit transition on need an
    /// entry here (matched by fromClipId/toClipId). Missing junctions render
    /// as a hard cut (see [`CUT_DURATION_SEC`]).
    pub transitions: Vec<Transition>,
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

/// v1 (M2/M3/M5): normalize + xfade/acrossfade-chained transitions (hard
/// cuts are just a very short transition, see [`resolve_junctions`]). Audio
/// replacement lands in M6.
#[tauri::command]
pub async fn export_project(app: AppHandle, request: ExportRequest) -> Result<ExportResult, String> {
    if request.clips.is_empty() {
        return Err("project has no clips".into());
    }

    let target = NormalizeTarget {
        width: request.export_settings.max_width,
        height: request.export_settings.max_width * 9 / 16,
        fps: request.export_settings.fps,
        sample_rate: 48000,
    };

    let graph_clips: Vec<GraphClip> = request
        .clips
        .iter()
        .enumerate()
        .map(|(i, c)| GraphClip {
            input_index: i,
            trim_in_sec: c.trim_in_sec,
            trim_out_sec: c.trim_out_sec,
            audio_input_index: None, // audio replacement lands in M6
            has_audio: true,         // TODO(M6): use probed ClipMeta.has_audio
        })
        .collect();

    let junctions = resolve_junctions(&request.clips, &request.transitions);
    let (filter_complex, maps, total_duration_sec) =
        build_transition_graph(&graph_clips, target, &junctions);

    let mut args: Vec<String> = vec!["-y".into()];
    for clip in &request.clips {
        args.push("-i".into());
        args.push(clip.source_path.clone());
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
    args.push("-progress".into());
    args.push("pipe:1".into());
    args.push("-nostats".into());
    args.push(request.output_path.clone());

    let shell = app.shell();
    let sidecar = shell
        .sidecar("ffmpeg")
        .map_err(|e| format!("failed to resolve ffmpeg sidecar: {e}"))?;
    let (mut rx, _child) = sidecar
        .args(args)
        .spawn()
        .map_err(|e| format!("failed to spawn ffmpeg: {e}"))?;

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

    if !success {
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
