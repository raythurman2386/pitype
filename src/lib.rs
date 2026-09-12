//! Testable typing-app core shared by the GPUI Kit UI.

pub mod engine;
pub mod icons;
pub mod keyboard;
pub mod lessons;
pub mod metrics;
pub mod stats;
pub mod theme;

pub use engine::{classify_key, Engine, KeyInput, Mode, SessionResult};
pub use lessons::{build_prompt, quote_text, PromptSpec, Rng, QUOTES};
pub use metrics::{accuracy, consistency, net_wpm, raw_wpm, WpmSeries};
pub use stats::{SessionRecord, Stats, StatsStore};
pub use theme::{parse_hex_color, OmarchyPalette, RgbaColor};
