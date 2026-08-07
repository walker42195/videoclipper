//! Pure, ffmpeg-process-free logic for turning a timeline (clip durations +
//! transition durations) into the numbers needed to build an ffmpeg
//! `filter_complex` graph. Kept dependency-free so it can be unit tested
//! without spawning any subprocess.

use crate::model::{AudioBlendMode, AudioMode, TransitionType};

/// Junctions with no user-chosen transition are still built through the
/// xfade/acrossfade chain (rather than falling back to `concat`) so the
/// graph-building code has one uniform path; a duration this short (~1
/// frame at 24fps) is visually indistinguishable from a hard cut.
pub const CUT_DURATION_SEC: f64 = 0.04;

/// One junction between two adjacent clips.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct JunctionInput {
    pub transition_duration_sec: f64,
}

/// Computed placement for a single xfade/acrossfade junction.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct JunctionPlacement {
    pub offset_sec: f64,
    pub duration_sec: f64,
}

/// For each interior clip (touched by two transitions), the incoming and
/// outgoing transition durations must not together exceed the clip's own
/// duration, otherwise the two transitions eat into each other. This clamps
/// both proportionally so `incoming + outgoing <= clip_duration` holds.
///
/// `clip_durations.len()` must equal `transitions.len() + 1`.
pub fn clamp_transitions(
    clip_durations: &[f64],
    transitions: &[JunctionInput],
) -> Vec<JunctionInput> {
    assert_eq!(
        clip_durations.len(),
        transitions.len() + 1,
        "there must be exactly one fewer transition than clips"
    );

    let mut out: Vec<f64> = transitions.iter().map(|t| t.transition_duration_sec).collect();

    // Interior clips are indices 1..len-1; clip i is bordered by
    // transitions[i-1] (incoming) and transitions[i] (outgoing).
    for i in 1..clip_durations.len().saturating_sub(1) {
        let incoming = out[i - 1];
        let outgoing = out[i];
        let budget = clip_durations[i];
        let total = incoming + outgoing;
        if total > budget && total > 0.0 {
            let scale = budget / total;
            out[i - 1] = incoming * scale;
            out[i] = outgoing * scale;
        }
    }

    out.into_iter()
        .map(|d| JunctionInput { transition_duration_sec: d })
        .collect()
}

/// Compute the `offset`/`duration` for every xfade/acrossfade junction, plus
/// the total merged duration of the whole timeline. Transitions should
/// already be clamped via [`clamp_transitions`].
///
/// merged_duration[0] = d_0
/// offset[i]          = merged_duration[i-1] - t_{i-1}
/// merged_duration[i] = merged_duration[i-1] + d_i - t_{i-1}
pub fn compute_junctions(
    clip_durations: &[f64],
    transitions: &[JunctionInput],
) -> (Vec<JunctionPlacement>, f64) {
    assert_eq!(clip_durations.len(), transitions.len() + 1);

    let mut merged_duration = clip_durations[0];
    let mut placements = Vec::with_capacity(transitions.len());

    for (i, t) in transitions.iter().enumerate() {
        let offset = merged_duration - t.transition_duration_sec;
        placements.push(JunctionPlacement {
            offset_sec: offset,
            duration_sec: t.transition_duration_sec,
        });
        merged_duration = merged_duration + clip_durations[i + 1] - t.transition_duration_sec;
    }

    (placements, merged_duration)
}

/// A per-clip audio replacement: a different ffmpeg input, looped or padded
/// to the clip's own on-timeline duration and gain-adjusted, standing in for
/// the clip's original audio track.
#[derive(Debug, Clone, Copy)]
pub struct AudioReplacement {
    pub input_index: usize,
    pub fill_mode: AudioMode,
    pub gain_db: f64,
}

/// One normalized input clip going into a graph.
#[derive(Debug, Clone)]
pub struct GraphClip {
    pub input_index: usize,
    pub trim_in_sec: f64,
    pub trim_out_sec: f64,
    /// If Some, this clip's audio comes from a replacement file instead of
    /// its own. Takes priority over `has_audio`.
    pub audio_replacement: Option<AudioReplacement>,
    pub has_audio: bool,
}

#[derive(Debug, Clone, Copy)]
pub struct NormalizeTarget {
    pub width: u32,
    pub height: u32,
    pub fps: u32,
    pub sample_rate: u32,
}

fn normalize_video_chain(clip: &GraphClip, target: NormalizeTarget, label: &str) -> String {
    format!(
        "[{i}:v]trim=start={in_}:end={out},setpts=PTS-STARTPTS,\
         scale={w}:{h}:force_original_aspect_ratio=decrease,\
         pad={w}:{h}:(ow-iw)/2:(oh-ih)/2,setsar=1,fps={fps},format=yuv420p,settb=AVTB[{label}];\n",
        i = clip.input_index,
        in_ = clip.trim_in_sec,
        out = clip.trim_out_sec,
        w = target.width,
        h = target.height,
        fps = target.fps,
        label = label,
    )
}

fn normalize_audio_chain(clip: &GraphClip, target: NormalizeTarget, label: &str) -> String {
    let clip_duration = clip.trim_out_sec - clip.trim_in_sec;

    if let Some(repl) = &clip.audio_replacement {
        // Replacement audio: loop or pad it to the clip's own on-timeline
        // duration and apply its gain right here, so downstream code (concat/
        // xfade chains) never needs to know a replacement happened.
        let fill = match repl.fill_mode {
            AudioMode::Loop => "aloop=loop=-1:size=2147483647,",
            AudioMode::Pad => "apad,",
        };
        format!(
            "[{i}:a]aformat=sample_rates={sr}:channel_layouts=stereo,{fill}atrim=0:{dur},\
             asetpts=PTS-STARTPTS,volume={gain}dB[{label}];\n",
            i = repl.input_index,
            sr = target.sample_rate,
            fill = fill,
            dur = clip_duration,
            gain = repl.gain_db,
            label = label,
        )
    } else if clip.has_audio {
        // Original audio: trim to match the video's trim window.
        format!(
            "[{i}:a]atrim=start={in_}:end={out},asetpts=PTS-STARTPTS,\
             aformat=sample_rates={sr}:channel_layouts=stereo[{label}];\n",
            i = clip.input_index,
            in_ = clip.trim_in_sec,
            out = clip.trim_out_sec,
            sr = target.sample_rate,
            label = label,
        )
    } else {
        // No source audio and no replacement: synthesize silence for this
        // clip's duration so a mixed timeline (some clips with audio, some
        // without) still concatenates/crossfades into one uniform audio
        // stream instead of losing audio entirely.
        format!(
            "anullsrc=channel_layout=stereo:sample_rate={sr},atrim=0:{dur},\
             asetpts=PTS-STARTPTS[{label}];\n",
            sr = target.sample_rate,
            dur = clip_duration,
            label = label,
        )
    }
}

/// Build a `-filter_complex` graph (+ trailing `-map` args) that normalizes
/// every clip to a common resolution/fps/sample-rate and concatenates them
/// with hard cuts (no transitions). This is the M3 code path; the M5
/// transition-chaining path reuses the same per-clip normalize chains but
/// links them with `xfade`/`acrossfade` instead of `concat`.
pub fn build_concat_graph(clips: &[GraphClip], target: NormalizeTarget) -> (String, Vec<String>) {
    assert!(!clips.is_empty(), "need at least one clip");

    let any_audio = clips.iter().any(|c| c.has_audio || c.audio_replacement.is_some());

    let mut graph = String::new();
    let mut v_labels = Vec::with_capacity(clips.len());
    let mut a_labels = Vec::with_capacity(clips.len());

    for (idx, clip) in clips.iter().enumerate() {
        let vlabel = format!("v{idx}");
        graph.push_str(&normalize_video_chain(clip, target, &vlabel));
        v_labels.push(vlabel);

        if any_audio {
            let alabel = format!("a{idx}");
            graph.push_str(&normalize_audio_chain(clip, target, &alabel));
            a_labels.push(alabel);
        }
    }

    let has_audio = any_audio;

    if clips.len() == 1 {
        graph.push_str(&format!("[{}]null[vout];\n", v_labels[0]));
        if has_audio {
            graph.push_str(&format!("[{}]anull[aout];\n", a_labels[0]));
        }
    } else {
        let v_inputs: String = v_labels.iter().map(|l| format!("[{l}]")).collect();
        if has_audio {
            let a_inputs: String = a_labels.iter().map(|l| format!("[{l}]")).collect();
            let interleaved: String = v_labels
                .iter()
                .zip(a_labels.iter())
                .map(|(v, a)| format!("[{v}][{a}]"))
                .collect();
            let _ = (v_inputs, a_inputs); // interleaved form is what concat wants
            graph.push_str(&format!(
                "{interleaved}concat=n={n}:v=1:a=1[vout][aout];\n",
                n = clips.len()
            ));
        } else {
            graph.push_str(&format!(
                "{v_inputs}concat=n={n}:v=1:a=0[vout];\n",
                n = clips.len()
            ));
        }
    }

    let mut maps = vec!["-map".to_string(), "[vout]".to_string()];
    if has_audio {
        maps.push("-map".to_string());
        maps.push("[aout]".to_string());
    }

    (graph, maps)
}

/// One junction's transition choice, going into [`build_transition_graph`].
#[derive(Debug, Clone, Copy)]
pub struct TransitionJunction {
    pub transition_type: TransitionType,
    pub duration_sec: f64,
}

/// Build a `-filter_complex` graph (+ trailing `-map` args + the resulting
/// total output duration in seconds) that normalizes every clip like
/// [`build_concat_graph`] does, but links consecutive clips with
/// `xfade`/`acrossfade` instead of `concat`. `transitions.len()` must equal
/// `clips.len() - 1` (use [`CUT_DURATION_SEC`] for junctions the user didn't
/// put an explicit transition on). The returned duration is shorter than the
/// sum of clip durations by however much the transitions overlap - callers
/// computing export progress must use it instead of summing clip durations.
pub fn build_transition_graph(
    clips: &[GraphClip],
    target: NormalizeTarget,
    transitions: &[TransitionJunction],
) -> (String, Vec<String>, f64) {
    assert!(!clips.is_empty(), "need at least one clip");
    assert_eq!(
        transitions.len(),
        clips.len().saturating_sub(1),
        "need exactly one fewer transition than clips"
    );

    if clips.len() == 1 {
        let (graph, maps) = build_concat_graph(clips, target);
        let duration = clips[0].trim_out_sec - clips[0].trim_in_sec;
        return (graph, maps, duration);
    }

    let clip_durations: Vec<f64> = clips.iter().map(|c| c.trim_out_sec - c.trim_in_sec).collect();
    let junction_inputs: Vec<JunctionInput> = transitions
        .iter()
        .map(|t| JunctionInput { transition_duration_sec: t.duration_sec })
        .collect();
    let clamped = clamp_transitions(&clip_durations, &junction_inputs);
    let (placements, total_duration_sec) = compute_junctions(&clip_durations, &clamped);

    let any_audio = clips.iter().any(|c| c.has_audio || c.audio_replacement.is_some());

    let mut graph = String::new();
    let mut v_labels = Vec::with_capacity(clips.len());
    let mut a_labels = Vec::with_capacity(clips.len());

    for (idx, clip) in clips.iter().enumerate() {
        let vlabel = format!("v{idx}");
        graph.push_str(&normalize_video_chain(clip, target, &vlabel));
        v_labels.push(vlabel);

        if any_audio {
            let alabel = format!("a{idx}");
            graph.push_str(&normalize_audio_chain(clip, target, &alabel));
            a_labels.push(alabel);
        }
    }
    let has_audio = any_audio;

    let mut prev_v = v_labels[0].clone();
    let mut prev_a = has_audio.then(|| a_labels[0].clone());

    for (i, (junction, placement)) in transitions.iter().zip(placements.iter()).enumerate() {
        let next_v = format!("vx{i}");
        graph.push_str(&format!(
            "[{prev}][{cur}]xfade=transition={ttype}:duration={dur}:offset={off}[{out}];\n",
            prev = prev_v,
            cur = v_labels[i + 1],
            ttype = junction.transition_type.xfade_name(),
            dur = placement.duration_sec,
            off = placement.offset_sec,
            out = next_v,
        ));
        prev_v = next_v;

        if has_audio {
            let next_a = format!("ax{i}");
            graph.push_str(&format!(
                "[{prev}][{cur}]acrossfade=d={dur}:c1=tri:c2=tri[{out}];\n",
                prev = prev_a.as_ref().unwrap(),
                cur = a_labels[i + 1],
                dur = placement.duration_sec,
                out = next_a,
            ));
            prev_a = Some(next_a);
        }
    }

    graph.push_str(&format!("[{prev_v}]null[vout];\n"));
    let mut maps = vec!["-map".to_string(), "[vout]".to_string()];
    if let Some(a) = prev_a {
        graph.push_str(&format!("[{a}]anull[aout];\n"));
        maps.push("-map".to_string());
        maps.push("[aout]".to_string());
    }

    (graph, maps, total_duration_sec)
}

/// A whole-movie background-music track to layer onto (or replace) the
/// timeline's existing audio, going into [`build_background_music_chain`].
#[derive(Debug, Clone, Copy)]
pub struct BackgroundMusicSpec {
    pub input_index: usize,
    pub blend_mode: AudioBlendMode,
    pub fill_mode: AudioMode,
    pub gain_db: f64,
    pub fade_in_sec: f64,
    pub fade_out_sec: f64,
}

/// Builds the filter_complex fragment that layers a background-music input
/// onto `base_audio_label` (the `"aout"` label from [`build_transition_graph`],
/// or `None` if the timeline has no audio at all) and produces
/// `output_label`. In [`AudioBlendMode::Replace`] mode `base_audio_label` is
/// ignored entirely; in [`AudioBlendMode::Mix`] mode with no base audio,
/// mixing degrades to just using the music track (nothing to mix with).
pub fn build_background_music_chain(
    spec: &BackgroundMusicSpec,
    base_audio_label: Option<&str>,
    total_duration_sec: f64,
    sample_rate: u32,
    output_label: &str,
) -> String {
    const MUSIC_LABEL: &str = "bgmusic";

    let fill = match spec.fill_mode {
        AudioMode::Loop => "aloop=loop=-1:size=2147483647,",
        AudioMode::Pad => "apad,",
    };
    let fade_out_start = (total_duration_sec - spec.fade_out_sec).max(0.0);

    let mut graph = format!(
        "[{i}:a]aformat=sample_rates={sr}:channel_layouts=stereo,{fill}atrim=0:{dur},\
         asetpts=PTS-STARTPTS,volume={gain}dB,afade=t=in:d={fadein},\
         afade=t=out:st={fadeoutstart}:d={fadeout}[{label}];\n",
        i = spec.input_index,
        sr = sample_rate,
        fill = fill,
        dur = total_duration_sec,
        gain = spec.gain_db,
        fadein = spec.fade_in_sec,
        fadeoutstart = fade_out_start,
        fadeout = spec.fade_out_sec,
        label = MUSIC_LABEL,
    );

    match (spec.blend_mode, base_audio_label) {
        (AudioBlendMode::Mix, Some(base)) => {
            graph.push_str(&format!(
                "[{base}][{MUSIC_LABEL}]amix=inputs=2:duration=first:dropout_transition=0:normalize=0[{output_label}];\n"
            ));
        }
        (AudioBlendMode::Replace, _) | (AudioBlendMode::Mix, None) => {
            graph.push_str(&format!("[{MUSIC_LABEL}]anull[{output_label}];\n"));
        }
    }

    graph
}

#[cfg(test)]
mod tests {
    use super::*;

    fn approx(a: f64, b: f64) {
        assert!((a - b).abs() < 1e-9, "expected {a} ~= {b}");
    }

    #[test]
    fn single_clip_no_transitions() {
        let (placements, total) = compute_junctions(&[5.0], &[]);
        assert!(placements.is_empty());
        approx(total, 5.0);
    }

    #[test]
    fn plan_worked_example_three_clips() {
        // 5s / 4s / 6s clips, fade 1s at 0->1, dissolve 0.75s at 1->2.
        let durations = [5.0, 4.0, 6.0];
        let transitions = [
            JunctionInput { transition_duration_sec: 1.0 },
            JunctionInput { transition_duration_sec: 0.75 },
        ];
        let (placements, total) = compute_junctions(&durations, &transitions);
        approx(placements[0].offset_sec, 4.0);
        approx(placements[1].offset_sec, 7.25);
        approx(total, 13.25); // 5 + 4 - 1 + 6 - 0.75
    }

    #[test]
    fn clamp_leaves_non_conflicting_transitions_untouched() {
        let durations = [5.0, 4.0, 6.0];
        let transitions = [
            JunctionInput { transition_duration_sec: 1.0 },
            JunctionInput { transition_duration_sec: 0.75 },
        ];
        let clamped = clamp_transitions(&durations, &transitions);
        approx(clamped[0].transition_duration_sec, 1.0);
        approx(clamped[1].transition_duration_sec, 0.75);
    }

    #[test]
    fn clamp_scales_down_conflicting_transitions() {
        // Middle clip is only 1s long but wants 1s in + 1s out -> must shrink.
        let durations = [5.0, 1.0, 6.0];
        let transitions = [
            JunctionInput { transition_duration_sec: 1.0 },
            JunctionInput { transition_duration_sec: 1.0 },
        ];
        let clamped = clamp_transitions(&durations, &transitions);
        approx(clamped[0].transition_duration_sec, 0.5);
        approx(clamped[1].transition_duration_sec, 0.5);
    }

    #[test]
    fn concat_graph_single_clip_maps_vout_and_aout() {
        let clips = vec![GraphClip {
            input_index: 0,
            trim_in_sec: 0.0,
            trim_out_sec: 5.0,
            audio_replacement: None,
            has_audio: true,
        }];
        let target = NormalizeTarget { width: 1920, height: 1080, fps: 30, sample_rate: 48000 };
        let (graph, maps) = build_concat_graph(&clips, target);
        assert!(graph.contains("[v0]null[vout]"));
        assert!(graph.contains("[a0]anull[aout]"));
        assert_eq!(maps, vec!["-map", "[vout]", "-map", "[aout]"]);
    }

    #[test]
    fn concat_graph_multi_clip_uses_concat_filter() {
        let clips = vec![
            GraphClip { input_index: 0, trim_in_sec: 0.0, trim_out_sec: 5.0, audio_replacement: None, has_audio: true },
            GraphClip { input_index: 1, trim_in_sec: 1.0, trim_out_sec: 4.0, audio_replacement: None, has_audio: true },
        ];
        let target = NormalizeTarget { width: 1280, height: 720, fps: 24, sample_rate: 44100 };
        let (graph, maps) = build_concat_graph(&clips, target);
        assert!(graph.contains("[v0][a0][v1][a1]concat=n=2:v=1:a=1[vout][aout]"));
        assert_eq!(maps, vec!["-map", "[vout]", "-map", "[aout]"]);
    }

    #[test]
    fn concat_graph_no_audio_omits_audio_map() {
        let clips = vec![
            GraphClip { input_index: 0, trim_in_sec: 0.0, trim_out_sec: 5.0, audio_replacement: None, has_audio: false },
            GraphClip { input_index: 1, trim_in_sec: 0.0, trim_out_sec: 3.0, audio_replacement: None, has_audio: false },
        ];
        let target = NormalizeTarget { width: 1920, height: 1080, fps: 30, sample_rate: 48000 };
        let (graph, maps) = build_concat_graph(&clips, target);
        assert!(graph.contains("concat=n=2:v=1:a=0[vout]"));
        assert_eq!(maps, vec!["-map", "[vout]"]);
    }

    #[test]
    fn concat_graph_synthesizes_silence_for_clips_missing_audio() {
        // Mixed timeline: clip 0 has real audio, clip 1 doesn't. The audio
        // track must still be included (not dropped entirely), with clip 1
        // getting a silent stand-in of its own duration.
        let clips = vec![
            GraphClip { input_index: 0, trim_in_sec: 0.0, trim_out_sec: 5.0, audio_replacement: None, has_audio: true },
            GraphClip { input_index: 1, trim_in_sec: 0.0, trim_out_sec: 3.0, audio_replacement: None, has_audio: false },
        ];
        let target = NormalizeTarget { width: 1920, height: 1080, fps: 30, sample_rate: 48000 };
        let (graph, maps) = build_concat_graph(&clips, target);
        assert!(graph.contains("[1:a]atrim") == false); // clip 1 has no real audio input to trim
        assert!(graph.contains("anullsrc=channel_layout=stereo:sample_rate=48000,atrim=0:3,asetpts=PTS-STARTPTS[a1]"));
        assert!(graph.contains("concat=n=2:v=1:a=1[vout][aout]"));
        assert_eq!(maps, vec!["-map", "[vout]", "-map", "[aout]"]);
    }

    #[test]
    fn concat_graph_uses_replacement_audio_with_loop_fill() {
        let clips = vec![GraphClip {
            input_index: 0,
            trim_in_sec: 0.0,
            trim_out_sec: 5.0,
            audio_replacement: Some(AudioReplacement { input_index: 4, fill_mode: AudioMode::Loop, gain_db: -3.0 }),
            has_audio: true, // replacement takes priority over the clip's own audio
        }];
        let target = NormalizeTarget { width: 1920, height: 1080, fps: 30, sample_rate: 48000 };
        let (graph, _maps) = build_concat_graph(&clips, target);
        assert!(graph.contains(
            "[4:a]aformat=sample_rates=48000:channel_layouts=stereo,aloop=loop=-1:size=2147483647,atrim=0:5,asetpts=PTS-STARTPTS,volume=-3dB[a0]"
        ));
    }

    #[test]
    fn concat_graph_uses_replacement_audio_with_pad_fill() {
        let clips = vec![GraphClip {
            input_index: 0,
            trim_in_sec: 0.0,
            trim_out_sec: 5.0,
            audio_replacement: Some(AudioReplacement { input_index: 4, fill_mode: AudioMode::Pad, gain_db: 0.0 }),
            has_audio: false,
        }];
        let target = NormalizeTarget { width: 1920, height: 1080, fps: 30, sample_rate: 48000 };
        let (graph, _maps) = build_concat_graph(&clips, target);
        assert!(graph.contains(
            "[4:a]aformat=sample_rates=48000:channel_layouts=stereo,apad,atrim=0:5,asetpts=PTS-STARTPTS,volume=0dB[a0]"
        ));
    }

    #[test]
    fn transition_graph_single_clip_delegates_to_concat_graph() {
        let clips = vec![GraphClip {
            input_index: 0,
            trim_in_sec: 0.0,
            trim_out_sec: 5.0,
            audio_replacement: None,
            has_audio: true,
        }];
        let target = NormalizeTarget { width: 1920, height: 1080, fps: 30, sample_rate: 48000 };
        let (graph, maps, total) = build_transition_graph(&clips, target, &[]);
        assert!(graph.contains("[v0]null[vout]"));
        assert_eq!(maps, vec!["-map", "[vout]", "-map", "[aout]"]);
        approx(total, 5.0);
    }

    #[test]
    fn transition_graph_chains_xfade_and_acrossfade() {
        // Matches the plan's worked example: 5s/4s/6s clips, fade 1s at
        // 0->1, dissolve 0.75s at 1->2.
        let clips = vec![
            GraphClip { input_index: 0, trim_in_sec: 0.0, trim_out_sec: 5.0, audio_replacement: None, has_audio: true },
            GraphClip { input_index: 1, trim_in_sec: 0.0, trim_out_sec: 4.0, audio_replacement: None, has_audio: true },
            GraphClip { input_index: 2, trim_in_sec: 0.0, trim_out_sec: 6.0, audio_replacement: None, has_audio: true },
        ];
        let target = NormalizeTarget { width: 1920, height: 1080, fps: 30, sample_rate: 48000 };
        let transitions = [
            TransitionJunction { transition_type: TransitionType::Fade, duration_sec: 1.0 },
            TransitionJunction { transition_type: TransitionType::Dissolve, duration_sec: 0.75 },
        ];
        let (graph, maps, total) = build_transition_graph(&clips, target, &transitions);

        assert!(graph.contains("[v0][v1]xfade=transition=fade:duration=1:offset=4[vx0]"));
        assert!(graph.contains("[a0][a1]acrossfade=d=1:c1=tri:c2=tri[ax0]"));
        assert!(graph.contains("[vx0][v2]xfade=transition=dissolve:duration=0.75:offset=7.25[vx1]"));
        assert!(graph.contains("[ax0][a2]acrossfade=d=0.75:c1=tri:c2=tri[ax1]"));
        assert!(graph.contains("[vx1]null[vout]"));
        assert!(graph.contains("[ax1]anull[aout]"));
        assert_eq!(maps, vec!["-map", "[vout]", "-map", "[aout]"]);
        approx(total, 13.25);
    }

    #[test]
    fn transition_graph_no_audio_omits_acrossfade_and_audio_map() {
        let clips = vec![
            GraphClip { input_index: 0, trim_in_sec: 0.0, trim_out_sec: 5.0, audio_replacement: None, has_audio: false },
            GraphClip { input_index: 1, trim_in_sec: 0.0, trim_out_sec: 4.0, audio_replacement: None, has_audio: false },
        ];
        let target = NormalizeTarget { width: 1920, height: 1080, fps: 30, sample_rate: 48000 };
        let transitions = [TransitionJunction { transition_type: TransitionType::Wipeleft, duration_sec: 1.0 }];
        let (graph, maps, _total) = build_transition_graph(&clips, target, &transitions);

        assert!(graph.contains("xfade=transition=wipeleft"));
        assert!(!graph.contains("acrossfade"));
        assert_eq!(maps, vec!["-map", "[vout]"]);
    }

    #[test]
    #[should_panic(expected = "need exactly one fewer transition than clips")]
    fn transition_graph_rejects_mismatched_transition_count() {
        let clips = vec![
            GraphClip { input_index: 0, trim_in_sec: 0.0, trim_out_sec: 5.0, audio_replacement: None, has_audio: true },
            GraphClip { input_index: 1, trim_in_sec: 0.0, trim_out_sec: 4.0, audio_replacement: None, has_audio: true },
        ];
        let target = NormalizeTarget { width: 1920, height: 1080, fps: 30, sample_rate: 48000 };
        let _ = build_transition_graph(&clips, target, &[]);
    }

    #[test]
    fn background_music_mix_mode_mixes_with_base_audio() {
        let spec = BackgroundMusicSpec {
            input_index: 3,
            blend_mode: AudioBlendMode::Mix,
            fill_mode: AudioMode::Loop,
            gain_db: -18.0,
            fade_in_sec: 1.5,
            fade_out_sec: 2.0,
        };
        let graph = build_background_music_chain(&spec, Some("aout"), 20.0, 48000, "finalaudio");
        assert!(graph.contains("[3:a]aformat=sample_rates=48000:channel_layouts=stereo,aloop=loop=-1:size=2147483647,atrim=0:20"));
        assert!(graph.contains("volume=-18dB"));
        assert!(graph.contains("afade=t=in:d=1.5"));
        assert!(graph.contains("afade=t=out:st=18:d=2"));
        assert!(graph.contains("[aout][bgmusic]amix=inputs=2:duration=first:dropout_transition=0:normalize=0[finalaudio]"));
    }

    #[test]
    fn background_music_replace_mode_ignores_base_audio() {
        let spec = BackgroundMusicSpec {
            input_index: 2,
            blend_mode: AudioBlendMode::Replace,
            fill_mode: AudioMode::Pad,
            gain_db: 0.0,
            fade_in_sec: 0.0,
            fade_out_sec: 0.0,
        };
        let graph = build_background_music_chain(&spec, Some("aout"), 10.0, 48000, "finalaudio");
        assert!(!graph.contains("amix"));
        assert!(!graph.contains("[aout]"));
        assert!(graph.contains("[bgmusic]anull[finalaudio]"));
    }

    #[test]
    fn background_music_mix_mode_with_no_base_audio_falls_back_to_music_only() {
        let spec = BackgroundMusicSpec {
            input_index: 1,
            blend_mode: AudioBlendMode::Mix,
            fill_mode: AudioMode::Loop,
            gain_db: -6.0,
            fade_in_sec: 0.5,
            fade_out_sec: 0.5,
        };
        let graph = build_background_music_chain(&spec, None, 8.0, 48000, "finalaudio");
        assert!(!graph.contains("amix"));
        assert!(graph.contains("[bgmusic]anull[finalaudio]"));
    }

    #[test]
    fn clamp_handles_two_clips_one_transition() {
        let durations = [5.0, 4.0];
        let transitions = [JunctionInput { transition_duration_sec: 10.0 }];
        // No interior clips (len-1 loop range is empty for 2 clips), so this
        // particular pathological case (transition longer than either clip)
        // is intentionally NOT clamped here - it's a boundary case validated
        // elsewhere (UI should cap duration pickers to min(d0, d1) directly).
        let clamped = clamp_transitions(&durations, &transitions);
        approx(clamped[0].transition_duration_sec, 10.0);
    }
}
