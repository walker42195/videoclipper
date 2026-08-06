//! Parses ffmpeg's `-progress pipe:1` output: repeating blocks of `key=value`
//! lines terminated by a `progress=continue` or `progress=end` line.

use serde::Serialize;
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportProgress {
    pub pct: f64,
    pub out_time_sec: f64,
    pub speed: Option<f64>,
    pub done: bool,
}

/// Accumulates `-progress` lines and yields an [`ExportProgress`] each time a
/// `progress=` terminator line completes a block.
#[derive(Default)]
pub struct ProgressParser {
    current: HashMap<String, String>,
    total_duration_sec: f64,
}

impl ProgressParser {
    pub fn new(total_duration_sec: f64) -> Self {
        Self {
            current: HashMap::new(),
            total_duration_sec,
        }
    }

    /// Feed one line of stdout. Returns `Some(progress)` when a block just
    /// completed (i.e. this line was `progress=continue` or `progress=end`).
    pub fn feed_line(&mut self, line: &str) -> Option<ExportProgress> {
        let line = line.trim();
        let (key, value) = line.split_once('=')?;
        if key == "progress" {
            let done = value == "end";
            let out_time_sec = self
                .current
                .get("out_time_us")
                .and_then(|v| v.parse::<f64>().ok())
                .map(|us| us / 1_000_000.0)
                .unwrap_or(0.0);
            let speed = self
                .current
                .get("speed")
                .and_then(|v| v.trim_end_matches('x').parse::<f64>().ok());
            let pct = if self.total_duration_sec > 0.0 {
                (out_time_sec / self.total_duration_sec * 100.0).clamp(0.0, 100.0)
            } else {
                0.0
            };
            let progress = ExportProgress {
                pct: if done { 100.0 } else { pct },
                out_time_sec,
                speed,
                done,
            };
            self.current.clear();
            return Some(progress);
        }
        self.current.insert(key.to_string(), value.to_string());
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_a_full_block() {
        let mut p = ProgressParser::new(10.0);
        for line in [
            "frame=120",
            "fps=30.0",
            "out_time_us=5000000",
            "speed=2.5x",
            "progress=continue",
        ] {
            let result = p.feed_line(line);
            if line.starts_with("progress=") {
                let prog = result.expect("should yield progress on terminator line");
                assert!((prog.pct - 50.0).abs() < 1e-9);
                assert!((prog.out_time_sec - 5.0).abs() < 1e-9);
                assert_eq!(prog.speed, Some(2.5));
                assert!(!prog.done);
            } else {
                assert!(result.is_none());
            }
        }
    }

    #[test]
    fn end_marker_reports_done_and_full_pct() {
        let mut p = ProgressParser::new(10.0);
        p.feed_line("out_time_us=9999999");
        let prog = p.feed_line("progress=end").unwrap();
        assert!(prog.done);
        assert!((prog.pct - 100.0).abs() < 1e-9);
    }

    #[test]
    fn ignores_malformed_lines() {
        let mut p = ProgressParser::new(10.0);
        assert!(p.feed_line("not a key value line").is_none());
        assert!(p.feed_line("").is_none());
    }
}
