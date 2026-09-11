//! Session history and best-score persistence (JSON, atomic writes).

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// One completed run as stored on disk.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SessionRecord {
    /// `finished_at` as epoch seconds (chrono `Utc::now` in the UI layer).
    pub finished_at: i64,
    /// Stats key, e.g. "time-30" or "words-25" or "quote-0".
    pub mode_tag: String,
    pub prompt_chars: usize,
    pub duration_s: f32,
    pub net_wpm: f32,
    pub raw_wpm: f32,
    pub accuracy: f32,
    pub errors: usize,
    pub consistency: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Stats {
    /// Best net WPM per mode tag.
    #[serde(default)]
    pub bests: BTreeMap<String, f32>,
    /// Most recent sessions, newest last.
    #[serde(default)]
    pub history: Vec<SessionRecord>,
}

const HISTORY_CAP: usize = 50;

pub struct StatsStore {
    path: PathBuf,
}

impl StatsStore {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }

    /// Load from disk; missing or corrupt files fall back to defaults.
    pub fn load(&self) -> Stats {
        self.load_from(&self.path)
    }

    fn load_from(&self, path: &Path) -> Stats {
        fs::read_to_string(path)
            .ok()
            .and_then(|raw| serde_json::from_str(&raw).ok())
            .unwrap_or_default()
    }

    /// Merge a completed session into bests + history and persist.
    /// Returns true when this run set a new best for its mode.
    pub fn record(&self, mut stats: Stats, record: SessionRecord) -> (Stats, bool) {
        let previous_best = stats.bests.get(&record.mode_tag).copied();
        let new_best = previous_best.is_none_or(|best| record.net_wpm > best);
        if new_best {
            stats.bests.insert(record.mode_tag.clone(), record.net_wpm);
        }
        stats.history.push(record);
        if stats.history.len() > HISTORY_CAP {
            stats.history.drain(0..stats.history.len() - HISTORY_CAP);
        }
        self.save(&stats);
        (stats, new_best)
    }

    fn save(&self, stats: &Stats) {
        atomic_write(
            &self.path,
            &serde_json::to_string_pretty(stats).unwrap_or_default(),
        );
    }

    /// The data directory this store persists into.
    pub fn path(&self) -> &Path {
        &self.path
    }
}

/// Write-to-tmp, fsync, rename — the suite's crash-safe write pattern.
pub fn atomic_write(path: &Path, contents: &str) {
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let tmp = path.with_extension("json.tmp");
    if fs::write(&tmp, contents)
        .and_then(|_| fs::File::open(&tmp).and_then(|f| f.sync_all()))
        .is_ok()
    {
        let _ = fs::rename(&tmp, path);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn store(name: &str) -> (StatsStore, PathBuf) {
        let dir = std::env::temp_dir().join(format!("pitype-stats-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let path = dir.join("stats.json");
        (StatsStore::new(path.clone()), path)
    }

    fn record(wpm: f32, tag: &str) -> SessionRecord {
        SessionRecord {
            finished_at: 0,
            mode_tag: tag.into(),
            prompt_chars: 100,
            duration_s: 30.0,
            net_wpm: wpm,
            raw_wpm: wpm + 5.0,
            accuracy: 0.95,
            errors: 3,
            consistency: 0.8,
        }
    }

    #[test]
    fn load_missing_file_yields_defaults() {
        let (store, path) = store("missing");
        let stats = store.load();
        assert!(stats.bests.is_empty());
        assert!(stats.history.is_empty());
        assert!(!path.exists());
        let _ = fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn record_tracks_best_per_mode() {
        let (store, path) = store("bests");
        let (stats, first) = store.record(store.load(), record(60.0, "time-30"));
        assert!(first);
        let (stats, better) = store.record(stats, record(72.0, "time-30"));
        assert!(better);
        let (stats, worse) = store.record(stats, record(50.0, "time-30"));
        assert!(!worse);
        assert_eq!(stats.bests.get("time-30"), Some(&72.0));
        // Same mode map is independent of other modes.
        let (_, new_mode) = store.record(stats.clone(), record(10.0, "words-50"));
        assert!(new_mode);
        let reloaded = StatsStore::new(path.clone()).load();
        assert_eq!(reloaded.bests.get("time-30"), Some(&72.0));
        assert_eq!(reloaded.bests.get("words-50"), Some(&10.0));
        assert_eq!(reloaded.history.len(), 4);
        let _ = fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn history_caps_at_50() {
        let (store, path) = store("cap");
        let mut stats = store.load();
        for i in 0..60 {
            let (s, _) = store.record(stats, record(i as f32, "time-15"));
            stats = s;
        }
        assert_eq!(stats.history.len(), 50);
        assert_eq!(stats.history[0].net_wpm, 10.0); // oldest kept entries shift off
        let _ = fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn corrupt_file_falls_back_to_defaults() {
        let dir = std::env::temp_dir().join(format!("pitype-corrupt-{}", std::process::id()));
        let _ = fs::create_dir_all(&dir);
        let path = dir.join("stats.json");
        fs::write(&path, "{not json").unwrap();
        let stats = StatsStore::new(path).load();
        assert!(stats.bests.is_empty());
        let _ = fs::remove_dir_all(&dir);
    }
}
