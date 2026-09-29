//! Winding down, if wanted: a few slow breaths paced by the wisp, then a
//! line to think over. Nothing is written or kept.

use serde::Deserialize;

use crate::finds::stable_hash;
use crate::time::UnixMs;

#[derive(Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct WindDown {
    pub ask: String,
    pub yes: String,
    pub no: String,
    pub breaths: u32,
    pub in_seconds: f64,
    pub out_seconds: f64,
    pub in_words: String,
    pub out_words: String,
    pub skip: String,
    reflections: Vec<String>,
}

/// Where a breath is: which one (from zero), whether breathing in, and how
/// far through that half, 0 to 1.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Breath {
    pub count: u32,
    pub inhaling: bool,
    pub through: f64,
}

impl WindDown {
    pub fn bundled() -> WindDown {
        toml::from_str(include_str!("../data/wind-down.toml")).expect("wind-down.toml")
    }

    fn breath_ms(&self) -> f64 {
        (self.in_seconds + self.out_seconds) * 1000.0
    }

    /// The breath `elapsed` milliseconds in, or none once they're done.
    pub fn breath(&self, elapsed: UnixMs) -> Option<Breath> {
        let t = elapsed.max(0) as f64;
        let count = (t / self.breath_ms()) as u32;
        if count >= self.breaths {
            return None;
        }
        let into = t % self.breath_ms() / 1000.0;
        Some(if into < self.in_seconds {
            Breath {
                count,
                inhaling: true,
                through: into / self.in_seconds,
            }
        } else {
            Breath {
                count,
                inhaling: false,
                through: (into - self.in_seconds) / self.out_seconds,
            }
        })
    }

    /// How far through the whole breath, 0 to 1, in then out: the wisp
    /// breathes in time with this.
    pub fn phase(&self, elapsed: UnixMs) -> f64 {
        (elapsed.max(0) as f64 % self.breath_ms()) / self.breath_ms()
    }

    /// Tonight's line to think over: one, so the evening winds down
    /// rather than opening up again.
    pub fn reflections(&self, night: &str) -> Vec<String> {
        let mut all: Vec<&String> = self.reflections.iter().collect();
        all.sort_by_key(|r| stable_hash((0, 0, 11), &format!("{night}:{r}")));
        all.into_iter().take(1).cloned().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::questions::UNSAID;

    #[test]
    fn five_slow_breaths_then_done() {
        let w = WindDown::bundled();
        let b = w.breath(1_000).expect("breathing");
        assert!(b.inhaling && b.count == 0 && (b.through - 0.25).abs() < 1e-9);
        let b = w.breath(7_000).expect("breathing");
        assert!(!b.inhaling && (b.through - 0.5).abs() < 1e-9);
        assert_eq!(w.breath(12_000).map(|b| b.count), Some(1));
        assert!(w.breath(49_999).is_some());
        assert!(w.breath(50_000).is_none());
        // In over the first two fifths, as the wisp's own breath goes.
        assert!((w.in_seconds / (w.in_seconds + w.out_seconds) - 0.4).abs() < 1e-9);
    }

    #[test]
    fn one_line_a_night_that_never_names_the_feeling() {
        let w = WindDown::bundled();
        let tonight = w.reflections("2026-09-29");
        assert_eq!(tonight.len(), 1);
        assert_eq!(tonight, w.reflections("2026-09-29"));
        let all = include_str!("../data/wind-down.toml").to_lowercase();
        for word in UNSAID {
            assert!(!all.contains(word), "{word}");
        }
    }
}
