//! The typing engine: one funnel for every keystroke, pure and
//! wall-clock-free so tests can drive it deterministically.

use std::time::{Duration, Instant};

use crate::lessons::PromptSpec;
use crate::metrics::{accuracy, consistency, net_wpm, raw_wpm, WpmSeries};

/// A printable keystroke reduced to what the engine needs. Built in the UI
/// layer from GPUI's `Keystroke`; kept clonable so tests can drive the
/// engine exactly as the app does.
#[derive(Debug, Clone, PartialEq)]
pub enum KeyInput {
    Char(char),
    Backspace,
    /// Printable keys the prompt cannot match (e.g. tab while typing).
    Ignored,
}

/// A mistyped keystroke at a prompt position, for the red error flash.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ErrorFlash {
    pub position: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Mode {
    #[default]
    Timed,
    Words,
    Quote,
}

impl Mode {
    pub fn label(&self) -> &'static str {
        match self {
            Mode::Timed => "Time",
            Mode::Words => "Words",
            Mode::Quote => "Quote",
        }
    }

    pub fn next(self) -> Self {
        match self {
            Mode::Timed => Mode::Words,
            Mode::Words => Mode::Quote,
            Mode::Quote => Mode::Timed,
        }
    }
}

/// The typing state machine for one run.
#[derive(Debug, Clone)]
pub struct Engine {
    prompt: String,
    /// Char (not byte) offset of the next expected prompt character. Also
    /// the count of currently matched characters (we only advance on match).
    cursor: usize,
    /// Count of keystrokes that did not match (not fixups).
    errors: usize,
    /// Total printable keystrokes, for raw WPM.
    keystrokes: usize,
    started: Option<Instant>,
    /// `char, typed_at` for each correctly typed char (timestamped for series).
    timeline: Vec<(char, Instant)>,
    series: WpmSeries,
    finished: bool,
    /// Last error position with its timestamp, cleared after 350 ms.
    flash: Option<(ErrorFlash, Instant)>,
}

const FLASH_TTL: Duration = Duration::from_millis(350);
/// Rolling window (seconds) for the live WPM readout.
const ROLLING_WINDOW: usize = 5;

impl Engine {
    pub fn new(prompt: String) -> Self {
        Self {
            prompt,
            cursor: 0,
            errors: 0,
            keystrokes: 0,
            started: None,
            timeline: Vec::new(),
            series: WpmSeries::default(),
            finished: false,
            flash: None,
        }
    }

    pub fn prompt(&self) -> &str {
        &self.prompt
    }

    /// Char offset of the next expected prompt character.
    pub fn cursor(&self) -> usize {
        self.cursor
    }

    /// Characters currently matched (the typed prefix without errors).
    pub fn correct(&self) -> usize {
        self.cursor
    }

    pub fn errors(&self) -> usize {
        self.errors
    }

    pub fn is_started(&self) -> bool {
        self.started.is_some()
    }

    pub fn is_finished(&self) -> bool {
        self.finished
    }

    /// Position and age of the current error flash, if still fresh.
    pub fn flash(&self, now: Instant) -> Option<usize> {
        let (flash, at) = self.flash?;
        if now.duration_since(at) > FLASH_TTL {
            None
        } else {
            Some(flash.position)
        }
    }

    /// Process one keystroke. `now` is injected so the engine stays pure.
    pub fn type_key(&mut self, key: KeyInput, now: Instant) {
        if self.finished {
            return;
        }
        match key {
            KeyInput::Backspace => {
                if self.cursor > 0 {
                    self.cursor -= 1;
                    // Drop the timestamp of the char being corrected.
                    self.timeline.truncate(self.timeline.len().min(self.cursor));
                }
            }
            KeyInput::Char(c) => {
                if self.started.is_none() {
                    self.started = Some(now);
                }
                self.keystrokes += 1;
                let expected = self.prompt[self.cursor_byte()..].chars().next();
                if expected == Some(c) {
                    self.cursor += 1;
                    self.timeline.push((c, now));
                    self.flash = None;
                } else {
                    self.errors += 1;
                    self.flash = Some((
                        ErrorFlash {
                            position: self.cursor,
                        },
                        now,
                    ));
                }
            }
            KeyInput::Ignored => {}
        }
    }

    pub fn elapsed(&self, now: Instant) -> Duration {
        self.started
            .map_or(Duration::ZERO, |start| now.duration_since(start))
    }

    /// Whole-run net WPM right now.
    pub fn live_net_wpm(&self, now: Instant) -> f32 {
        net_wpm(self.correct(), self.elapsed(now))
    }

    /// Rolling recent WPM for the live readout (last few seconds).
    pub fn rolling_wpm(&self) -> f32 {
        self.series
            .rolling_wpm(self.series.last_second(), ROLLING_WINDOW)
    }

    /// Words completed. A word counts once a space follows it; a typed word
    /// still waiting for its trailing space counts too (Words-mode runs end
    /// on the last word's last character, not on a space).
    pub fn words_done(&self) -> usize {
        let cursor = self.cursor_byte();
        let spaces = self.prompt[..cursor].chars().filter(|c| *c == ' ').count();
        let trailing = self.prompt[..cursor]
            .chars()
            .rev()
            .take_while(|c| *c != ' ')
            .count();
        spaces + usize::from(trailing > 0)
    }

    fn cursor_byte(&self) -> usize {
        self.prompt
            .char_indices()
            .nth(self.cursor)
            .map(|(i, _)| i)
            .unwrap_or(self.prompt.len())
    }

    /// Called once per second by the UI tick to keep the series moving.
    pub fn tick_second(&mut self, now: Instant) {
        let Some(start) = self.started else {
            return;
        };
        let elapsed = now.duration_since(start).as_secs();
        let last = self.series.last_second();
        for second in (last + 1)..=(elapsed as usize) {
            let at = start + Duration::from_secs(second as u64);
            let correct = self.timeline.iter().take_while(|(_, t)| *t <= at).count();
            self.series.push(second, correct, correct + self.errors);
        }
    }

    /// Whole-run net WPM series points (sparkline).
    pub fn series_points(&self) -> Vec<f32> {
        self.series.points()
    }

    /// Consistency over the per-second series.
    pub fn consistency(&self) -> f32 {
        consistency(&self.series.points())
    }

    /// Completion check: timed runs end on elapsed time, word-count runs end
    /// when the required words are correct, quote runs end at the last char.
    pub fn check_finished(&mut self, spec: &PromptSpec, now: Instant) -> bool {
        if self.finished {
            return true;
        }
        let done = match spec {
            PromptSpec::Time(d) => self.started.is_some() && self.elapsed(now) >= *d,
            PromptSpec::Words(n) => self.words_done() >= *n,
            PromptSpec::Quote(_) => self.cursor >= self.prompt.chars().count(),
        };
        self.finished = done;
        done
    }

    /// Final stats for the run.
    pub fn finish(&self, spec: &PromptSpec, now: Instant) -> SessionResult {
        let elapsed = self.elapsed(now);
        SessionResult {
            net_wpm: net_wpm(self.correct(), elapsed),
            raw_wpm: raw_wpm(self.keystrokes, elapsed),
            accuracy: accuracy(self.correct(), self.errors),
            errors: self.errors,
            consistency: self.consistency(),
            duration_s: elapsed.as_secs_f32(),
            mode_tag: spec.mode_tag(),
            prompt_chars: self.prompt.chars().count(),
        }
    }
}

/// Final numbers for a completed run (pre-persistence shape).
#[derive(Debug, Clone, PartialEq)]
pub struct SessionResult {
    pub net_wpm: f32,
    pub raw_wpm: f32,
    pub accuracy: f32,
    pub errors: usize,
    pub consistency: f32,
    pub duration_s: f32,
    pub mode_tag: String,
    pub prompt_chars: usize,
}

/// Classify a GPUI keystroke's printable payload for the engine.
/// `key` is `Keystroke.key`, `key_char` is `Keystroke.key_char`.
pub fn classify_key(key: &str, key_char: Option<&str>) -> KeyInput {
    if key == "space" {
        return KeyInput::Char(' ');
    }
    match key_char {
        Some(s) if s.chars().count() == 1 && !s.chars().next().unwrap().is_control() => {
            KeyInput::Char(s.chars().next().unwrap())
        }
        _ => match key {
            "backspace" => KeyInput::Backspace,
            "shift" | "ctrl" | "alt" | "super" | "function" | "capslock" | "tab" | "enter"
            | "escape" | "up" | "down" | "left" | "right" | "home" | "end" | "pageup"
            | "pagedown" | "delete" | "insert" => KeyInput::Ignored,
            _ => KeyInput::Ignored,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn t(sec: u64) -> Instant {
        // The engine never reads the wall clock; any base instant works.
        Instant::now() + Duration::from_secs(sec * 1000)
    }

    fn typed(engine: &mut Engine, c: char, now: Instant) {
        engine.type_key(KeyInput::Char(c), now);
    }

    #[test]
    fn correct_typing_advances_cursor() {
        let mut e = Engine::new("abc".into());
        let now = t(0);
        typed(&mut e, 'a', now);
        typed(&mut e, 'b', now + Duration::from_secs(1));
        typed(&mut e, 'c', now + Duration::from_secs(2));
        assert_eq!(e.cursor(), 3);
        assert_eq!(e.correct(), 3);
        assert_eq!(e.errors(), 0);
        assert!(e.is_started());
    }

    #[test]
    fn errors_do_not_advance_and_flash() {
        let mut e = Engine::new("abc".into());
        let now = t(0);
        typed(&mut e, 'x', now);
        assert_eq!(e.cursor(), 0);
        assert_eq!(e.errors(), 1);
        assert_eq!(e.flash(now), Some(0));
        // Flash expires.
        assert_eq!(e.flash(now + Duration::from_millis(400)), None);
    }

    #[test]
    fn backspace_steps_back_within_typed_text() {
        let mut e = Engine::new("abc".into());
        let now = t(0);
        typed(&mut e, 'a', now);
        typed(&mut e, 'b', now);
        e.type_key(KeyInput::Backspace, now);
        assert_eq!(e.cursor(), 1);
        typed(&mut e, 'b', now);
        typed(&mut e, 'c', now);
        assert_eq!(e.correct(), 3);
        // Backspace steps back into typed text (re-typing 'c' restores it).
        e.type_key(KeyInput::Backspace, now);
        assert_eq!(e.cursor(), 2);
        typed(&mut e, 'c', now);
        assert_eq!(e.cursor(), 3);
        // Backspace at position 0 is a no-op.
        for _ in 0..3 {
            e.type_key(KeyInput::Backspace, now);
        }
        assert_eq!(e.cursor(), 0);
    }

    #[test]
    fn clock_starts_on_first_printable_key() {
        let mut e = Engine::new("abc".into());
        let now = t(0);
        assert!(!e.is_started());
        e.type_key(KeyInput::Backspace, now);
        assert!(!e.is_started());
        typed(&mut e, 'a', now + Duration::from_secs(3));
        assert!(e.is_started());
        assert_eq!(
            e.elapsed(now + Duration::from_secs(5)),
            Duration::from_secs(2)
        );
    }

    #[test]
    fn timed_mode_finishes_on_elapsed() {
        let mut e = Engine::new("abcdef".into());
        let spec = PromptSpec::Time(Duration::from_secs(15));
        let now = t(0);
        typed(&mut e, 'a', now);
        assert!(!e.check_finished(&spec, now + Duration::from_secs(14)));
        assert!(e.check_finished(&spec, now + Duration::from_secs(15)));
        assert!(e.is_finished());
        // Keys after finish are ignored.
        let errors = e.errors();
        typed(&mut e, 'z', now);
        assert_eq!(e.errors(), errors);
    }

    #[test]
    fn words_mode_finishes_at_target() {
        let mut e = Engine::new("ab cd ef".into());
        let spec = PromptSpec::Words(2);
        let now = Instant::now();
        for c in "ab ".chars() {
            typed(&mut e, c, now);
        }
        assert!(!e.check_finished(&spec, now));
        for c in "cd".chars() {
            typed(&mut e, c, now);
        }
        assert!(e.check_finished(&spec, now));
    }

    #[test]
    fn quote_mode_finishes_at_end() {
        let mut e = Engine::new("hi".into());
        let spec = PromptSpec::Quote(0);
        let now = t(0);
        typed(&mut e, 'h', now);
        assert!(!e.check_finished(&spec, now));
        typed(&mut e, 'i', now);
        assert!(e.check_finished(&spec, now));
    }

    #[test]
    fn finish_reports_wpm_and_accuracy() {
        let mut e = Engine::new("abcde".into());
        let spec = PromptSpec::Words(1);
        let now = t(0);
        for c in "abcde".chars() {
            typed(&mut e, c, now);
        }
        typed(&mut e, 'q', now); // one error
        e.check_finished(&spec, now);
        let r = e.finish(&spec, now + Duration::from_secs(5));
        assert!((r.net_wpm - 12.0).abs() < 1e-3); // 5 chars / 5 = 1 word / (5/60) min
        assert!((r.accuracy - 5.0 / 6.0).abs() < 1e-6);
        assert_eq!(r.errors, 1);
        assert_eq!(r.mode_tag, "words-1");
    }

    #[test]
    fn classify_handles_space_and_modifiers() {
        assert_eq!(classify_key("space", None), KeyInput::Char(' '));
        assert_eq!(classify_key("s", Some("s")), KeyInput::Char('s'));
        assert_eq!(classify_key("s", Some("S")), KeyInput::Char('S'));
        assert_eq!(classify_key("backspace", None), KeyInput::Backspace);
        assert_eq!(classify_key("tab", None), KeyInput::Ignored);
        assert_eq!(classify_key("s", None), KeyInput::Ignored);
        assert_eq!(classify_key("comma", Some(",")), KeyInput::Char(','));
    }

    #[test]
    fn unicode_prompt_counts_by_char() {
        let mut e = Engine::new("héllo".into());
        let now = t(0);
        for c in "héllo".chars() {
            typed(&mut e, c, now);
        }
        assert_eq!(e.cursor(), 5);
        assert_eq!(e.correct(), 5);
        assert!(e.check_finished(&PromptSpec::Quote(0), now));
    }

    #[test]
    fn second_tick_fills_series() {
        let mut e = Engine::new("abcdefghij".into());
        let now = t(0);
        for c in "abcdef".chars() {
            typed(&mut e, c, now);
        }
        e.tick_second(now + Duration::from_secs(3));
        // Seconds 1..=3 recorded; all 6 chars happened at t=0 so each
        // sample sees all 6.
        assert_eq!(e.series_points().len(), 3);
        let pts = e.series_points();
        assert!((pts[0] - 72.0).abs() < 1e-3); // 6 chars / 1 s = 72 wpm
    }
}
