//! What the wisp does on its moss between times: looks up at something
//! really in the sky, watches the pointer for a moment, tidies its moss,
//! stretches, hops, yawns when it's late, or just looks out at whoever's
//! there. One thing at a time, with long quiet gaps, and never the same
//! thing twice running.

use westering_core::time::UnixMs;
use westering_core::wisp::Gesture;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// Looks up at one of tonight's finds, or the Moon.
    LookUp,
    /// Follows the pointer with its eyes.
    Watch,
    /// Looks straight out, warmly.
    LookOut,
    Tend,
    Stretch,
    Hop,
    Yawn,
}

impl Kind {
    const ALL: [Kind; 7] = [
        Kind::LookUp,
        Kind::Watch,
        Kind::LookOut,
        Kind::Tend,
        Kind::Stretch,
        Kind::Hop,
        Kind::Yawn,
    ];

    /// How likely it is, when it can happen.
    fn weight(self) -> f64 {
        match self {
            Kind::LookUp => 3.0,
            Kind::Watch => 2.0,
            Kind::LookOut => 2.0,
            Kind::Tend => 1.5,
            Kind::Stretch => 1.0,
            Kind::Hop => 0.6,
            Kind::Yawn => 2.0,
        }
    }

    /// How long it lasts.
    fn length(self) -> UnixMs {
        match self {
            Kind::LookUp => 3_500,
            Kind::Watch => 4_500,
            Kind::LookOut => 2_600,
            Kind::Tend => 2_600,
            Kind::Stretch => 1_700,
            Kind::Hop => 700,
            Kind::Yawn => 2_000,
        }
    }

    /// Only the eyes move: the rest is left out with calm motion on.
    pub fn eyes_only(self) -> bool {
        matches!(self, Kind::LookUp | Kind::Watch | Kind::LookOut)
    }

    pub fn gesture(self) -> Option<Gesture> {
        match self {
            Kind::Tend => Some(Gesture::Tend),
            Kind::Stretch => Some(Gesture::Stretch),
            Kind::Hop => Some(Gesture::Hop),
            Kind::Yawn => Some(Gesture::Yawn),
            Kind::LookOut => Some(Gesture::Glow),
            Kind::LookUp | Kind::Watch => None,
        }
    }
}

/// What's going on around it, for choosing.
pub struct Scene {
    /// Something worth looking up at is on the screen.
    pub sky: bool,
    /// The pointer moved a moment ago.
    pub pointer: bool,
    pub sleepy: bool,
    pub calm: bool,
    /// How slow the evening has become, 1 at the start and less later.
    pub tempo: f64,
}

pub struct Idle {
    next: UnixMs,
    last: Option<Kind>,
    doing: Option<(Kind, UnixMs)>,
    seed: u64,
}

impl Idle {
    pub fn new(now: UnixMs) -> Idle {
        Idle {
            next: now + 10_000,
            last: None,
            doing: None,
            seed: 0x51_7cc1_b727_220a ^ now as u64,
        }
    }

    fn random(&mut self) -> f64 {
        self.seed ^= self.seed << 13;
        self.seed ^= self.seed >> 7;
        self.seed ^= self.seed << 17;
        (self.seed >> 11) as f64 / (1u64 << 53) as f64
    }

    /// What it's doing now, if anything.
    pub fn doing(&self, now: UnixMs) -> Option<Kind> {
        self.doing
            .filter(|&(k, start)| now >= start && now - start < k.length())
            .map(|(k, _)| k)
    }

    /// Something else needs it: whatever it was doing stops, and the next
    /// idle moment is a while off.
    pub fn interrupt(&mut self, now: UnixMs) {
        self.doing = None;
        self.next = self.next.max(now + 12_000);
    }

    /// Chooses something to do, when it's time. Returns a newly begun one.
    pub fn tick(&mut self, now: UnixMs, scene: &Scene) -> Option<Kind> {
        if self.doing(now).is_some() || now < self.next {
            return None;
        }
        let can = |k: Kind| {
            Some(k) != self.last
                && (!scene.calm || k.eyes_only())
                && match k {
                    Kind::LookUp => scene.sky,
                    Kind::Watch => scene.pointer,
                    Kind::Yawn => scene.sleepy,
                    Kind::Hop => !scene.sleepy,
                    _ => true,
                }
        };
        let choices: Vec<Kind> = Kind::ALL.into_iter().filter(|&k| can(k)).collect();
        let total: f64 = choices.iter().map(|k| k.weight()).sum();
        // Quiet gaps that lengthen as the evening slows.
        let gap = (8_000.0 + self.random() * 14_000.0) / scene.tempo.max(0.3);
        self.next = now + gap as UnixMs;
        if total <= 0.0 {
            return None;
        }
        let mut pick = self.random() * total;
        let kind = *choices
            .iter()
            .find(|k| {
                pick -= k.weight();
                pick <= 0.0
            })
            .unwrap_or(&choices[0]);
        self.last = Some(kind);
        self.doing = Some((kind, now));
        Some(kind)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scene() -> Scene {
        Scene {
            sky: true,
            pointer: true,
            sleepy: false,
            calm: false,
            tempo: 1.0,
        }
    }

    #[test]
    fn never_the_same_twice_running_and_never_busy() {
        let mut idle = Idle::new(0);
        let mut last = None;
        let mut t = 0;
        let mut count = 0;
        while t < 3_600_000 {
            if let Some(k) = idle.tick(t, &scene()) {
                assert_ne!(Some(k), last);
                last = Some(k);
                count += 1;
            }
            t += 500;
        }
        // Somewhere between one every twenty seconds and one a minute.
        assert!((60..=450).contains(&count), "{count}");
    }

    #[test]
    fn calm_motion_keeps_to_the_eyes_and_yawns_wait_for_late() {
        let (mut calm, mut awake) = (Idle::new(0), Idle::new(0));
        let calm_scene = Scene {
            calm: true,
            ..scene()
        };
        let mut t = 0;
        while t < 3_600_000 {
            if let Some(k) = calm.tick(t, &calm_scene) {
                assert!(k.eyes_only(), "{k:?}");
            }
            if let Some(k) = awake.tick(t, &scene()) {
                assert_ne!(k, Kind::Yawn);
            }
            t += 500;
        }
    }
}
