use crate::ffmpeg::sidecar::{run_sidecar_capture, run_sidecar_capture_bytes};
use crate::model::ClipMeta;
use base64::{engine::general_purpose::STANDARD, Engine};
use serde::Deserialize;
use tauri::AppHandle;

#[derive(Debug, Deserialize)]
struct FfprobeOutput {
    format: FfprobeFormat,
    streams: Vec<FfprobeStream>,
}

#[derive(Debug, Deserialize)]
struct FfprobeFormat {
    duration: Option<String>,
}

#[derive(Debug, Deserialize)]
struct FfprobeStream {
    codec_type: String,
    width: Option<u32>,
    height: Option<u32>,
    r_frame_rate: Option<String>,
    sample_rate: Option<String>,
    duration: Option<String>,
}

fn parse_frame_rate(s: &str) -> Option<f64> {
    // r_frame_rate looks like "30000/1001" or "30/1".
    let (num, den) = s.split_once('/')?;
    let num: f64 = num.parse().ok()?;
    let den: f64 = den.parse().ok()?;
    if den == 0.0 {
        None
    } else {
        Some(num / den)
    }
}

#[tauri::command]
pub async fn ffmpeg_version(app: AppHandle) -> Result<String, String> {
    let (stdout, stderr, success) =
        run_sidecar_capture(&app, "ffmpeg", vec!["-version".into()]).await?;
    if !success {
        return Err(format!("ffmpeg -version failed: {stderr}"));
    }
    Ok(stdout.lines().next().unwrap_or("unknown").to_string())
}

#[tauri::command]
pub async fn probe_clip(app: AppHandle, path: String) -> Result<ClipMeta, String> {
    let args = vec![
        "-v".into(),
        "quiet".into(),
        "-print_format".into(),
        "json".into(),
        "-show_format".into(),
        "-show_streams".into(),
        path.clone(),
    ];
    let (stdout, stderr, success) = run_sidecar_capture(&app, "ffprobe", args).await?;
    if !success {
        return Err(format!("ffprobe failed for '{path}': {stderr}"));
    }

    let parsed: FfprobeOutput =
        serde_json::from_str(&stdout).map_err(|e| format!("failed to parse ffprobe JSON: {e}"))?;

    let video_stream = parsed
        .streams
        .iter()
        .find(|s| s.codec_type == "video")
        .ok_or_else(|| format!("'{path}' has no video stream"))?;

    let audio_stream = parsed.streams.iter().find(|s| s.codec_type == "audio");

    let duration_sec = parsed
        .format
        .duration
        .as_deref()
        .or(video_stream.duration.as_deref())
        .and_then(|d| d.parse::<f64>().ok())
        .ok_or_else(|| format!("could not determine duration for '{path}'"))?;

    Ok(ClipMeta {
        duration_sec,
        width: video_stream.width.unwrap_or(0),
        height: video_stream.height.unwrap_or(0),
        fps: video_stream
            .r_frame_rate
            .as_deref()
            .and_then(parse_frame_rate)
            .unwrap_or(30.0),
        sample_rate: audio_stream
            .and_then(|a| a.sample_rate.as_deref())
            .and_then(|s| s.parse::<u32>().ok())
            .unwrap_or(48000),
        has_audio: audio_stream.is_some(),
    })
}

/// Extracts a single JPEG frame at `at_seconds` and returns it as a
/// `data:image/jpeg;base64,...` URL, small enough (~160px wide) to hand
/// straight to an `<img>` tag without touching the filesystem/asset scope.
#[tauri::command]
pub async fn extract_thumbnail(app: AppHandle, path: String, at_seconds: f64) -> Result<String, String> {
    let args = vec![
        "-ss".into(),
        at_seconds.max(0.0).to_string(),
        "-i".into(),
        path.clone(),
        "-frames:v".into(),
        "1".into(),
        "-vf".into(),
        "scale=160:-1".into(),
        "-f".into(),
        "image2".into(),
        "-vcodec".into(),
        "mjpeg".into(),
        "pipe:1".into(),
    ];
    let (stdout, stderr, success) = run_sidecar_capture_bytes(&app, "ffmpeg", args).await?;
    if !success || stdout.is_empty() {
        return Err(format!("thumbnail extraction failed for '{path}': {stderr}"));
    }
    Ok(format!("data:image/jpeg;base64,{}", STANDARD.encode(&stdout)))
}
