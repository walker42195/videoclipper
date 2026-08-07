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
    /// Whether the source file has an audio stream at all, as reported by
    /// probe_clip. Clips without one get silence synthesized in their place
    /// so the audio chain stays uniform across a mixed timeline.
    pub has_audio: bool,
    pub audio_override: Option<AudioOverride>,
}

impl Clip {
    pub fn duration_sec(&self) -> f64 {
        self.trim_out_sec - self.trim_in_sec
    }
}

/// Mirrors ffmpeg's `xfade` filter's built-in `transition` values exactly
/// (verified against `ffmpeg -h filter=xfade`, transition indices 0-57;
/// `custom` (-1, a user-supplied expression) is intentionally not exposed).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TransitionType {
    Fade,
    Wipeleft,
    Wiperight,
    Wipeup,
    Wipedown,
    Slideleft,
    Slideright,
    Slideup,
    Slidedown,
    Circlecrop,
    Rectcrop,
    Distance,
    Fadeblack,
    Fadewhite,
    Radial,
    Smoothleft,
    Smoothright,
    Smoothup,
    Smoothdown,
    Circleopen,
    Circleclose,
    Vertopen,
    Vertclose,
    Horzopen,
    Horzclose,
    Dissolve,
    Pixelize,
    Diagtl,
    Diagtr,
    Diagbl,
    Diagbr,
    Hlslice,
    Hrslice,
    Vuslice,
    Vdslice,
    Hblur,
    Fadegrays,
    Wipetl,
    Wipetr,
    Wipebl,
    Wipebr,
    Squeezeh,
    Squeezev,
    Zoomin,
    Fadefast,
    Fadeslow,
    Hlwind,
    Hrwind,
    Vuwind,
    Vdwind,
    Coverleft,
    Coverright,
    Coverup,
    Coverdown,
    Revealleft,
    Revealright,
    Revealup,
    Revealdown,
}

impl TransitionType {
    /// The literal string ffmpeg's `xfade` filter expects for `transition=`.
    pub fn xfade_name(&self) -> &'static str {
        match self {
            TransitionType::Fade => "fade",
            TransitionType::Wipeleft => "wipeleft",
            TransitionType::Wiperight => "wiperight",
            TransitionType::Wipeup => "wipeup",
            TransitionType::Wipedown => "wipedown",
            TransitionType::Slideleft => "slideleft",
            TransitionType::Slideright => "slideright",
            TransitionType::Slideup => "slideup",
            TransitionType::Slidedown => "slidedown",
            TransitionType::Circlecrop => "circlecrop",
            TransitionType::Rectcrop => "rectcrop",
            TransitionType::Distance => "distance",
            TransitionType::Fadeblack => "fadeblack",
            TransitionType::Fadewhite => "fadewhite",
            TransitionType::Radial => "radial",
            TransitionType::Smoothleft => "smoothleft",
            TransitionType::Smoothright => "smoothright",
            TransitionType::Smoothup => "smoothup",
            TransitionType::Smoothdown => "smoothdown",
            TransitionType::Circleopen => "circleopen",
            TransitionType::Circleclose => "circleclose",
            TransitionType::Vertopen => "vertopen",
            TransitionType::Vertclose => "vertclose",
            TransitionType::Horzopen => "horzopen",
            TransitionType::Horzclose => "horzclose",
            TransitionType::Dissolve => "dissolve",
            TransitionType::Pixelize => "pixelize",
            TransitionType::Diagtl => "diagtl",
            TransitionType::Diagtr => "diagtr",
            TransitionType::Diagbl => "diagbl",
            TransitionType::Diagbr => "diagbr",
            TransitionType::Hlslice => "hlslice",
            TransitionType::Hrslice => "hrslice",
            TransitionType::Vuslice => "vuslice",
            TransitionType::Vdslice => "vdslice",
            TransitionType::Hblur => "hblur",
            TransitionType::Fadegrays => "fadegrays",
            TransitionType::Wipetl => "wipetl",
            TransitionType::Wipetr => "wipetr",
            TransitionType::Wipebl => "wipebl",
            TransitionType::Wipebr => "wipebr",
            TransitionType::Squeezeh => "squeezeh",
            TransitionType::Squeezev => "squeezev",
            TransitionType::Zoomin => "zoomin",
            TransitionType::Fadefast => "fadefast",
            TransitionType::Fadeslow => "fadeslow",
            TransitionType::Hlwind => "hlwind",
            TransitionType::Hrwind => "hrwind",
            TransitionType::Vuwind => "vuwind",
            TransitionType::Vdwind => "vdwind",
            TransitionType::Coverleft => "coverleft",
            TransitionType::Coverright => "coverright",
            TransitionType::Coverup => "coverup",
            TransitionType::Coverdown => "coverdown",
            TransitionType::Revealleft => "revealleft",
            TransitionType::Revealright => "revealright",
            TransitionType::Revealup => "revealup",
            TransitionType::Revealdown => "revealdown",
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

#[cfg(test)]
mod tests {
    use super::*;

    /// The authoritative list from `ffmpeg -h filter=xfade` (indices 0-57,
    /// `custom` excluded) - guards against typos in `xfade_name()` and
    /// silently-missing/extra variants.
    #[test]
    fn xfade_name_matches_authoritative_ffmpeg_list() {
        let expected = [
            "fade", "wipeleft", "wiperight", "wipeup", "wipedown", "slideleft", "slideright",
            "slideup", "slidedown", "circlecrop", "rectcrop", "distance", "fadeblack", "fadewhite",
            "radial", "smoothleft", "smoothright", "smoothup", "smoothdown", "circleopen",
            "circleclose", "vertopen", "vertclose", "horzopen", "horzclose", "dissolve", "pixelize",
            "diagtl", "diagtr", "diagbl", "diagbr", "hlslice", "hrslice", "vuslice", "vdslice",
            "hblur", "fadegrays", "wipetl", "wipetr", "wipebl", "wipebr", "squeezeh", "squeezev",
            "zoomin", "fadefast", "fadeslow", "hlwind", "hrwind", "vuwind", "vdwind", "coverleft",
            "coverright", "coverup", "coverdown", "revealleft", "revealright", "revealup",
            "revealdown",
        ];
        let actual = [
            TransitionType::Fade,
            TransitionType::Wipeleft,
            TransitionType::Wiperight,
            TransitionType::Wipeup,
            TransitionType::Wipedown,
            TransitionType::Slideleft,
            TransitionType::Slideright,
            TransitionType::Slideup,
            TransitionType::Slidedown,
            TransitionType::Circlecrop,
            TransitionType::Rectcrop,
            TransitionType::Distance,
            TransitionType::Fadeblack,
            TransitionType::Fadewhite,
            TransitionType::Radial,
            TransitionType::Smoothleft,
            TransitionType::Smoothright,
            TransitionType::Smoothup,
            TransitionType::Smoothdown,
            TransitionType::Circleopen,
            TransitionType::Circleclose,
            TransitionType::Vertopen,
            TransitionType::Vertclose,
            TransitionType::Horzopen,
            TransitionType::Horzclose,
            TransitionType::Dissolve,
            TransitionType::Pixelize,
            TransitionType::Diagtl,
            TransitionType::Diagtr,
            TransitionType::Diagbl,
            TransitionType::Diagbr,
            TransitionType::Hlslice,
            TransitionType::Hrslice,
            TransitionType::Vuslice,
            TransitionType::Vdslice,
            TransitionType::Hblur,
            TransitionType::Fadegrays,
            TransitionType::Wipetl,
            TransitionType::Wipetr,
            TransitionType::Wipebl,
            TransitionType::Wipebr,
            TransitionType::Squeezeh,
            TransitionType::Squeezev,
            TransitionType::Zoomin,
            TransitionType::Fadefast,
            TransitionType::Fadeslow,
            TransitionType::Hlwind,
            TransitionType::Hrwind,
            TransitionType::Vuwind,
            TransitionType::Vdwind,
            TransitionType::Coverleft,
            TransitionType::Coverright,
            TransitionType::Coverup,
            TransitionType::Coverdown,
            TransitionType::Revealleft,
            TransitionType::Revealright,
            TransitionType::Revealup,
            TransitionType::Revealdown,
        ];
        assert_eq!(actual.len(), expected.len());
        for (variant, name) in actual.iter().zip(expected.iter()) {
            assert_eq!(variant.xfade_name(), *name);
        }
    }
}
