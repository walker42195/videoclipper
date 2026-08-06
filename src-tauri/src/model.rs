use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AudioMode {
    #[serde(rename = "loop")]
    Loop,
    #[serde(rename = "pad")]
    Pad,
}

/// How a whole-movie audio track combines with whatever audio the timeline
/// already has (original per-clip audio, or per-clip overrides).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AudioBlendMode {
    /// Drop all existing audio and use only this track.
    Replace,
    /// Keep existing audio and layer this track underneath it (e.g.
    /// background music that doesn't remove the clips' own sound).
    Mix,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioOverride {
    pub source_path: String,
    pub mode: AudioMode,
    pub gain_db: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Clip {
    pub id: String,
    pub source_path: String,
    /// Full duration of the source file, as reported by probe_clip - the
    /// upper bound trim handles in the UI can't drag `trim_out_sec` past.
    pub source_duration_sec: f64,
    pub trim_in_sec: f64,
    pub trim_out_sec: f64,
    pub audio_override: Option<AudioOverride>,
}

impl Clip {
    pub fn duration_sec(&self) -> f64 {
        self.trim_out_sec - self.trim_in_sec
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TransitionType {
    Fade,
    Dissolve,
    Fadeblack,
    Fadewhite,
    Wipeleft,
    Wiperight,
    Slideleft,
    Slideright,
    Circleopen,
    Smoothleft,
}

impl TransitionType {
    /// The literal string ffmpeg's `xfade` filter expects for `transition=`.
    pub fn xfade_name(&self) -> &'static str {
        match self {
            TransitionType::Fade => "fade",
            TransitionType::Dissolve => "dissolve",
            TransitionType::Fadeblack => "fadeblack",
            TransitionType::Fadewhite => "fadewhite",
            TransitionType::Wipeleft => "wipeleft",
            TransitionType::Wiperight => "wiperight",
            TransitionType::Slideleft => "slideleft",
            TransitionType::Slideright => "slideright",
            TransitionType::Circleopen => "circleopen",
            TransitionType::Smoothleft => "smoothleft",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Transition {
    pub from_clip_id: String,
    pub to_clip_id: String,
    #[serde(rename = "type")]
    pub transition_type: TransitionType,
    pub duration_sec: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MovieAudioOverride {
    pub source_path: String,
    pub blend_mode: AudioBlendMode,
    /// How to stretch the track to the movie's full length if it's shorter.
    pub fill_mode: AudioMode,
    pub gain_db: f64,
    pub fade_in_sec: f64,
    pub fade_out_sec: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportSettings {
    pub preset: String,
    pub container: String,
    pub video_codec: String,
    pub crf: u32,
    pub x264_preset: String,
    pub max_width: u32,
    pub fps: u32,
    pub audio_codec: String,
    pub audio_bitrate_kbps: u32,
    pub faststart: bool,
}

impl Default for ExportSettings {
    fn default() -> Self {
        Self {
            preset: "web-recommended".into(),
            container: "mp4".into(),
            video_codec: "libx264".into(),
            crf: 20,
            x264_preset: "medium".into(),
            max_width: 1920,
            fps: 30,
            audio_codec: "aac".into(),
            audio_bitrate_kbps: 192,
            faststart: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Project {
    pub version: u32,
    pub id: String,
    pub name: String,
    pub clips: Vec<Clip>,
    pub transitions: Vec<Transition>,
    pub movie_audio_override: Option<MovieAudioOverride>,
    pub export_settings: ExportSettings,
}

impl Project {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            version: 1,
            id: uuid::Uuid::new_v4().to_string(),
            name: name.into(),
            clips: Vec::new(),
            transitions: Vec::new(),
            movie_audio_override: None,
            export_settings: ExportSettings::default(),
        }
    }

    /// Transition for the junction right after `clip_id`, if any.
    pub fn transition_after(&self, clip_id: &str) -> Option<&Transition> {
        self.transitions.iter().find(|t| t.from_clip_id == clip_id)
    }

    /// Drop transitions whose two clip ids are no longer adjacent in `clips`
    /// (call this whenever the clip order changes).
    pub fn prune_invalid_transitions(&mut self) {
        let adjacent_pairs: Vec<(String, String)> = self
            .clips
            .windows(2)
            .map(|w| (w[0].id.clone(), w[1].id.clone()))
            .collect();
        self.transitions.retain(|t| {
            adjacent_pairs
                .iter()
                .any(|(a, b)| *a == t.from_clip_id && *b == t.to_clip_id)
        });
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClipMeta {
    pub duration_sec: f64,
    pub width: u32,
    pub height: u32,
    pub fps: f64,
    pub sample_rate: u32,
    pub has_audio: bool,
}
