//! Pure, ffmpeg-process-free logic for turning a timeline (clip durations +
//! transition durations) into the numbers needed to build an ffmpeg
//! `filter_complex` graph. Kept dependency-free so it can be unit tested
//! without spawning any subprocess.

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

/// One normalized input clip going into a graph.
#[derive(Debug, Clone)]
pub struct GraphClip {
    pub input_index: usize,
    pub trim_in_sec: f64,
    pub trim_out_sec: f64,
    /// If Some, this clip's audio comes from a *different* input index
    /// (a replacement audio file) instead of `input_index`'s own audio.
    pub audio_input_index: Option<usize>,
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
    let src = clip.audio_input_index.unwrap_or(clip.input_index);
    if clip.audio_input_index.is_none() {
        // Original audio: trim to match the video's trim window.
        format!(
            "[{i}:a]atrim=start={in_}:end={out},asetpts=PTS-STARTPTS,\
             aformat=sample_rates={sr}:channel_layouts=stereo[{label}];\n",
            i = src,
            in_ = clip.trim_in_sec,
            out = clip.trim_out_sec,
            sr = target.sample_rate,
            label = label,
        )
    } else {
        // Replacement audio: caller is expected to have already trimmed/
        // looped it to the clip's duration upstream; just reformat here.
        format!(
            "[{i}:a]aformat=sample_rates={sr}:channel_layouts=stereo[{label}];\n",
            i = src,
            sr = target.sample_rate,
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

    let mut graph = String::new();
    let mut v_labels = Vec::with_capacity(clips.len());
    let mut a_labels = Vec::with_capacity(clips.len());

    for (idx, clip) in clips.iter().enumerate() {
        let vlabel = format!("v{idx}");
        graph.push_str(&normalize_video_chain(clip, target, &vlabel));
        v_labels.push(vlabel);

        if clip.has_audio || clip.audio_input_index.is_some() {
            let alabel = format!("a{idx}");
            graph.push_str(&normalize_audio_chain(clip, target, &alabel));
            a_labels.push(alabel);
        }
    }

    let has_audio = a_labels.len() == clips.len();

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
            audio_input_index: None,
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
            GraphClip { input_index: 0, trim_in_sec: 0.0, trim_out_sec: 5.0, audio_input_index: None, has_audio: true },
            GraphClip { input_index: 1, trim_in_sec: 1.0, trim_out_sec: 4.0, audio_input_index: None, has_audio: true },
        ];
        let target = NormalizeTarget { width: 1280, height: 720, fps: 24, sample_rate: 44100 };
        let (graph, maps) = build_concat_graph(&clips, target);
        assert!(graph.contains("[v0][a0][v1][a1]concat=n=2:v=1:a=1[vout][aout]"));
        assert_eq!(maps, vec!["-map", "[vout]", "-map", "[aout]"]);
    }

    #[test]
    fn concat_graph_no_audio_omits_audio_map() {
        let clips = vec![
            GraphClip { input_index: 0, trim_in_sec: 0.0, trim_out_sec: 5.0, audio_input_index: None, has_audio: false },
            GraphClip { input_index: 1, trim_in_sec: 0.0, trim_out_sec: 3.0, audio_input_index: None, has_audio: false },
        ];
        let target = NormalizeTarget { width: 1920, height: 1080, fps: 30, sample_rate: 48000 };
        let (graph, maps) = build_concat_graph(&clips, target);
        assert!(graph.contains("concat=n=2:v=1:a=0[vout]"));
        assert_eq!(maps, vec!["-map", "[vout]"]);
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
