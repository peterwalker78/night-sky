//! The wisp as a guide. It lives on a tuft of moss on the horizon, but it's
//! free to fly: off to the list, the compass, the ring or whatever it's
//! talking about, looping wide round things in the sky, trailing embers.
//! On the first night it shows the way and says what each part is for.
//! After that it mostly keeps quiet, speaking up if someone seems stuck or
//! asks (? or a click on it). It brightens when something is caught and
//! goes home to sleep when the lights go out.

use crate::flight::Flight;
use crate::game::{Game, envelope};
use crate::sprite::{FLYING, NOOK, SIZE, render_flying, render_moss};
use crate::talk::Flow;
use crate::view::{Bubble, Point, Sprite};
use westering_core::coords::{apply, unit};
use westering_core::finds::{Target, stable_hash};
use westering_core::session::Phase;
use westering_core::time::UnixMs;
use westering_core::wisp::{Mode, Trend, Wisp};

/// A place on the screen, in pixels.
type Spot = (f64, f64);

/// Where the wisp goes while it says something.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Aim {
    /// Stay where it is.
    Stay,
    Home,
    /// A spot on the screen, as fractions of its width and height.
    Near(f64, f64),
    /// The Tonight list.
    List,
    /// The strip at the top.
    Compass,
    /// The prompt at the foot of the sky.
    Prompt,
    /// The card for what was just caught.
    Card,
    /// The ring in the middle.
    Ring,
    /// One of tonight's finds, wherever it is on the screen.
    Find(usize),
    /// Tonight's weights, low in the west.
    Weights,
    /// A place on the sky, by right ascension and declination (J2000).
    Sky(f64, f64),
    /// A place on the Moon, by index into the features.
    Moon(usize),
    /// The evening's guide, top left.
    Evening,
}

struct Line {
    text: String,
    shown: UnixMs,
    hold: UnixMs,
    aim: Aim,
}

pub struct Guide {
    pub(crate) wisp: Wisp,
    line: Option<Line>,
    /// Lines waiting for the one before to be read.
    queue: Vec<Line>,
    cheer_until: UnixMs,
    last_nudge: UnixMs,
    pub(crate) last_progress: UnixMs,
    pub(crate) scale: f64,
    flight: Flight,
    /// Where it's going, and until when it has something to do there.
    aim: Aim,
    busy_until: UnixMs,
    /// A little flight of its own now and then, when nothing's going on.
    next_wander: UnixMs,
    wander: Option<(f64, f64, UnixMs)>,
    /// When it arrived at what it's pointing at, for the glow round it.
    pointing_since: Option<UnixMs>,
    last_frame: UnixMs,
}

/// How long someone can look around without finding anything before the
/// wisp offers to help.
const STUCK_MS: UnixMs = 35_000;
/// Long enough to read a line before another replaces it.
const READ_MS: UnixMs = 6_000;
const NUDGE_EVERY_MS: UnixMs = 90_000;
/// How long it lingers after speaking before it drifts home.
const LINGER_MS: UnixMs = 5_000;

fn number(n: usize) -> String {
    const WORDS: [&str; 13] = [
        "No", "One", "Two", "Three", "Four", "Five", "Six", "Seven", "Eight", "Nine", "Ten",
        "Eleven", "Twelve",
    ];
    WORDS
        .get(n)
        .map(|w| (*w).to_owned())
        .unwrap_or_else(|| n.to_string())
}

impl Guide {
    pub fn new(real: UnixMs) -> Guide {
        let mut wisp = Wisp::new(FLYING.0, FLYING.1);
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
            flight: Flight::new(-100.0, -100.0),
            aim: Aim::Home,
            busy_until: 0,
            next_wander: real + 40_000,
            wander: None,
            pointing_since: None,
            last_frame: real,
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
                eprintln!("westering: couldn't save settings: {e}");
            }
        }
    }

    /// Says something where it is.
    pub(crate) fn say(&mut self, text: impl Into<String>, real: UnixMs, hold: UnixMs) {
        self.say_at(Aim::Stay, text, real, hold);
    }

    /// Flies somewhere and says something. A line already showing gets a
    /// fair chance to be read first (at least `READ_MS`), then gives way.
    pub(crate) fn say_at(&mut self, aim: Aim, text: impl Into<String>, real: UnixMs, hold: UnixMs) {
        let line = Line {
            text: text.into(),
            shown: real,
            hold,
            aim,
        };
        let showing = self
            .guide
            .line
            .as_ref()
            .is_some_and(|c| c.shown + c.hold > real);
        if showing {
            // Follow whatever is showing or waiting, each read in turn.
            let last = self.guide.queue.last_mut().or(self.guide.line.as_mut());
            if let Some(last) = last {
                last.hold = last.hold.min(READ_MS);
                let start = last.shown + last.hold + 900;
                self.guide.queue.retain(|l| l.text != line.text);
                self.guide.queue.push(Line {
                    shown: start.max(real),
                    ..line
                });
            }
        } else {
            self.guide.queue.clear();
            self.guide.line = Some(line);
        }
    }

    /// Says something the first time only, ever.
    fn say_once(
        &mut self,
        key: &str,
        aim: Aim,
        text: impl Into<String>,
        real: UnixMs,
        hold: UnixMs,
    ) -> bool {
        if self.seen(key) {
            return false;
        }
        self.mark_seen(key);
        self.say_at(aim, text, real, hold);
        true
    }

    /// Flies somewhere without saying anything, and lingers a while.
    pub(crate) fn fly(&mut self, aim: Aim, real: UnixMs, linger: UnixMs) {
        if self.guide.aim != aim {
            self.guide.aim = aim;
            self.guide.pointing_since = None;
        }
        self.guide.busy_until = self.guide.busy_until.max(real + linger);
    }

    /// Hurries on to the next line waiting, if there is one.
    pub(crate) fn guide_next(&mut self, real: UnixMs) -> bool {
        if self.guide.queue.is_empty() {
            return false;
        }
        let mut next = self.guide.queue.remove(0);
        next.shown = real;
        for (k, later) in self.guide.queue.iter_mut().enumerate() {
            later.shown = later.shown.min(real + (k as UnixMs + 1) * (READ_MS + 900));
        }
        self.guide.line = Some(next);
        true
    }

    pub(crate) fn hush(&mut self) {
        self.guide.line = None;
        self.guide.queue.clear();
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
            self.say_at(
                Aim::Near(0.36, 0.42),
                "Hello. I'm the wisp. This is the real sky over you tonight, just as it is outside.",
                real + 1_200,
                7_000,
            );
            self.say_at(
                Aim::Near(0.4, 0.4),
                "Come here for a few quiet minutes at the end of the day: to put the day down, come back to what matters to you, and get ready for sleep. The sky is never the same two nights running, so there's always something new to find.",
                real + 1_200,
                9_000,
            );
            self.say_at(
                Aim::Evening,
                "Every evening has the same three parts, shown up here: set down what's on your mind, look up for a while, then wind down.",
                real + 1_200,
                9_000,
            );
            return;
        }
        let greetings = [
            "Hello again.",
            "Good to see you.",
            "The sky has turned since you were last here.",
            "Clear skies in here, at least.",
            "Let's set the day down for a while.",
            "A few quiet minutes, and then the rest of the night is yours.",
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
        self.say_at(Aim::Near(0.3, 0.5), text, real + 1_200, 6_000);
    }

    pub(crate) fn guide_weights(&mut self, real: UnixMs) {
        self.say_once(
            "weights",
            Aim::Prompt,
            "Each evening starts here. A worry written down is easier to put down, and I'll hang yours low in the west. Only you will ever see it.",
            real,
            30_000,
        );
    }

    pub(crate) fn guide_placing(&mut self, real: UnixMs) {
        // The prompt says what to do; the wisp only says why, the first time.
        self.say_once(
            "placing",
            Aim::Ring,
            "Things written down are easier to put down. When the sky sets tonight, that goes with it.",
            real,
            12_000,
        );
    }

    pub(crate) fn guide_placed(&mut self) {
        self.hush();
    }

    pub(crate) fn guide_hunt(&mut self, real: UnixMs) {
        self.guide.last_progress = real;
        let n = self.to_find();
        if n == 0 || self.seen("hunt") {
            return;
        }
        self.mark_seen("hunt");
        let what = if n == 1 {
            "One thing is worth finding tonight. It's listed here, with where to look. Click it and I'll turn you to it.".to_owned()
        } else {
            format!(
                "{} things are worth finding tonight. They're listed here, with where to look for each. Click one and I'll turn you to it.",
                number(n)
            )
        };
        self.say_at(Aim::List, what, real + 500, 9_000);
        self.say_at(
            Aim::Compass,
            "This strip shows which way you're facing. The arrows, or a drag, turn you round.",
            real + 500,
            7_000,
        );
        self.say_at(
            Aim::Ring,
            "When something's inside the ring, hold Space to catch it. Tab turns you to the next one.",
            real + 500,
            8_000,
        );
        self.say_at(
            Aim::Near(0.5, 0.35),
            "Point at anything in the sky to see what it is, and click it to hear more.",
            real + 500,
            8_000,
        );
    }

    pub(crate) fn guide_caught(&mut self, real: UnixMs) {
        // Tips about the ring are done with once something's caught.
        self.guide.queue.retain(|l| l.aim != Aim::Ring);
        if self.guide.line.as_ref().is_some_and(|l| l.aim == Aim::Ring) {
            self.guide.line = None;
        }
        self.guide.cheer_until = real + 4_000;
        self.guide.last_progress = real;
        // Over to the card, pleased.
        self.fly(Aim::Card, real, 4_000);
        if self.say_once(
            "caught",
            Aim::Card,
            "Lovely. Each one comes with something true about it. Space puts the card away, and Tab turns you to the next.",
            real + 600,
            9_000,
        ) {
            return;
        }
        let caught = self.caught.iter().filter(|c| **c).count();
        if caught >= 2 {
            self.say_once(
                "draw",
                Aim::Ring,
                "You can join bright stars into a shape of your own, too: press C. People have drawn the sky that way for thousands of years.",
                real + 600,
                9_000,
            );
        }
    }

    /// A story or a walk has begun: the wisp goes along, and keeps its tips
    /// for later.
    pub(crate) fn guide_tour_began(&mut self, real: UnixMs) {
        self.guide.cheer_until = real + 3_000;
        self.guide.last_progress = real;
        self.hush();
    }

    pub(crate) fn guide_all_found(&mut self, real: UnixMs) {
        self.guide.cheer_until = real + 5_000;
        if self.say_once(
            "tomorrow",
            Aim::Near(0.5, 0.35),
            "That's tonight's sky. By tomorrow it will have turned: new things will be up and the Moon will have moved on. Look around as long as you like, then W, or Wind down at the top left, when you're ready.",
            real,
            11_000,
        ) {
            return;
        }
        self.say_at(
            Aim::Near(0.5, 0.35),
            "That's tonight's sky, all of it. Look around as long as you like, then wind down when you're ready.",
            real,
            9_000,
        );
    }

    pub(crate) fn guide_meteor_left(&mut self, real: UnixMs) {
        self.say_at(
            Aim::Near(0.5, 0.3),
            "The last one is a meteor. Keep watching, and press Space the moment one flies.",
            real,
            8_000,
        );
    }

    pub(crate) fn guide_question(&mut self, real: UnixMs) {
        let (plan, name) = match &self.talk.flow {
            Some(Flow::Question { chosen, .. }) => (
                chosen.question.thread == "plans",
                chosen.question.answer == westering_core::questions::AnswerKind::Name,
            ),
            _ => (false, false),
        };
        if plan
            && self.say_once(
                "plans",
                Aim::Prompt,
                "Something to look forward to is worth having. If you make a plan, I'll mention it when its night comes near.",
                real + 400,
                11_000,
            )
        {
            return;
        }
        if name
            && self.seen("question")
            && self.say_once(
                "names",
                Aim::Prompt,
                "Just a first name will do. Only you will ever see it.",
                real + 400,
                8_000,
            )
        {
            return;
        }
        self.say_once(
            "question",
            Aim::Prompt,
            "Now and then the sky asks something small. There's no right answer, and only you will ever see what you write. Esc lets it pass.",
            real + 400,
            12_000,
        );
    }

    /// After something's been written down, where it all goes.
    pub(crate) fn guide_answered(&mut self, real: UnixMs) {
        self.say_once(
            "logbook",
            Aim::Near(0.3, 0.55),
            "It's kept in your logbook, which L opens. Anything that keeps coming up gathers on a page of its own, so over time you can see what matters.",
            real + 800,
            11_000,
        );
    }

    pub(crate) fn guide_look_back(&mut self, real: UnixMs) {
        self.say_once(
            "look-back",
            Aim::Prompt,
            "Now and then I'll bring back something you set down a while ago. Most weights change shape once they're written down; it helps to notice when one has.",
            real + 400,
            11_000,
        );
    }

    /// Tonight's end chosen: what happens now.
    pub(crate) fn guide_ending(&mut self, ending: crate::game::Ending, real: UnixMs) {
        let line = match ending {
            crate::game::Ending::Outside => {
                "Then I'll show you what's up out there. Give your eyes twenty minutes in the dark: they keep opening all that time."
            }
            crate::game::Ending::Bed => {
                "Then this is the last of the screen tonight. Watch the day go down with the sky."
            }
        };
        self.say_at(Aim::Near(0.4, 0.4), line, real + 300, 9_000);
    }

    /// The first night's end: where tonight is kept.
    pub(crate) fn guide_logbook_at_end(&mut self, real: UnixMs) {
        self.say_once(
            "logbook-end",
            Aim::Home,
            "Everything from tonight is in your logbook: what you found, what you set down, what you wrote. L opens it, any night.",
            real + 12_000,
            9_000,
        );
    }

    /// A word after an old weight has been looked at again.
    pub(crate) fn guide_looked_back(&mut self, chip: usize, real: UnixMs) {
        let line = match chip {
            0 | 3 => "Good. Whatever helped with that is worth remembering.",
            1 => "Some things take longer. It can stay in the west as long as it needs.",
            _ => {
                "If you'd like, the logbook can help you chart a small course for it. Only if you want to."
            }
        };
        self.say_at(Aim::Near(0.3, 0.55), line, real + 500, 8_000);
    }

    pub(crate) fn guide_drawing(&mut self, real: UnixMs) {
        self.say_at(
            Aim::Ring,
            "Arrows step between bright stars, Enter joins them, Backspace takes one back. Press C when it's done.",
            real,
            600_000,
        );
    }

    pub(crate) fn guide_help(&mut self, real: UnixMs) {
        self.say_at(
            Aim::Near(0.32, 0.5),
            "Arrows or a drag look around. Hold Space to catch whatever's in the ring. Tab, or a click on the list, turns you to the next find. Point at anything to see what it is. C draws, L opens the logbook, M turns the music off or on, and W winds down. The button top left opens the menu.",
            real,
            15_000,
        );
    }

    pub(crate) fn guide_phase(&mut self, phase: Phase, real: UnixMs) {
        match phase {
            Phase::Dimming => self.say_at(
                Aim::Prompt,
                "Let's wind down. I'll dim the screen a little at a time: bright light keeps a mind awake, and eyes need the dark to see the faint stars.",
                real + 600,
                10_000,
            ),
            Phase::Finale => {
                let first = self
                    .page
                    .weights
                    .first()
                    .and_then(|w| self.journal.weight(w.weight))
                    .map(|w| w.text.clone());
                if let Some(first) = first {
                    self.say_at(
                        Aim::Weights,
                        format!("Watch the west. The turning sky takes \u{201c}{first}\u{201d} down with it."),
                        real + 800,
                        9_000,
                    );
                } else if self.page.weights.is_empty() {
                    self.say_at(
                        Aim::Near(0.4, 0.45),
                        "Watch the sky turn.",
                        real + 800,
                        6_000,
                    );
                } else {
                    self.say_at(
                        Aim::Weights,
                        "Watch the west. The sky takes tonight's weights down with it.",
                        real + 800,
                        9_000,
                    );
                }
            }
            Phase::LightsOut => self.say_at(Aim::Home, "Goodnight.", real, 5_000),
            _ => {}
        }
    }

    /// Nudges someone who seems stuck, now and then, and keeps the face right.
    pub(crate) fn guide_tick(&mut self, real: UnixMs) {
        let hunting = self.hunting()
            && self.card.is_none()
            && self.talk.prompt.is_none()
            && self.drawing.is_none();
        if hunting && self.catch.target.is_some() {
            self.say_once(
                "ring",
                Aim::Ring,
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
                self.say_at(
                    Aim::Ring,
                    "There's one in the ring. Hold Space to catch it.",
                    real,
                    8_000,
                );
            } else if only_meteor {
                self.guide_meteor_left(real);
            } else {
                self.say_at(
                    Aim::List,
                    "Lost? Pick one from the list, or press Tab, and I'll turn you to it.",
                    real,
                    8_000,
                );
            }
        }
        // A long look: offer to wind down, once.
        if hunting
            && self.session.in_phase(real) > self.session.timings.hunt_most * 4 / 5
            && !self.offered_wind_down()
        {
            self.set_offered_wind_down();
            self.say_at(
                Aim::Evening,
                "It's getting late. When you're ready, W winds down, or Wind down up here.",
                real,
                10_000,
            );
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

    /// Where the wisp's nook sits: on the horizon, bottom left.
    pub(crate) fn nook(&self) -> (f64, f64, f64, f64) {
        let (w, h) = (NOOK.0 * SIZE, NOOK.1 * SIZE);
        (10.0, self.camera.height - h - 34.0, w, h)
    }

    /// Where the wisp's body is when it sits on its moss.
    fn home_spot(&self) -> (f64, f64) {
        let (x, y, _, _) = self.nook();
        (
            x + NOOK.0 / 2.0 * SIZE,
            y + (NOOK.1 - 7.0 - 8.0 - 13.5 * 1.15) * SIZE,
        )
    }

    pub(crate) fn on_wisp(&self, x: f64, y: f64) -> bool {
        let (wx, wy) = if self.guide.flight.home {
            self.home_spot()
        } else {
            (self.guide.flight.x, self.guide.flight.y)
        };
        ((x - wx).powi(2) + (y - wy).powi(2)).sqrt() < 34.0
    }

    /// What an aim means on the screen now: where to hover, and the thing
    /// being pointed at, if any.
    fn resolve(&self, aim: Aim) -> Option<(Spot, Option<Spot>)> {
        let (w, h) = (self.camera.width, self.camera.height);
        let (cx, cy) = (w / 2.0, h / 2.0);
        let beside = |px: f64, py: f64| {
            // Hover off to the side nearer the middle of the screen.
            let dx = if px > cx { -110.0 } else { 110.0 };
            (
                (px + dx).clamp(60.0, w - 60.0),
                (py - 45.0).clamp(60.0, h - 80.0),
            )
        };
        Some(match aim {
            Aim::Stay => return None,
            Aim::Home => (self.home_spot(), None),
            Aim::Near(fx, fy) => ((w * fx, h * fy), None),
            Aim::List => ((w - 330.0, 110.0), Some((w - 270.0, 70.0))),
            Aim::Compass => ((cx - 350.0, 44.0), Some((cx - 250.0, 28.0))),
            Aim::Evening => ((150.0, 130.0), Some((120.0, 40.0))),
            // Above the prompt's left end, so the words sit clear of it.
            Aim::Prompt => ((cx - 330.0, h - 360.0), Some((cx - 250.0, h - 250.0))),
            Aim::Card => {
                let x = self.card.as_ref().map_or(cx + 100.0, |c| c.x);
                let right = x + 440.0 < w - 40.0;
                let hover = if right {
                    (x + 440.0, cy - 110.0)
                } else {
                    (x - 50.0, cy - 110.0)
                };
                (hover, Some((x + 30.0, cy - 60.0)))
            }
            Aim::Ring => {
                let r = self.reticle_radius();
                // Above and to the left, clear of a card either side.
                ((cx - r - 30.0, cy - r - 80.0), Some((cx, cy)))
            }
            Aim::Find(i) => {
                let now = self.sky_now(self.last_real);
                let hz = westering_core::coords::horizon(self.observer, now);
                let prec = westering_core::coords::precession(now);
                match self
                    .find_dir(i, now, &hz, &prec)
                    .and_then(|v| self.camera.project(v))
                {
                    Some((x, y)) if self.camera.on_screen(x, y, -40.0) => {
                        (beside(x, y), Some((x, y)))
                    }
                    _ => ((cx - 120.0, cy - 60.0), Some((cx, cy))),
                }
            }
            Aim::Sky(ra, dec) => {
                let now = self.sky_now(self.last_real);
                let hz = westering_core::coords::horizon(self.observer, now);
                let prec = westering_core::coords::precession(now);
                match self.camera.project(apply(&hz, apply(&prec, unit(ra, dec)))) {
                    // Above it, clear of a card beside the middle.
                    Some((x, y)) if self.camera.on_screen(x, y, -40.0) => (
                        ((x + 20.0).clamp(60.0, w - 60.0), (y - 95.0).max(60.0)),
                        Some((x, y)),
                    ),
                    _ => ((cx - 120.0, cy - 60.0), None),
                }
            }
            Aim::Moon(f) => match self.moon_spot(f) {
                Some((x, y, _)) => {
                    // Hover just off the place, towards the middle of the disc.
                    let (dx, dy) = (cx - x, cy - y);
                    let l = (dx * dx + dy * dy).sqrt().max(1.0);
                    let hover = if l > 90.0 {
                        (x + dx / l * 70.0, y + dy / l * 70.0 - 20.0)
                    } else {
                        (x - 70.0, y - 40.0)
                    };
                    (hover, Some((x, y)))
                }
                None => ((cx - 120.0, cy - 60.0), None),
            },
            Aim::Weights => {
                let now = self.sky_now(self.last_real);
                let hz = westering_core::coords::horizon(self.observer, now);
                let spot = self
                    .page
                    .weights
                    .iter()
                    .filter_map(|wt| self.camera.project(apply(&hz, unit(wt.ra, wt.dec))))
                    .find(|&(x, y)| self.camera.on_screen(x, y, -40.0));
                match spot {
                    Some((x, y)) => (beside(x, y), Some((x, y))),
                    None => ((cx, cy - 80.0), None),
                }
            }
        })
    }

    pub(crate) fn guide_frame(
        &mut self,
        real: UnixMs,
        brightness: f64,
    ) -> (Vec<Sprite>, Option<Bubble>, Vec<Point>) {
        let dt = ((real - self.guide.last_frame) as f64 / 1000.0).clamp(0.0, 0.1);
        self.guide.last_frame = real;
        let (nx, ny, nw, nh) = self.nook();
        let alpha = if matches!(self.session.phase(), Phase::LightsOut | Phase::Over) {
            brightness / westering_core::session::DIM
        } else {
            brightness.max(0.6)
        };

        // The line to show, moving the queue along.
        let finished = self
            .guide
            .line
            .as_ref()
            .is_none_or(|l| real - l.shown > l.hold + 1_400);
        // A line that waited too long has probably stopped being true.
        self.guide.queue.retain(|l| real - l.shown < 8_000);
        if finished && !self.guide.queue.is_empty() {
            let next = self.guide.queue.remove(0);
            if next.shown <= real {
                self.guide.line = Some(next);
            } else {
                self.guide.queue.insert(0, next);
            }
        }
        let speaking = self
            .guide
            .line
            .as_ref()
            .filter(|l| real >= l.shown && real - l.shown < l.hold + 900)
            .map(|l| l.aim);
        if let Some(aim) = speaking {
            if aim != Aim::Stay && aim != self.guide.aim {
                self.guide.aim = aim;
                self.guide.pointing_since = None;
            }
            self.guide.busy_until = real + LINGER_MS;
        }

        // Nothing to say for a while: home, with a little flight now and then.
        if real > self.guide.busy_until && self.guide.aim != Aim::Home {
            self.guide.aim = Aim::Home;
            self.guide.pointing_since = None;
        }
        if self.guide.aim == Aim::Home
            && self.hunting()
            && self.card.is_none()
            && self.talk.prompt.is_none()
            && real > self.guide.next_wander
        {
            let (w, h) = (self.camera.width, self.camera.height);
            let pick = (real % 997) as f64 / 997.0;
            self.guide.wander = Some((
                w * (0.08 + 0.2 * pick),
                h * (0.45 + 0.2 * (1.0 - pick)),
                real + 3_500,
            ));
            self.guide.next_wander = real + 30_000 + (real % 20_000);
        }
        let wandering = self.guide.wander.filter(|&(_, _, until)| real < until);
        if wandering.is_none() {
            self.guide.wander = None;
        }

        // With calm motion on, the wisp keeps to its moss and speaks from there.
        let aim = if self.calm() {
            Aim::Home
        } else {
            self.guide.aim
        };
        let wandering = if self.calm() { None } else { wandering };
        let (goal, pointing) = match (self.resolve(aim), wandering) {
            (Some(_), Some((wx, wy, _))) if aim == Aim::Home => ((wx, wy), None),
            (Some(g), _) => g,
            (None, _) => ((self.guide.flight.x, self.guide.flight.y), None),
        };
        let home = self.home_spot();
        if self.guide.flight.x < -50.0 {
            self.guide.flight.place(home.0, home.1);
        }
        let at_home_goal = (goal.0 - home.0).abs() < 1.0 && (goal.1 - home.1).abs() < 1.0;
        // Pointed at from beside, with a soft ring, never by flying round it.
        let circle = false;
        self.guide
            .flight
            .step(real, dt, goal, pointing, circle, !at_home_goal);
        let settled = at_home_goal
            && ((self.guide.flight.x - home.0).powi(2) + (self.guide.flight.y - home.1).powi(2))
                .sqrt()
                < 3.0
            && self.guide.flight.speed() < 25.0;
        if settled {
            self.guide.flight.place(home.0, home.1);
        }
        self.guide.flight.home = settled;
        let near_goal = ((self.guide.flight.x - goal.0).powi(2)
            + (self.guide.flight.y - goal.1).powi(2))
        .sqrt()
            < 60.0;
        if pointing.is_some() && near_goal && self.guide.pointing_since.is_none() {
            self.guide.pointing_since = Some(real);
        }

        let (fx, fy) = (self.guide.flight.x, self.guide.flight.y);
        let look = pointing.map(|(px, _)| ((px - fx) / 120.0).clamp(-1.0, 1.0));
        let mut sprites = Vec::new();
        let mut points = self.guide.flight.embers(real, alpha as f32);
        // The moss stays put; the wisp is drawn the same way whether it sits
        // on it or flies, so going home has no seam.
        if let Some(texture) = render_moss(&self.guide.wisp, real as f64, self.guide.scale) {
            sprites.push(Sprite {
                texture,
                x: nx,
                y: ny,
                width: nw,
                height: nh,
                alpha,
            });
        }
        let lean = if settled {
            0.0
        } else {
            self.guide.flight.lean()
        };
        let look = if settled {
            look
        } else {
            look.or(Some(lean * 0.8))
        };
        self.guide.wisp.set_flight(true, lean, look);
        if let Some(texture) = render_flying(&mut self.guide.wisp, real as f64, self.guide.scale) {
            let (sw, sh) = (FLYING.0 * SIZE, FLYING.1 * SIZE);
            sprites.push(Sprite {
                texture,
                x: fx - sw / 2.0,
                y: fy - sh / 2.0,
                width: sw,
                height: sh,
                alpha,
            });
        }

        // A soft glow round what it's pointing at, for a moment.
        if let (Some(since), Some((px, py))) = (self.guide.pointing_since, pointing) {
            let age = (real - since) as f64;
            if age < 2_600.0 {
                let a = (1.0 - age / 2_600.0) as f32;
                let r = 22.0 + 6.0 * (age / 400.0).sin();
                for k in 0..16 {
                    let t = k as f64 / 16.0 * std::f64::consts::TAU + age / 900.0;
                    points.push(Point {
                        x: px + r * t.cos(),
                        y: py + r * t.sin(),
                        radius: 1.4,
                        color: [1.0, 0.82, 0.55],
                        alpha: 0.7 * a * alpha as f32,
                        halo: 0.6,
                    });
                }
            }
        }

        // More waiting to be said: dots in the bubble, or over the wisp
        // between lines, so a pause doesn't read as the end.
        let more = !self.guide.queue.is_empty() || self.talk.pending.is_some();
        let between = more
            && self
                .guide
                .line
                .as_ref()
                .is_none_or(|l| real - l.shown > l.hold + 900);
        if between {
            let t = real as f64 / 1000.0;
            for k in 0..3 {
                let phase =
                    ((t * 1.6 - k as f64 * 0.22).rem_euclid(1.0) * std::f64::consts::TAU).sin();
                points.push(Point {
                    x: fx - 8.0 + k as f64 * 8.0,
                    y: fy - 38.0,
                    radius: 2.0,
                    color: [1.0, 0.9, 0.72],
                    alpha: (0.35 + 0.5 * phase.max(0.0)) as f32 * alpha as f32,
                    halo: 0.3,
                });
            }
        }
        let bubble = self.guide.line.as_ref().and_then(|l| {
            let a = envelope(real - l.shown, 500, l.hold, 900);
            if a <= 0.0 {
                return None;
            }
            let width = 300.0;
            let w = self.camera.width;
            let (x, bottom, tail) = if settled {
                (nx + 26.0, ny + 14.0, true)
            } else {
                let right = fx + 36.0 + width + 28.0 < w - 10.0;
                let x = if right {
                    fx + 36.0
                } else {
                    fx - 36.0 - width - 28.0
                };
                (x.max(10.0), (fy - 26.0).max(150.0), false)
            };
            Some(Bubble {
                x,
                bottom,
                text: l.text.clone(),
                width,
                alpha: a * alpha.min(1.0),
                tail,
                more: more.then_some(real as f64 / 1000.0),
            })
        });
        (sprites, bubble, points)
    }
}
