//! The wisp as a guide. On the first night it shows the way: what the sky
//! is, what to do with a weight, how to catch something, where the next one
//! is. After that it mostly keeps quiet, speaking up if someone seems stuck
//! or asks (? or a click on it). It brightens when something is caught and
//! falls asleep when the lights go out.

use crate::game::{Game, envelope};
use crate::sprite::{NOOK, SIZE, render};
use crate::view::{Bubble, Sprite};
use night_sky_core::finds::{Target, stable_hash};
use night_sky_core::session::Phase;
use night_sky_core::time::UnixMs;
use night_sky_core::wisp::{Mode, Trend, Wisp};

struct Line {
    text: String,
    shown: UnixMs,
    hold: UnixMs,
}

pub struct Guide {
    pub(crate) wisp: Wisp,
    line: Option<Line>,
    /// Lines waiting for the one before to be read.
    queue: Vec<Line>,
    cheer_until: UnixMs,
    last_nudge: UnixMs,
    last_progress: UnixMs,
    pub(crate) scale: f64,
}

/// How long someone can look around without finding anything before the
/// wisp offers to help.
const STUCK_MS: UnixMs = 35_000;
/// Long enough to read a line before another replaces it.
const READ_MS: UnixMs = 6_000;
const NUDGE_EVERY_MS: UnixMs = 90_000;

fn number(n: usize) -> String {
    const WORDS: [&str; 8] = ["No", "One", "Two", "Three", "Four", "Five", "Six", "Seven"];
    WORDS
        .get(n)
        .map(|w| (*w).to_owned())
        .unwrap_or_else(|| n.to_string())
}

impl Guide {
    pub fn new(real: UnixMs) -> Guide {
        let mut wisp = Wisp::new(NOOK.0, NOOK.1);
        // It wakes as the sky arrives.
        wisp.update(0.05, Mode::Away, Trend::Steady, false, true, false);
        Guide {
            wisp,
            line: None,
            queue: Vec::new(),
            cheer_until: 0,
            last_nudge: 0,
            last_progress: real,
            scale: 1.0,
        }
    }
}

impl Game {
    fn seen(&self, key: &str) -> bool {
        self.journal.settings.seen.iter().any(|k| k == key)
    }

    fn mark_seen(&mut self, key: &str) {
        if !self.seen(key) {
            self.journal.settings.seen.push(key.to_owned());
            if let Err(e) = self.journal.save_settings() {
                eprintln!("night-sky: couldn't save settings: {e}");
            }
        }
    }

    /// Says something. A line already showing gets a fair chance to be read
    /// first (at least `READ_MS`), then gives way.
    pub(crate) fn say(&mut self, text: impl Into<String>, real: UnixMs, hold: UnixMs) {
        let line = Line {
            text: text.into(),
            shown: real,
            hold,
        };
        match &mut self.guide.line {
            Some(current)
                if real - current.shown < READ_MS && current.shown + current.hold > real =>
            {
                // Cut the current one short once it has been read, and follow it.
                current.hold = current.hold.min(READ_MS);
                let start = current.shown + current.hold + 900;
                self.guide.queue.retain(|l| l.text != line.text);
                self.guide.queue.push(Line {
                    shown: start.max(real),
                    ..line
                });
            }
            _ => {
                self.guide.queue.clear();
                self.guide.line = Some(line);
            }
        }
    }

    /// Says something the first time only, ever.
    fn say_once(&mut self, key: &str, text: impl Into<String>, real: UnixMs, hold: UnixMs) -> bool {
        if self.seen(key) {
            return false;
        }
        self.mark_seen(key);
        self.say(text, real, hold);
        true
    }

    pub(crate) fn hush(&mut self) {
        self.guide.line = None;
        self.guide.queue.clear();
    }

    fn first_night(&self) -> bool {
        !self.seen("hunt")
    }

    fn to_find(&self) -> usize {
        self.caught.iter().filter(|c| !**c).count()
    }

    pub(crate) fn guide_arrival(&mut self, real: UnixMs) {
        let n = self.to_find();
        let things = match n {
            0 => String::new(),
            1 => " One thing to find tonight.".into(),
            n => format!(" {} things to find tonight.", number(n)),
        };
        if !self.seen("hello") {
            self.mark_seen("hello");
            self.say(
                "Hello. I'm the wisp. This is the real sky over you tonight, just as it is outside.",
                real + 1_200,
                7_000,
            );
            return;
        }
        let greetings = [
            "Hello again.",
            "Good to see you.",
            "The sky has turned since you were last here.",
            "Clear skies in here, at least.",
        ];
        let pick = stable_hash((0, 0, 0), &self.night) as usize % greetings.len();
        let text = if n == 0 && !self.finds.is_empty() {
            format!(
                "{} You've found tonight's sky already; look around as long as you like.",
                greetings[pick]
            )
        } else if self.finds.is_empty() {
            format!(
                "{} It's still light out, so there's not much to find yet.",
                greetings[pick]
            )
        } else {
            format!("{}{things}", greetings[pick])
        };
        self.say(text, real + 1_200, 6_000);
    }

    pub(crate) fn guide_weights(&mut self, real: UnixMs) {
        self.say_once(
            "weights",
            "Before we look up: if something's weighing on you, tell me and I'll hang it in the west for you. Esc if not tonight.",
            real,
            30_000,
        );
    }

    pub(crate) fn guide_placing(&mut self, real: UnixMs) {
        let text = "Find it a spot low in the west with the arrows, then press Enter.";
        if !self.say_once("placing", text, real, 60_000) {
            self.say(text, real, 60_000);
        }
    }

    pub(crate) fn guide_placed(&mut self) {
        self.hush();
    }

    pub(crate) fn guide_hunt(&mut self, real: UnixMs) {
        self.guide.last_progress = real;
        let n = self.to_find();
        if n == 0 {
            return;
        }
        let what = if n == 1 {
            "One thing is worth finding tonight: the little circle in the corner.".to_owned()
        } else {
            format!(
                "{} things are worth finding tonight: the little circles in the corner.",
                number(n)
            )
        };
        self.say_once(
            "hunt",
            format!("{what} Look around with the arrows, or drag the sky."),
            real + 500,
            12_000,
        );
    }

    pub(crate) fn guide_caught(&mut self, real: UnixMs) {
        self.guide.cheer_until = real + 4_000;
        self.guide.last_progress = real;
        if self.say_once(
            "caught",
            "Lovely. Press Space when you've read it, and Tab turns you towards the next one.",
            real + 600,
            9_000,
        ) {
            return;
        }
        let caught = self.caught.iter().filter(|c| **c).count();
        if caught >= 2 {
            self.say_once(
                "draw",
                "You can join stars into a shape of your own, too: press C.",
                real + 600,
                8_000,
            );
        }
    }

    pub(crate) fn guide_all_found(&mut self, real: UnixMs) {
        self.guide.cheer_until = real + 5_000;
        self.say(
            "That's tonight's sky, all of it. Look around as long as you like, then Esc twice to finish.",
            real,
            9_000,
        );
    }

    pub(crate) fn guide_meteor_left(&mut self, real: UnixMs) {
        self.say(
            "The last one is a meteor. Keep watching, and press Space the moment one flies.",
            real,
            8_000,
        );
    }

    pub(crate) fn guide_question(&mut self, real: UnixMs) {
        self.say_once(
            "question",
            "Now and then the sky asks something. Answer if you like, or Esc to let it pass. It all goes in the logbook, which L opens.",
            real + 400,
            12_000,
        );
    }

    pub(crate) fn guide_drawing(&mut self, real: UnixMs) {
        self.say(
            "Arrows step between bright stars, Enter joins them, Backspace takes one back. Press C when it's done.",
            real,
            600_000,
        );
    }

    pub(crate) fn guide_help(&mut self, real: UnixMs) {
        self.say(
            "Arrows or a drag look around. Hold Space to catch whatever's in the ring. Tab turns you to the next find, C draws, L opens the logbook, and Esc twice ends the night.",
            real,
            14_000,
        );
    }

    pub(crate) fn guide_phase(&mut self, phase: Phase, real: UnixMs) {
        match phase {
            Phase::Dimming => self.say("Let's give your eyes a rest.", real + 600, 6_000),
            Phase::Finale => {
                if self.page.weights.is_empty() {
                    self.say("Watch the sky turn.", real + 800, 6_000);
                } else {
                    self.say(
                        "Watch the west. The sky takes tonight's weights down with it.",
                        real + 800,
                        8_000,
                    );
                }
            }
            Phase::LightsOut => self.say("Goodnight.", real, 5_000),
            _ => {}
        }
    }

    /// Nudges someone who seems stuck, now and then, and keeps the face right.
    pub(crate) fn guide_tick(&mut self, real: UnixMs) {
        let hunting = self.hunting()
            && self.card.is_none()
            && self.talk.prompt.is_none()
            && self.drawing.is_none();
        if hunting && self.catch.target.is_some() && self.first_night_ring_pending() {
            self.say_once(
                "ring",
                "There's one, inside the ring. Hold Space.",
                real,
                8_000,
            );
        }
        let left = self.to_find();
        if hunting
            && left > 0
            && real - self.guide.last_progress.max(self.last_input) > STUCK_MS
            && real - self.guide.last_nudge > NUDGE_EVERY_MS
        {
            self.guide.last_nudge = real;
            let only_meteor = (0..self.finds.len())
                .filter(|&i| !self.caught[i])
                .all(|i| matches!(self.finds[i].target, Target::Meteor(_)));
            if self.catch.target.is_some() {
                self.say(
                    "There's one in the ring. Hold Space to catch it.",
                    real,
                    8_000,
                );
            } else if only_meteor {
                self.guide_meteor_left(real);
            } else {
                self.say(
                    "Lost? Press Tab and I'll turn you towards the next one.",
                    real,
                    8_000,
                );
            }
        }
        let mode = match self.session.phase() {
            Phase::LightsOut | Phase::Over => Mode::Away,
            Phase::Arrival if self.session.in_phase(real) < 1_200 => Mode::Away,
            _ if real < self.guide.cheer_until => Mode::Nourishing,
            _ if self.talk.prompt.is_some() => Mode::Holding,
            _ => Mode::Resting,
        };
        self.guide
            .wisp
            .update(0.05, mode, Trend::Steady, false, true, false);
    }

    fn first_night_ring_pending(&self) -> bool {
        self.first_night() || !self.seen("ring")
    }

    /// Where the wisp's nook sits: on the horizon, bottom left.
    pub(crate) fn nook(&self) -> (f64, f64, f64, f64) {
        let (w, h) = (NOOK.0 * SIZE, NOOK.1 * SIZE);
        (10.0, self.camera.height - h - 34.0, w, h)
    }

    pub(crate) fn on_wisp(&self, x: f64, y: f64) -> bool {
        let (nx, ny, nw, nh) = self.nook();
        x >= nx && x <= nx + nw && y >= ny - 10.0 && y <= ny + nh
    }

    pub(crate) fn guide_frame(
        &mut self,
        real: UnixMs,
        brightness: f64,
    ) -> (Option<Sprite>, Option<Bubble>) {
        let (x, y, w, h) = self.nook();
        let alpha = if matches!(self.session.phase(), Phase::LightsOut | Phase::Over) {
            brightness / night_sky_core::session::DIM
        } else {
            brightness.max(0.6)
        };
        let sprite =
            render(&mut self.guide.wisp, real as f64, self.guide.scale).map(|texture| Sprite {
                texture,
                x,
                y,
                width: w,
                height: h,
                alpha,
            });
        let finished = self
            .guide
            .line
            .as_ref()
            .is_none_or(|l| real - l.shown > l.hold + 1_400);
        if finished && !self.guide.queue.is_empty() {
            let next = self.guide.queue.remove(0);
            if next.shown <= real {
                self.guide.line = Some(next);
            } else {
                self.guide.queue.insert(0, next);
            }
        }
        let bubble = self.guide.line.as_ref().and_then(|l| {
            let a = envelope(real - l.shown, 500, l.hold, 900);
            (a > 0.0).then(|| Bubble {
                x: x + 26.0,
                bottom: y + 14.0,
                text: l.text.clone(),
                width: 300.0,
                alpha: a * alpha.min(1.0),
            })
        });
        (sprite, bubble)
    }
}
