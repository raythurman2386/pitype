//! Pure WPM/accuracy math and the per-second series used for live tracking
//! and the results sparkline. No GPUI imports.

use std::time::Duration;

/// Net WPM: correct characters, 5 per word, per minute.
pub fn net_wpm(correct_chars: usize, elapsed: Duration) -> f32 {
    let minutes = elapsed.as_secs_f32() / 60.0;
    if minutes <= 0.0 {
        0.0
    } else {
        (correct_chars as f32 / 5.0) / minutes
    }
}

/// Raw WPM: every keystroke counted (correct + incorrect), 5 per word.
pub fn raw_wpm(keystrokes: usize, elapsed: Duration) -> f32 {
    net_wpm(keystrokes, elapsed)
}

/// Accuracy as a fraction 0..=1. Zero attempts counts as 1.0 so a fresh
/// engine never reads as 0%.
pub fn accuracy(correct: usize, errors: usize) -> f32 {
    let total = correct + errors;
    if total == 0 {
        1.0
    } else {
        correct as f32 / total as f32
    }
}

/// Consistency 0..=1 derived from the per-second WPM series: the closer the
/// per-second speeds cluster, the higher the score. Uses the mean absolute
/// deviation relative to the mean, inverted.
pub fn consistency(series: &[f32]) -> f32 {
    if series.len() < 2 {
        return 1.0;
    }
    let mean = series.iter().sum::<f32>() / series.len() as f32;
    if mean <= 0.0 {
        return 0.0;
    }
    let mad = series.iter().map(|v| (v - mean).abs()).sum::<f32>() / series.len() as f32;
    (1.0 - mad / mean).clamp(0.0, 1.0)
}

/// Cumulative keystroke counts sampled once per second of a run.
#[derive(Debug, Clone, Default)]
pub struct WpmSeries {
    /// (second_index, correct_chars, keystrokes) at each completed second.
    samples: Vec<(usize, usize, usize)>,
}

impl WpmSeries {
    /// Record a completed second. `second_index` should count up from 1.
    /// Later samples for an earlier second replace nothing; ordering is the
    /// caller's responsibility, but sorting is harmless here.
    pub fn push(&mut self, second_index: usize, correct: usize, keystrokes: usize) {
        self.samples.push((second_index, correct, keystrokes));
        self.samples.sort_by_key(|(i, _, _)| *i);
    }

    pub fn len(&self) -> usize {
        self.samples.len()
    }

    pub fn is_empty(&self) -> bool {
        self.samples.is_empty()
    }

    /// Net WPM measured over the last `window` completed seconds ending at
    /// `now_second`, using only samples within that window. This is the live
    /// readout: recent speed, not the whole-run average.
    pub fn rolling_wpm(&self, now_second: usize, window: usize) -> f32 {
        let start = now_second.saturating_sub(window);
        let older = self
            .samples
            .iter()
            .rev()
            .find(|(i, _, _)| *i <= start)
            .map(|(_, c, _)| *c);
        let latest = match self.samples.last() {
            Some((_, c, _)) => *c,
            None => return 0.0,
        };
        let span = match self.samples.last() {
            Some((i, _, _)) => (*i).saturating_sub(start).max(1),
            None => return 0.0,
        };
        let base = older.unwrap_or(0);
        net_wpm(
            latest.saturating_sub(base),
            Duration::from_secs(span as u64),
        )
    }

    /// Per-second net WPM points for the sparkline (whole-run view).
    pub fn points(&self) -> Vec<f32> {
        let mut out = Vec::with_capacity(self.samples.len());
        let mut prev_second = 0usize;
        let mut prev_correct = 0usize;
        for (second, correct, _) in &self.samples {
            let span = (*second - prev_second).max(1);
            out.push(net_wpm(
                correct.saturating_sub(prev_correct),
                Duration::from_secs(span as u64),
            ));
            prev_second = *second;
            prev_correct = *correct;
        }
        out
    }

    /// Per-second raw WPM points (all keystrokes, for the results detail).
    pub fn raw_points(&self) -> Vec<f32> {
        let mut out = Vec::with_capacity(self.samples.len());
        let mut prev_second = 0usize;
        let mut prev_keystrokes = 0usize;
        for (second, _, keystrokes) in &self.samples {
            let span = (*second - prev_second).max(1);
            out.push(net_wpm(
                keystrokes.saturating_sub(prev_keystrokes),
                Duration::from_secs(span as u64),
            ));
            prev_second = *second;
            prev_keystrokes = *keystrokes;
        }
        out
    }

    /// The most recent completed second index recorded.
    pub fn last_second(&self) -> usize {
        self.samples.last().map(|(i, _, _)| *i).unwrap_or(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn net_wpm_at_300_chars_per_minute() {
        // 25 correct chars in 5 s = 300 chars/min = 60 wpm.
        let wpm = net_wpm(25, Duration::from_secs(5));
        assert!((wpm - 60.0).abs() < 1e-4);
    }

    #[test]
    fn zero_elapsed_is_zero() {
        assert_eq!(net_wpm(100, Duration::from_secs(0)), 0.0);
    }

    #[test]
    fn accuracy_bounds() {
        assert_eq!(accuracy(0, 0), 1.0);
        assert!((accuracy(8, 2) - 0.8).abs() < 1e-6);
        assert_eq!(accuracy(0, 5), 0.0);
    }

    #[test]
    fn consistency_high_for_flat_series() {
        assert!(consistency(&[60.0; 10]) > 0.99);
        assert!(consistency(&[]) == 1.0);
        assert!(consistency(&[60.0]) == 1.0);
    }

    #[test]
    fn consistency_low_for_spiky_series() {
        let flat = consistency(&[60.0; 10]);
        let spiky = consistency(&[5.0, 120.0, 5.0, 120.0, 5.0, 120.0, 5.0, 120.0]);
        assert!(spiky < 0.5);
        assert!(flat > spiky);
    }

    #[test]
    fn series_points_accumulate_per_second() {
        let mut s = WpmSeries::default();
        s.push(1, 5, 5);
        s.push(2, 10, 10);
        s.push(3, 15, 10);
        let pts = s.points();
        assert_eq!(pts.len(), 3);
        // 5 chars/s = 60 wpm in each of the first two seconds.
        assert!((pts[0] - 60.0).abs() < 1e-4);
        assert!((pts[1] - 60.0).abs() < 1e-4);
        // Third second: 5 correct chars = 60 wpm net; no new keystrokes
        // (the two errors happened in earlier seconds) = 0 raw.
        assert!((pts[2] - 60.0).abs() < 1e-4);
        assert!((s.raw_points()[2] - 0.0).abs() < 1e-4);
    }

    #[test]
    fn rolling_wpm_uses_window_tail() {
        let mut s = WpmSeries::default();
        // Slow start: 5 chars in the first 10 s.
        s.push(5, 1, 1);
        s.push(10, 5, 5);
        // Then fast: 30 more chars in the next 10 s.
        s.push(15, 20, 20);
        s.push(20, 35, 35);
        // Rolling 10 s ending at second 20: 30 chars over 10 s = 36 wpm.
        let wpm = s.rolling_wpm(20, 10);
        assert!((wpm - 36.0).abs() < 1e-3);
        // Whole-run average is lower (35 chars / 20 s = 21 wpm).
        assert!(s.rolling_wpm(20, 20) < 30.0);
        // An index before any sample still measures the whole run; zero
        // only when there are no samples at all.
        assert!(s.rolling_wpm(0, 5) >= 0.0);
        assert_eq!(WpmSeries::default().rolling_wpm(5, 5), 0.0);
    }
}
