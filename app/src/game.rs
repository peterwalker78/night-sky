//! One visit: the real sky drawn as dots, the hunt, and the ending. Owns the
//! session, the camera and the dot field, takes input, and turns each tick
//! into a frame for the view.

use crate::camera::Camera;
use crate::field::{Field, Rgb, noise3};
use crate::view::{Frame, Text};
use gtk::gdk;
use gtk::gdk::prelude::TextureExt;
use westering_core::catalogues::Kind;
use westering_core::coords::{
    Mat3, Observer, Vec3, alt_az, apply, from_alt_az, horizon, precession, refraction, unit,
};
use westering_core::ephem::{Body, moon_age, moon_phase_name};
use westering_core::finale::{Handoff, handoff};
use westering_core::finds::{Find, Target, night_key, night_of, tonight};
use westering_core::journal::{Journal, Night};
use westering_core::session::{Phase, Session, Timings};
use westering_core::sky::{Sky, limiting_magnitude, see};
use westering_core::time::{MONTHS, UnixMs, civil_date, weekday};

/// One line of the Tonight list.
#[derive(Clone, Debug, PartialEq)]
pub struct Row {
    pub name: String,
    pub kind: &'static str,
    pub whereabouts: String,
    pub found: bool,
}

/// Converts the real clock to the sky's: normally the same, but the feel lab
/// can start the sky at another moment and run it faster.
pub struct Clock {
    pub real0: UnixMs,
    pub sky0: UnixMs,
    pub speed: f64,
}

impl Clock {
    pub fn sky(&self, real: UnixMs) -> UnixMs {
        self.sky0 + ((real - self.real0) as f64 * self.speed) as UnixMs
    }
}

pub struct Options {
    pub clock: Clock,
    pub observer: Observer,
    pub offset_s: i32,
    pub timings: Timings,
    pub journal: Journal,
}

#[derive(Default)]
pub(crate) struct Held {
    pub(crate) space: bool,
    pub(crate) left: bool,
    pub(crate) right: bool,
    pub(crate) up: bool,
    pub(crate) down: bool,
    pub(crate) zoom_in: bool,
    pub(crate) zoom_out: bool,
    pub(crate) fast: bool,
}

/// Easing the view towards somewhere.
#[derive(Clone, Copy)]
pub(crate) struct Look {
    pub(crate) az: f64,
    pub(crate) alt: f64,
    pub(crate) fov: f64,
    /// Per second, 0 to 1: how much of the gap closes.
    pub(crate) rate: f64,
}

#[derive(Default)]
pub(crate) struct Catch {
    pub(crate) target: Option<usize>,
    pub(crate) holding: bool,
    pub(crate) progress: f64,
    /// The field of view before the zoom, to go back to.
    pub(crate) fov_before: Option<f64>,
}

pub(crate) struct Card {
    pub(crate) x: f64,
    pub(crate) kicker: String,
    pub(crate) title: String,
    pub(crate) body: String,
    pub(crate) shown: UnixMs,
}

pub(crate) struct Timed {
    pub(crate) text: String,
    pub(crate) shown: UnixMs,
    pub(crate) hold: UnixMs,
}

pub(crate) struct Meteor {
    start: UnixMs,
    duration: UnixMs,
    from: Vec3,
    toward: Vec3,
    length: f64,
    brightness: f32,
}

/// What the slow-changing base layer was last drawn for.
#[derive(Clone, Copy, PartialEq)]
pub(crate) struct BaseStamp {
    az: f64,
    alt: f64,
    fov: f64,
    width: f64,
    height: f64,
    sky_minute: i64,
    lapse: bool,
}

/// A star with everything that doesn't change during a visit worked out.
pub(crate) struct Prepared {
    dir: Vec3,
    mag: f64,
    light: f32,
    tint: Rgb,
    rate: f64,
    phase: f64,
}

pub struct Game {
    pub(crate) sky: Sky,
    pub(crate) prepared: Vec<Prepared>,
    pub(crate) observer: Observer,
    pub(crate) clock: Clock,
    pub(crate) offset_s: i32,
    pub(crate) night: String,
    pub(crate) session: Session,
    pub(crate) finds: Vec<Find>,
    pub(crate) caught: Vec<bool>,
    pub(crate) journal: Journal,
    pub(crate) page: Night,
    pub(crate) camera: Camera,
    pub(crate) look: Option<Look>,
    pub(crate) field: Field,
    pub(crate) base: Option<BaseStamp>,
    pub(crate) star_dirs: Vec<Vec3>,
    pub(crate) held: Held,
    pub(crate) pan: (f64, f64),
    pub(crate) drag_from: Option<(f64, f64, f64, f64)>,
    pub(crate) catch: Catch,
    pub(crate) card: Option<Card>,
    pub(crate) caption: Option<Timed>,
    pub(crate) hint: Option<Timed>,
    pub(crate) meteors: Vec<Meteor>,
    pub(crate) next_meteor: UnixMs,
    pub(crate) handoff: Option<Handoff>,
    pub(crate) finale_turned: bool,
    pub(crate) last_input: UnixMs,
    /// Key releases wait a moment: X11's auto-repeat sends a release before
    /// every repeated press, and a real release has no press behind it.
    pub(crate) releases: Vec<(gdk::Key, UnixMs)>,
    pub(crate) esc_armed: UnixMs,
    pub(crate) last_real: UnixMs,
    pub(crate) rng: u64,
    pub quit: bool,
    pub(crate) talk: crate::talk::Talk,
    pub(crate) drawing: Option<crate::drawing::Drawing>,
    pub(crate) reveal: Option<(usize, UnixMs)>,
    /// Something the window should open: the logbook, a course, settings.
    pub(crate) request: Option<crate::talk::Request>,
    pub(crate) guide: crate::guide::Guide,
    pub(crate) points: Vec<crate::view::Point>,
    /// Where Jupiter's moons were drawn this frame, to name them.
    moon_labels: Vec<(f64, f64, &'static str, f32)>,
    pub(crate) photos: crate::eyepiece::Photos,
    /// A find the view keeps centred as the sky turns, like a telescope's drive.
    pub(crate) track: Option<usize>,
    /// The find the eyepiece shows, and how far it has faded in.
    eye: Option<usize>,
    eye_alpha: f64,
    /// How many steps of each hop have been found.
    pub(crate) steps: Vec<usize>,
    /// A find being told a card at a time.
    pub(crate) tour: Option<crate::tour::Tour>,
    /// Tonight's places on the Moon, by index into the features.
    pub(crate) moon_stops: Vec<usize>,
    /// Algol's place among the prepared stars: it dims now and then.
    algol: Option<usize>,
    /// Points drawn over the photographs.
    pub(crate) marks: Vec<crate::view::Point>,
    /// The star pattern being shown, and how far it has faded in.
    pub(crate) pattern: Option<Vec<(u16, u16)>>,
    pub(crate) pattern_alpha: f64,
}

pub(crate) const WARM: Rgb = [1.0, 0.86, 0.66];
const RETICLE: Rgb = [0.78, 0.84, 1.0];

pub(crate) fn smoothstep(x: f64) -> f64 {
    let x = x.clamp(0.0, 1.0);
    x * x * (3.0 - 2.0 * x)
}

/// Fades in over `rise`, holds, and fades out over `fall`.
pub(crate) fn envelope(age: UnixMs, rise: UnixMs, hold: UnixMs, fall: UnixMs) -> f64 {
    if age < 0 {
        0.0
    } else if age < rise {
        smoothstep(age as f64 / rise as f64)
    } else if age < rise + hold {
        1.0
    } else {
        1.0 - smoothstep((age - rise - hold) as f64 / fall.max(1) as f64)
    }
}

/// The gentle hills along the horizon, in degrees of altitude.
pub(crate) fn hills(az: f64) -> f64 {
    let a = az.to_radians();
    (0.75
        + 0.45 * (3.0 * a + 0.7).sin()
        + 0.3 * (7.0 * a + 2.1).sin()
        + 0.18 * (13.0 * a + 0.3).sin())
    .max(0.15)
}

/// How many magnitudes fainter the view reaches when zoomed in to `fov`,
/// the way a telescope gathers more light than the eye.
pub(crate) fn gather(fov: f64) -> f64 {
    2.5 * (60.0 / fov).clamp(1.0, 100.0).log10()
}

/// Stars brighter than this are drawn as true points; fainter ones on the lattice.
const BRIGHT: f64 = 3.0;

/// A bright star or planet as a crisp point with a halo, sized by its light.
fn point(x: f64, y: f64, color: Rgb, amount: f32, halo: f32) -> crate::view::Point {
    crate::view::Point {
        x,
        y,
        radius: (1.05 + 0.62 * amount).min(4.4),
        color,
        alpha: (0.5 + 0.35 * amount).min(1.0),
        halo: (0.25 + amount / 2.2).min(1.0) * halo,
    }
}

/// How much light a star of magnitude `mag` puts on its dot.
fn light(mag: f64) -> f32 {
    (1.6 * 10f64.powf(-0.4 * (mag - 1.0)).powf(0.55)) as f32
}

pub(crate) fn transpose(m: &Mat3) -> Mat3 {
    [
        [m[0][0], m[1][0], m[2][0]],
        [m[0][1], m[1][1], m[2][1]],
        [m[0][2], m[1][2], m[2][2]],
    ]
}

pub(crate) fn dot3(a: Vec3, b: Vec3) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

pub(crate) fn angle_between(a: Vec3, b: Vec3) -> f64 {
    dot3(a, b).clamp(-1.0, 1.0).acos().to_degrees()
}

/// Shortest signed turn from one azimuth to another.
pub(crate) fn turn(from: f64, to: f64) -> f64 {
    (to - from + 540.0).rem_euclid(360.0) - 180.0
}

impl Game {
    pub fn new(options: Options, real_now: UnixMs) -> Game {
        let sky = Sky::bundled();
        let Options {
            clock,
            observer,
            offset_s,
            timings,
            journal,
        } = options;
        let now = clock.sky(real_now);
        let night = night_key(night_of(now, offset_s));
        let finds = tonight(&sky, observer, now, offset_s, &|id| {
            journal.times_found_before(id, &night)
        });
        let caught: Vec<bool> = finds
            .iter()
            .map(|f| journal.found_on(&f.id, &night))
            .collect();
        let still = caught.iter().filter(|c| !**c).count();
        let mut session = Session::new(real_now, timings, still, true);
        if still == 0 {
            session.found_one(real_now);
        }
        let page = journal.night(&night).unwrap_or_else(|| Night {
            key: night.clone(),
            ..Night::default()
        });
        let talk = crate::talk::Talk::new(
            &journal,
            &night,
            crate::talk::calendar(&sky.lists.showers, now),
        );
        let already = !finds.is_empty() && caught.iter().all(|c| *c);
        let star_dirs = sky.stars.precessed(&precession(now));
        let prepared: Vec<Prepared> = sky
            .stars
            .stars
            .iter()
            .zip(&star_dirs)
            .take_while(|(s, _)| s.mag <= 6.2)
            .map(|(s, &dir)| Prepared {
                dir,
                mag: s.mag as f64,
                light: light(s.mag as f64),
                tint: westering_core::stars::tint(s.bv),
                rate: 0.9 + (s.hr % 7) as f64 * 0.23,
                phase: s.hr as f64,
            })
            .collect();
        let moon_stops = westering_core::tours::moon_stops(
            &sky.tours.moon,
            moon_age(now) * westering_core::tours::SYNODIC_DAYS,
            night_of(now, offset_s),
            &|name| journal.found_before(&format!("moon:{name}"), &night),
        );
        let algol = sky
            .stars
            .stars
            .iter()
            .take(prepared.len())
            .position(|s| s.hr == 936);
        let steps = vec![0; finds.len()];
        let mut game = Game {
            sky,
            prepared,
            observer,
            clock,
            offset_s,
            night,
            session,
            finds,
            caught,
            journal,
            page,
            camera: Camera::new(180.0, 30.0, 95.0),
            look: None,
            field: Field::new(),
            base: None,
            star_dirs,
            held: Held::default(),
            pan: (0.0, 0.0),
            drag_from: None,
            catch: Catch::default(),
            card: None,
            caption: None,
            hint: None,
            meteors: Vec::new(),
            next_meteor: real_now + 8_000,
            handoff: None,
            finale_turned: false,
            last_input: real_now,
            releases: Vec::new(),
            esc_armed: 0,
            last_real: real_now,
            rng: (real_now as u64) ^ 0x9e37_79b9_7f4a_7c15,
            quit: false,
            talk,
            drawing: None,
            reveal: None,
            request: already.then_some(crate::talk::Request::Book(None)),
            guide: crate::guide::Guide::new(real_now),
            points: Vec::new(),
            photos: crate::eyepiece::Photos::load(),
            moon_labels: Vec::new(),
            track: None,
            eye: None,
            eye_alpha: 0.0,
            steps,
            tour: None,
            moon_stops,
            algol,
            marks: Vec::new(),
            pattern: None,
            pattern_alpha: 0.0,
        };
        game.arrive(real_now);
        game
    }

    fn random(&mut self) -> f64 {
        self.rng ^= self.rng << 13;
        self.rng ^= self.rng >> 7;
        self.rng ^= self.rng << 17;
        (self.rng >> 11) as f64 / (1u64 << 53) as f64
    }

    pub(crate) fn sky_now(&self, real: UnixMs) -> UnixMs {
        self.clock.sky(real) + self.session.lapse(real)
    }

    /// The first view and the line that names the night.
    fn arrive(&mut self, real: UnixMs) {
        let now = self.clock.sky(real);
        let first = self
            .finds
            .iter()
            .enumerate()
            .find(|(i, f)| !self.caught[*i] && matches!(f.target, Target::Body(_)))
            .map(|(i, _)| i);
        let (az, alt) = first
            .and_then(|i| self.find_dir(i, now, &horizon(self.observer, now), &precession(now)))
            .map(|v| {
                let (alt, az) = alt_az(v);
                (az, alt.clamp(15.0, 62.0))
            })
            .unwrap_or((180.0, 30.0));
        self.camera.az = az;
        self.camera.alt = alt;
        self.camera.update();

        let (_, month, day) = civil_date(now, self.offset_s);
        let mut line = format!(
            "{} {} {}",
            weekday(now, self.offset_s),
            day,
            MONTHS[month as usize - 1]
        );
        let moon = moon_phase_name(moon_age(now));
        self.page.moon = moon.to_owned();
        if see(Body::Moon, self.observer, now).alt > 0.0 {
            line.push_str(&format!(" · {moon}"));
        }
        let planets: Vec<&str> = self
            .finds
            .iter()
            .filter_map(|f| match f.target {
                Target::Body(b) if b != Body::Moon => Some(b.name()),
                _ => None,
            })
            .collect();
        match planets.len() {
            0 => {}
            1 => line.push_str(&format!(" · {} is up", planets[0])),
            n => line.push_str(&format!(
                " · {} and {} are up",
                planets[..n - 1].join(", "),
                planets[n - 1]
            )),
        }
        if self.caught.iter().all(|c| *c) && !self.finds.is_empty() {
            line.push_str(" · you've found tonight's sky");
        }
        if let Some(extra) = self.arrival_extra(now) {
            line.push('\n');
            line.push_str(&extra);
        }
        self.caption = Some(Timed {
            text: line,
            shown: real + 900,
            hold: 8_000,
        });
        self.guide_arrival(real);
    }

    /// Where find `i` is, as a horizon vector.
    pub(crate) fn find_dir(&self, i: usize, now: UnixMs, hz: &Mat3, prec: &Mat3) -> Option<Vec3> {
        match self.finds[i].target {
            Target::Body(body) => {
                let s = see(body, self.observer, now);
                Some(from_alt_az(s.alt, s.az))
            }
            Target::Showpiece(p) => {
                let piece = &self.sky.lists.showpieces[p];
                Some(apply(hz, apply(prec, unit(piece.ra, piece.dec))))
            }
            Target::Star(hr) => self
                .sky
                .stars
                .index_of(hr)
                .map(|idx| apply(hz, self.star_dirs[idx])),
            Target::Meteor(_) => None,
            Target::Figure(f) => self.sky.figures[f]
                .centre(&self.sky.stars)
                .map(|(c, _)| apply(hz, apply(prec, c))),
            Target::Hop(_) => self.stop_dir(self.hop_stop(i)?, hz, prec),
            Target::Story(s) => {
                let anchor = self.sky.tours.stories[s].anchor;
                self.sky
                    .stars
                    .index_of(anchor)
                    .map(|idx| apply(hz, self.star_dirs[idx]))
            }
            Target::MoonWalk => {
                let s = see(Body::Moon, self.observer, now);
                Some(from_alt_az(s.alt, s.az))
            }
        }
    }

    /// What kind of thing find `i` is, in a word or two.
    pub(crate) fn kind_word(&self, i: usize) -> &'static str {
        match self.finds[i].target {
            Target::Body(Body::Moon) => "Our Moon",
            Target::Body(_) => "Planet",
            Target::Star(_) => "Star",
            Target::Showpiece(p) => match self.sky.lists.showpieces[p].kind {
                Kind::Cluster => "Star cluster",
                Kind::Galaxy => "Galaxy",
                Kind::Nebula => "Nebula",
                Kind::Double => "Double star",
                Kind::Star => "Star",
                Kind::Dark => "Dark cloud",
            },
            Target::Meteor(_) => "Meteor",
            Target::Figure(_) => "Constellation",
            Target::Hop(_) => "Star-hop",
            Target::Story(_) => "Tonight's story",
            Target::MoonWalk => "Moon walk",
        }
    }

    /// What to look out for, to help find it.
    pub(crate) fn look_for(&self, i: usize) -> &'static str {
        match self.finds[i].target {
            Target::Body(Body::Moon) => "You can't miss it.",
            Target::Body(_) => "Look for a bright, steady light that doesn't twinkle.",
            Target::Star(_) => "Look for a single bright star.",
            Target::Showpiece(p) if self.sky.lists.showpieces[p].deep => {
                "Too faint for the eye alone: turn to it and zoom in close with +, and it will show."
            }
            Target::Showpiece(p) => match self.sky.lists.showpieces[p].kind {
                Kind::Cluster => "Look for a little knot of faint stars.",
                Kind::Galaxy | Kind::Nebula => "Look for a faint smudge of light.",
                Kind::Double => "It looks like one star; close up it's two.",
                _ => "Look for a single star.",
            },
            Target::Meteor(_) => "Watch the sky, and press Space the moment one flies.",
            Target::Figure(_) => "Look for its shape; put the ring in the middle of it.",
            Target::Hop(_) => "Start from a star you know, and hop from star to star.",
            Target::Story(_) => "It starts at this star: hold Space when it's in the ring.",
            Target::MoonWalk if self.moon_find().is_some_and(|m| !self.caught[m]) => {
                "Catch the Moon first, then hold Space on it again to go closer."
            }
            Target::MoonWalk => "Hold Space on the Moon to go closer.",
        }
    }

    /// How narrow the view goes to show a find close up.
    fn zoom_for(&self, i: usize) -> f64 {
        match self.finds[i].target {
            // Close enough for the disc to fill about a quarter of the view.
            Target::Body(body) => {
                let d = see(body, self.observer, self.clock.sky(self.last_real))
                    .position
                    .diameter
                    / 3600.0;
                // Saturn is framed by its rings, 2.27 times as wide as the planet;
                // Jupiter wide enough to show its inner moons beside it.
                let share = match body {
                    Body::Saturn => 0.24 / 2.27,
                    Body::Jupiter => 0.07,
                    _ => 0.24,
                };
                (d / share).clamp(crate::camera::FOV_NARROWEST, 2.4)
            }
            Target::Showpiece(p) => {
                let id = &self.sky.lists.showpieces[p].id;
                match self.photos.credit(id).and_then(|c| c.view) {
                    Some(view) => view,
                    None => (self.photo_degrees(p) * 2.6).clamp(0.4, 40.0),
                }
            }
            Target::Star(_) => 8.0,
            Target::Meteor(_) => self.camera.fov,
            Target::Figure(f) => self.sky.figures[f]
                .centre(&self.sky.stars)
                .map_or(60.0, |(_, reach)| (reach * 3.0).clamp(20.0, 100.0)),
            // A hop keeps the view wide enough to see the next step.
            Target::Hop(_) => self.catch.fov_before.unwrap_or(self.camera.fov),
            Target::Story(_) => 50.0,
            Target::MoonWalk => self.moon_walk_fov(),
        }
    }

    /// The id of a photograph of find `i`, if there is one.
    pub(crate) fn photo_id(&self, i: usize) -> Option<String> {
        let id = match self.finds[i].target {
            Target::Body(b) => b.id().to_owned(),
            Target::Showpiece(p) => self.sky.lists.showpieces[p].id.clone(),
            _ => return None,
        };
        self.photos.credit(&id).map(|_| id)
    }

    /// How wide a showpiece's photograph is on the sky, in degrees.
    fn photo_degrees(&self, p: usize) -> f64 {
        let piece = &self.sky.lists.showpieces[p];
        if let Some(d) = self.photos.credit(&piece.id).and_then(|c| c.degrees) {
            return d;
        }
        match piece.kind {
            // Pairs and single stars are pictured at a small telescope's scale.
            Kind::Double | Kind::Star => 0.3,
            _ => (piece.size / 60.0).max(0.1) / 0.8,
        }
    }

    /// Half the width of find `i`'s photograph on the screen, at a field of view.
    pub(crate) fn photo_half(&self, i: usize, fov: f64) -> f64 {
        let ppd = (self.camera.width / 2.0) / (2.0 * (fov.to_radians() / 4.0).tan())
            * std::f64::consts::PI
            / 180.0;
        let degrees = match self.finds[i].target {
            Target::Body(body) => {
                // Saturn's picture is framed by its rings, 2.27 times the
                // planet's width: the planet fills 38 per cent of it.
                let share = if body == Body::Saturn {
                    0.38
                } else {
                    crate::eyepiece::DISC
                };
                see(body, self.observer, self.clock.sky(self.last_real))
                    .position
                    .diameter
                    / 3600.0
                    / share
            }
            Target::Showpiece(p) => self.photo_degrees(p),
            _ => 0.0,
        };
        degrees / 2.0 * ppd
    }

    /// How big find `i` looks, in pixels, at a field of view.
    fn apparent_radius(&self, i: usize, fov: f64) -> f64 {
        if let Target::MoonWalk = self.finds[i].target {
            return self
                .moon_find()
                .map_or(0.0, |m| self.apparent_radius(m, fov));
        }
        if self.caught[i] && self.photo_id(i).is_some() {
            // Keep the card on the screen beside a picture that fills the view.
            return self
                .photo_half(i, fov)
                .min(self.camera.width.min(self.camera.height) * 0.3);
        }
        let ppd = (self.camera.width / 2.0) / (2.0 * (fov.to_radians() / 4.0).tan())
            * std::f64::consts::PI
            / 180.0;
        let degrees = match self.finds[i].target {
            Target::Body(body) => {
                see(body, self.observer, self.clock.sky(self.last_real))
                    .position
                    .diameter
                    / 7200.0
            }
            Target::Showpiece(p) => self.sky.lists.showpieces[p].size / 120.0,
            Target::Figure(f) => self.sky.figures[f]
                .centre(&self.sky.stars)
                .map_or(0.0, |(_, reach)| reach),
            _ => 0.0,
        };
        degrees * ppd
    }

    /// Where the words about something go: clear of it, to its right.
    pub(crate) fn beside_centre(&self) -> f64 {
        self.beside(None, self.camera.fov)
    }

    fn beside(&self, i: Option<usize>, fov: f64) -> f64 {
        const CARD: f64 = 400.0;
        let r = self.reticle_radius();
        let object = i.map(|i| self.apparent_radius(i, fov)).unwrap_or(0.0);
        // The Tonight list takes the right-hand edge during the hunt, except
        // while a photograph is up.
        let photo = i.is_some_and(|i| {
            self.photo_id(i).is_some() || self.finds[i].target == Target::MoonWalk
        });
        let list = if photo { 0.0 } else { 300.0 };
        let clear = r.max(object) + 34.0;
        let (w, cx) = (self.camera.width, self.camera.width / 2.0);
        let right = cx + clear;
        if right + CARD <= w - list {
            right
        } else {
            (cx - clear - CARD).max(24.0)
        }
    }

    pub(crate) fn reticle_radius(&self) -> f64 {
        (self.camera.width.min(self.camera.height) * 0.055).max(26.0)
    }

    pub fn resize(&mut self, width: f64, height: f64, scale: f64) {
        self.camera.set_size(width, height);
        self.guide.scale = scale;
    }

    fn input(&mut self, real: UnixMs) {
        self.last_input = real;
    }

    pub(crate) fn hunting(&self) -> bool {
        self.session.phase() == Phase::Hunt
    }

    pub fn key_pressed(&mut self, key: gdk::Key, real: UnixMs) -> bool {
        self.input(real);
        if self.typing() {
            return false;
        }
        self.releases.retain(|(k, _)| *k != key);
        if key == gdk::Key::space {
            if self.held.space {
                return true;
            }
            self.held.space = true;
        }
        let phase = self.session.phase();
        if matches!(phase, Phase::Finale | Phase::LightsOut | Phase::Over)
            && matches!(key, gdk::Key::Escape | gdk::Key::q | gdk::Key::Q)
        {
            self.quit = true;
            return true;
        }
        if self.drawing.is_some() && self.drawing_key(key, real) {
            return true;
        }
        if self.placing()
            && matches!(
                key,
                gdk::Key::Return | gdk::Key::KP_Enter | gdk::Key::Escape
            )
        {
            self.place_weight(real);
            return true;
        }
        if let Some(prompt) = &self.talk.prompt {
            let chips = prompt.chips.len();
            if key == gdk::Key::Escape {
                self.skip_prompt(real);
                return true;
            }
            if let Some(n) = key.to_unicode().and_then(|c| c.to_digit(10))
                && (1..=chips as u32).contains(&n)
            {
                self.answer_chip(n as usize - 1, real);
                return true;
            }
        }
        let arrow = matches!(
            key,
            gdk::Key::Left | gdk::Key::Right | gdk::Key::Up | gdk::Key::Down
        );
        if arrow && self.card.is_some() {
            // Looking away from a card puts it down.
            self.end_tour(real);
            self.dismiss_card(real);
        }
        match key {
            gdk::Key::Left => self.held.left = true,
            gdk::Key::Right => self.held.right = true,
            gdk::Key::Up => self.held.up = true,
            gdk::Key::Down => self.held.down = true,
            gdk::Key::question | gdk::Key::F1 => self.guide_help(real),
            gdk::Key::m | gdk::Key::M => {
                let quiet = !self.journal.settings.quiet;
                self.journal.settings.quiet = quiet;
                if let Err(e) = self.journal.save_settings() {
                    eprintln!("westering: couldn't save settings: {e}");
                }
                self.say(
                    if quiet {
                        "Music off."
                    } else {
                        "Music back on."
                    },
                    real,
                    2_500,
                );
            }
            gdk::Key::c | gdk::Key::C => {
                if self.hunting() && self.card.is_none() && self.talk.prompt.is_none() {
                    self.start_drawing(real);
                }
            }
            gdk::Key::l | gdk::Key::L => {
                if !matches!(phase, Phase::Finale | Phase::LightsOut | Phase::Over) {
                    self.request = Some(crate::talk::Request::Book(Some(self.night.clone())));
                }
            }
            gdk::Key::plus | gdk::Key::equal | gdk::Key::KP_Add => self.held.zoom_in = true,
            gdk::Key::minus | gdk::Key::KP_Subtract => self.held.zoom_out = true,
            gdk::Key::Shift_L | gdk::Key::Shift_R => self.held.fast = true,
            gdk::Key::space => {
                if self.card.is_some() {
                    self.dismiss_card(real);
                } else if self.hunting() {
                    self.catch.holding = true;
                    if self.catch.target.is_none() {
                        self.try_meteor(real);
                    }
                }
            }
            gdk::Key::Return | gdk::Key::KP_Enter => {
                if self.card.is_some() {
                    self.dismiss_card(real);
                }
            }
            gdk::Key::Tab => {
                if self.hunting() {
                    self.turn_to_next(real);
                }
            }
            gdk::Key::Escape => {
                if self.card.is_some() {
                    self.end_tour(real);
                    self.dismiss_card(real);
                } else if matches!(phase, Phase::Arrival | Phase::Hunt) {
                    if real < self.esc_armed {
                        if let Some(p) = self.session.finish(real) {
                            self.entered(p, real);
                        }
                    } else {
                        self.esc_armed = real + 4_000;
                        self.hint = Some(Timed {
                            text: "Press Esc again to end tonight's sky".into(),
                            shown: real,
                            hold: 3_000,
                        });
                    }
                }
            }
            _ => return false,
        }
        true
    }

    pub fn key_released(&mut self, key: gdk::Key, real: UnixMs) {
        self.input(real);
        self.releases.push((key, real));
    }

    fn settle_releases(&mut self, real: UnixMs) {
        let due: Vec<gdk::Key> = self
            .releases
            .iter()
            .filter(|(_, at)| real - at > 45)
            .map(|(k, _)| *k)
            .collect();
        self.releases.retain(|(_, at)| real - at <= 45);
        for key in due {
            self.release(key);
        }
    }

    fn release(&mut self, key: gdk::Key) {
        match key {
            gdk::Key::Left => self.held.left = false,
            gdk::Key::Right => self.held.right = false,
            gdk::Key::Up => self.held.up = false,
            gdk::Key::Down => self.held.down = false,
            gdk::Key::plus | gdk::Key::equal | gdk::Key::KP_Add => self.held.zoom_in = false,
            gdk::Key::minus | gdk::Key::KP_Subtract => self.held.zoom_out = false,
            gdk::Key::Shift_L | gdk::Key::Shift_R => self.held.fast = false,
            gdk::Key::space => {
                self.held.space = false;
                self.catch.holding = false;
            }
            _ => {}
        }
    }

    pub fn drag_begin(&mut self, real: UnixMs) {
        self.input(real);
        self.look = None;
        self.drag_from = Some((self.camera.az, self.camera.alt, 0.0, 0.0));
    }

    pub fn drag_update(&mut self, dx: f64, dy: f64, real: UnixMs) {
        self.input(real);
        if let Some((az, alt, _, _)) = self.drag_from {
            let ppd = self.camera.px_per_degree();
            let cos_alt = self.camera.alt.to_radians().cos().max(0.2);
            self.camera.az = az - dx / ppd / cos_alt;
            self.camera.alt = alt + dy / ppd;
            self.camera.update();
        }
    }

    pub fn drag_end(&mut self) {
        self.drag_from = None;
    }

    pub fn scroll(&mut self, dy: f64, real: UnixMs) {
        self.input(real);
        if matches!(
            self.session.phase(),
            Phase::Hunt | Phase::Arrival | Phase::Dimming
        ) {
            self.look = None;
            self.camera.fov *= 1.12f64.powf(dy);
            self.camera.update();
        }
    }

    /// A click on something turns the view to it.
    pub fn click(&mut self, x: f64, y: f64, real: UnixMs) {
        self.input(real);
        if self.on_wisp(x, y) {
            self.guide_help(real);
            return;
        }
        if !self.hunting() {
            return;
        }
        let v = self.camera.unproject(x, y);
        let (alt, az) = alt_az(v);
        self.look = Some(Look {
            az,
            alt,
            fov: self.camera.fov,
            rate: 2.2,
        });
    }

    fn turn_to_next(&mut self, real: UnixMs) {
        let now = self.sky_now(real);
        let hz = horizon(self.observer, now);
        let prec = precession(now);
        let here = from_alt_az(self.camera.alt, self.camera.az);
        let next = (0..self.finds.len())
            .filter(|&i| !self.caught[i])
            .filter_map(|i| self.find_dir(i, now, &hz, &prec).map(|v| (i, v)))
            .filter(|(_, v)| alt_az(*v).0 > 0.0)
            .min_by(|a, b| {
                // Skip the one already under the reticle.
                let da = angle_between(here, a.1);
                let db = angle_between(here, b.1);
                let da = if da < 1.0 { 999.0 } else { da };
                let db = if db < 1.0 { 999.0 } else { db };
                da.total_cmp(&db)
            })
            .map(|(i, _)| i);
        match next {
            Some(i) => self.turn_to(i, real),
            None => {
                let meteor_left = (0..self.finds.len())
                    .any(|i| !self.caught[i] && matches!(self.finds[i].target, Target::Meteor(_)));
                if meteor_left {
                    self.guide_meteor_left(real);
                }
            }
        }
    }

    /// Turns the view towards find `i`, and says what to look for.
    pub fn turn_to(&mut self, i: usize, real: UnixMs) {
        if i >= self.finds.len() || !self.hunting() {
            return;
        }
        self.input(real);
        if self.card.is_some() {
            self.dismiss_card(real);
        }
        if let Target::Meteor(_) = self.finds[i].target {
            self.guide_meteor_left(real);
            return;
        }
        let now = self.sky_now(real);
        let hz = horizon(self.observer, now);
        let prec = precession(now);
        let Some(v) = self.find_dir(i, now, &hz, &prec) else {
            return;
        };
        let (alt, az) = alt_az(v);
        if alt < 0.0 {
            let name = self.finds[i].name.clone();
            self.say(
                format!("{name} has gone below the horizon for now."),
                real,
                5_000,
            );
            return;
        }
        let fov = match self.finds[i].target {
            _ if self.caught[i] && self.photo_id(i).is_some() => self.zoom_for(i),
            // Too faint for the eye: close enough in for it to show.
            Target::Showpiece(p) if self.sky.lists.showpieces[p].deep => {
                let limit = limiting_magnitude(see(Body::Sun, self.observer, now).alt);
                let short = self.sky.lists.showpieces[p].mag + 0.6 - limit;
                if short > 0.0 {
                    (60.0 / 10f64.powf(short / 2.5)).clamp(1.5, 60.0)
                } else {
                    self.camera.fov.max(60.0)
                }
            }
            Target::Figure(_) => self.zoom_for(i).max(self.camera.fov.min(90.0)),
            _ => self.camera.fov.max(60.0),
        };
        self.look = Some(Look {
            az,
            alt,
            fov,
            rate: 1.6,
        });
        self.track = Some(i);
        if !self.caught[i] {
            let hint = match self.hop_stop(i) {
                Some(stop) => stop.say.clone(),
                None => self.look_for(i).to_owned(),
            };
            let line = format!("{}. {hint}", self.finds[i].name);
            self.say_at(crate::guide::Aim::Find(i), line, real, 7_000);
        }
    }

    /// Tonight's finds for the list: what, where, and whether found.
    pub fn tonight_rows(&self, real: UnixMs) -> Vec<Row> {
        let now = self.sky_now(real);
        let hz = horizon(self.observer, now);
        let prec = precession(now);
        (0..self.finds.len())
            .map(|i| {
                let whereabouts = match self.finds[i].target {
                    Target::Meteor(_) => "anywhere, any moment".to_owned(),
                    _ => match self.find_dir(i, now, &hz, &prec).map(alt_az) {
                        Some((alt, _)) if alt < 0.0 => "below the horizon now".to_owned(),
                        Some((alt, az)) => westering_core::finale::whereabouts(alt, az),
                        None => String::new(),
                    },
                };
                let whereabouts = match self.finds[i].target {
                    Target::Hop(h) if self.steps[i] > 0 => format!(
                        "step {} of {}, {whereabouts}",
                        self.steps[i] + 1,
                        self.sky.tours.hops[h].steps.len()
                    ),
                    _ => whereabouts,
                };
                Row {
                    name: self.finds[i].name.clone(),
                    kind: self.kind_word(i),
                    whereabouts,
                    found: self.caught[i],
                }
            })
            .collect()
    }

    pub fn show_tonight(&self) -> bool {
        matches!(self.session.phase(), Phase::Hunt)
            && !self.finds.is_empty()
            && self.eye_alpha < 0.3
    }

    /// Which found thing's photograph to show: the one nearest the middle of
    /// the view, among those close enough to be worth a picture.
    fn eye_target(&self, now: UnixMs, hz: &Mat3, prec: &Mat3) -> Option<(usize, f64)> {
        if !matches!(self.session.phase(), Phase::Hunt | Phase::Dimming) || self.drawing.is_some() {
            return None;
        }
        let (cx, cy) = (self.camera.width / 2.0, self.camera.height / 2.0);
        (0..self.finds.len())
            .filter(|&i| self.caught[i] && self.photo_id(i).is_some())
            .filter_map(|i| {
                let (x, y) = self
                    .find_dir(i, now, hz, prec)
                    .and_then(|v| self.camera.project(v))?;
                let half = self.photo_half(i, self.camera.fov);
                let d = ((x - cx).powi(2) + (y - cy).powi(2)).sqrt();
                // Worth showing once the picture is big enough to hold detail:
                // a planet's disc sooner than a spread-out cluster.
                let size = match self.finds[i].target {
                    Target::Body(_) => smoothstep((half - 15.0) / 60.0),
                    _ => smoothstep((half - 40.0) / 160.0),
                };
                (size > 0.0 && d < half + self.camera.width * 0.5).then_some((i, size, d))
            })
            .min_by(|a, b| a.2.total_cmp(&b.2))
            .map(|(i, size, _)| (i, size))
    }

    /// Chooses the photograph for this frame and eases it in and out; the
    /// dots it replaces fade the other way.
    fn update_eye(&mut self, now: UnixMs, hz: &Mat3, prec: &Mat3, dt: f64) {
        let want = self.eye_target(now, hz, prec);
        if let Some((i, _)) = want
            && self.eye != Some(i)
            && self.eye_alpha < 0.05
        {
            self.eye = Some(i);
        }
        let target = match want {
            Some((i, size)) if self.eye == Some(i) => size,
            _ => 0.0,
        };
        let k = 1.0 - (-dt / 0.3).exp();
        self.eye_alpha += (target - self.eye_alpha) * k;
    }

    /// The photograph to draw this frame, if any.
    fn eyepiece(&mut self, now: UnixMs, hz: &Mat3, prec: &Mat3) -> Option<crate::view::Eyepiece> {
        if self.eye_alpha < 0.01 {
            return None;
        }
        let i = self.eye?;
        let id = self.photo_id(i)?;
        let v = self.find_dir(i, now, hz, prec)?;
        let cam = self.camera;
        let (x, y) = cam.project(v)?;
        // Which way celestial north points on the screen, at the object.
        let pole = apply(hz, [0.0, 0.0, 1.0]);
        let along = dot3(pole, v);
        let t = [
            pole[0] - along * v[0],
            pole[1] - along * v[1],
            pole[2] - along * v[2],
        ];
        let n = dot3(t, t).sqrt().max(1e-9);
        let nudge = [
            v[0] + t[0] / n * 1e-5,
            v[1] + t[1] / n * 1e-5,
            v[2] + t[2] / n * 1e-5,
        ];
        let (nx, ny) = cam
            .project(nudge)
            .map_or((0.0, -1.0), |(a, b)| (a - x, b - y));
        let north = nx.atan2(-ny).to_degrees();
        let credit = self.photos.credit(&id).cloned()?;
        let rotation = north - credit.north.unwrap_or(0.0);
        let phase = match self.finds[i].target {
            // The Moon and the inner planets wear tonight's phase.
            Target::Body(body @ (Body::Moon | Body::Mercury | Body::Venus)) => {
                let seen = see(body, self.observer, now);
                let sun = see(Body::Sun, self.observer, now);
                let sv = from_alt_az(sun.alt, sun.az);
                let along = dot3(sv, v);
                let t = [
                    sv[0] - along * v[0],
                    sv[1] - along * v[1],
                    sv[2] - along * v[2],
                ];
                let n = dot3(t, t).sqrt().max(1e-9);
                let nudge = [
                    v[0] + t[0] / n * 1e-5,
                    v[1] + t[1] / n * 1e-5,
                    v[2] + t[2] / n * 1e-5,
                ];
                let (sx, sy) = cam
                    .project(nudge)
                    .map_or((1.0, 0.0), |(a, b)| (a - x, b - y));
                let l = (sx * sx + sy * sy).sqrt().max(1e-9);
                let (sx, sy) = (sx / l, sy / l);
                // Into the picture's own frame, before it is turned.
                let r = rotation.to_radians();
                let sun_in_picture = (sx * r.cos() + sy * r.sin(), -sx * r.sin() + sy * r.cos());
                let angle = if body == Body::Moon {
                    180.0 - seen.position.elongation
                } else {
                    seen.position
                        .phase
                        .mul_add(2.0, -1.0)
                        .clamp(-1.0, 1.0)
                        .acos()
                        .to_degrees()
                };
                Some(crate::eyepiece::Phase {
                    sun: sun_in_picture,
                    angle,
                })
            }
            _ => None,
        };
        let texture = self.photos.texture(&id, phase)?;
        let radius = self.photo_half(i, cam.fov);
        let aspect = texture.height() as f64 / texture.width().max(1) as f64;
        // The picture's middle, from the object, turned as the picture is.
        let [cx, cy] = credit.centre.unwrap_or([0.5, 0.5]);
        let (dx, dy) = (
            (0.5 - cx) * 2.0 * radius,
            (0.5 - cy) * 2.0 * radius * aspect,
        );
        let (s, c) = rotation.to_radians().sin_cos();
        let offset = (dx * c - dy * s, dx * s + dy * c);
        Some(crate::view::Eyepiece {
            texture,
            x,
            y,
            radius,
            aspect,
            offset,
            rotation,
            // Saturn's rings reach past a disc, so it's laid over the sky instead.
            disc: matches!(self.finds[i].target, Target::Body(b) if b != Body::Saturn),
            credit: credit.credit.clone(),
            alpha: self.eye_alpha,
        })
    }

    fn try_meteor(&mut self, real: UnixMs) {
        let Some(i) = (0..self.finds.len())
            .find(|&i| !self.caught[i] && matches!(self.finds[i].target, Target::Meteor(_)))
        else {
            return;
        };
        let alive = self
            .meteors
            .iter()
            .any(|m| real >= m.start && real <= m.start + m.duration + 400);
        if alive {
            self.catch.holding = false;
            self.caught_one(i, real);
        }
    }

    fn caught_one(&mut self, i: usize, real: UnixMs) {
        self.caught[i] = true;
        if !matches!(self.finds[i].target, Target::Meteor(_)) {
            self.track = Some(i);
        }
        self.session.found_one(real);
        let find = self.finds[i].clone();
        if let Err(e) = self.journal.mark_found(&find.id, &self.night) {
            eprintln!("westering: couldn't save what was found: {e}");
        }
        if !self.page.finds.contains(&find.name) {
            self.page.finds.push(find.name.clone());
        }
        self.save_page();
        if self.begin_tour(i, real) {
            self.guide_tour_began(real);
            return;
        }
        let x = self.beside(Some(i), self.zoom_for(i));
        let kicker = self.kind_word(i).to_owned();
        let mut body = find.fact;
        if matches!(self.finds[i].target, Target::Hop(_))
            && (!self.journal.settings.seen.iter().any(|k| k == "hop-done")
                || westering_core::finds::stable_hash((0, 0, 0), &self.night).is_multiple_of(3))
        {
            body = format!("{body}\n\n{}", self.sky.tours.hop_closing);
            if !self.journal.settings.seen.iter().any(|k| k == "hop-done") {
                self.journal.settings.seen.push("hop-done".into());
                if let Err(e) = self.journal.save_settings() {
                    eprintln!("westering: couldn't save settings: {e}");
                }
            }
        }
        self.card = Some(Card {
            x,
            kicker,
            title: find.name,
            body,
            shown: real,
        });
        if let Target::Figure(f) = self.finds[i].target {
            self.reveal = Some((f, real));
        }
        self.after_catch(i, real);
        self.guide_caught(real);
    }

    fn save_page(&self) {
        if let Err(e) = self.journal.save_night(&self.page) {
            eprintln!("westering: couldn't save tonight's page: {e}");
        }
    }

    pub(crate) fn dismiss_card(&mut self, real: UnixMs) {
        if self.tour.is_some() && self.turn_page(real) {
            return;
        }
        self.card = None;
        if let Some(fov) = self.catch.fov_before.take() {
            self.look = Some(Look {
                az: self.camera.az,
                alt: self.camera.alt,
                fov,
                rate: 1.4,
            });
        }
        self.catch.progress = 0.0;
        self.catch.target = None;
        if self.session.all_found() && self.hunting() {
            self.guide_all_found(real);
        }
    }

    /// Moves time on by one tick and draws.
    pub fn tick(&mut self, real: UnixMs) -> Frame {
        let dt = ((real - self.last_real) as f64 / 1000.0).clamp(0.0, 0.25);
        self.last_real = real;
        self.settle_releases(real);
        if let Some(phase) = self.session.tick(real) {
            self.entered(phase, real);
        }
        if self.session.phase() == Phase::Over {
            self.quit = true;
        }
        self.tick_talk(real);
        self.guide_tick(real);
        let tempo = self.session.tempo(real);
        self.steer(dt, tempo);
        let now = self.sky_now(real);
        let hz = horizon(self.observer, now);
        let prec = precession(now);
        self.update_catch(dt, tempo, real, now, &hz, &prec);
        self.spawn_meteors(real, now, &hz);
        self.draw(real, now, &hz, &prec, tempo, dt)
    }

    pub(crate) fn entered(&mut self, phase: Phase, real: UnixMs) {
        if matches!(phase, Phase::Dimming | Phase::Finale) {
            self.talk.flow = None;
            self.talk.pending = None;
            self.set_prompt(None);
            self.drawing = None;
        }
        self.guide_phase(phase, real);
        match phase {
            Phase::Weights => self.begin_weights(real),
            Phase::Hunt => self.guide_hunt(real),
            Phase::Dimming => {
                self.card = None;
                self.catch = Catch::default();
                self.caption = Some(Timed {
                    text: "Eyes take about twenty minutes to open fully to the dark. Let's help yours begin.".into(),
                    shown: real + 1_500,
                    hold: 9_000,
                });
                self.hint = None;
            }
            Phase::Finale => {
                let now = self.clock.sky(real);
                let mut last = handoff(&self.sky, self.observer, now, self.offset_s);
                // When someone has written something heavy, the last line is a person.
                let pole = if self.observer.lat >= 0.0 { 424 } else { 4730 };
                if self.talk.caring
                    && let Some(p) = self.journal.person_on(pole)
                {
                    last.line = format!("{} would pick up. {}", p.name, last.line);
                }
                self.handoff = Some(last);
                self.caption = None;
                // Face the west, where tonight's stars go down.
                self.look = Some(Look {
                    az: 268.0,
                    alt: 14.0,
                    fov: 100.0,
                    rate: 0.9,
                });
                self.finale_turned = false;
            }
            _ => {}
        }
    }

    /// Turns the view to the first thing still to find.
    pub(crate) fn face_first_find(&mut self, real: UnixMs) {
        let now = self.clock.sky(real);
        let hz = horizon(self.observer, now);
        let prec = precession(now);
        let first = (0..self.finds.len())
            .filter(|&i| !self.caught[i])
            .find_map(|i| self.find_dir(i, now, &hz, &prec));
        if let Some(v) = first {
            let (alt, az) = alt_az(v);
            self.look = Some(Look {
                az,
                alt: alt.clamp(15.0, 62.0),
                fov: 95.0,
                rate: 1.0,
            });
        }
    }

    /// Keys, easing and the finale's camera.
    fn steer(&mut self, dt: f64, tempo: f64) {
        let phase = self.session.phase();
        if phase == Phase::Finale && !self.finale_turned {
            let into = self.session.in_phase(self.last_real);
            if into >= self.session.timings.lapse
                && let Some(h) = &self.handoff
            {
                // After the lapse the sky is back to now: face the hand-off.
                self.camera.az = h.az;
                self.camera.alt = h.alt.clamp(8.0, 60.0) + 6.0;
                self.camera.fov = 95.0;
                self.camera.update();
                self.look = None;
                self.finale_turned = true;
            }
        }
        let free = matches!(
            phase,
            Phase::Arrival | Phase::Weights | Phase::Hunt | Phase::Dimming
        );
        let speed = self.camera.fov * 0.5 * if self.held.fast { 2.6 } else { 1.0 } * tempo.sqrt();
        let want = if free && self.catch.progress < 0.02 {
            (
                (self.held.right as i32 - self.held.left as i32) as f64 * speed,
                (self.held.up as i32 - self.held.down as i32) as f64 * speed,
            )
        } else {
            (0.0, 0.0)
        };
        let k = 1.0 - (-dt * 7.0).exp();
        self.pan.0 += (want.0 - self.pan.0) * k;
        self.pan.1 += (want.1 - self.pan.1) * k;
        if self.pan.0.abs() > 1e-4 || self.pan.1.abs() > 1e-4 {
            self.look = None;
            let cos_alt = self.camera.alt.to_radians().cos().max(0.2);
            self.camera.az += self.pan.0 * dt / cos_alt;
            self.camera.alt += self.pan.1 * dt;
        }
        if free {
            let zoom = self.held.zoom_out as i32 - self.held.zoom_in as i32;
            if zoom != 0 {
                self.camera.fov *= (1.0 + 0.9 * dt).powi(zoom);
            }
        }
        // Following something: keep it where it is in the view as the sky turns.
        if let Some(i) = self.track {
            let now = self.sky_now(self.last_real);
            let hz = horizon(self.observer, now);
            let prec = precession(now);
            match self.find_dir(i, now, &hz, &prec) {
                Some(v)
                    if self.pan.0.abs() < 1e-3
                        && self.pan.1.abs() < 1e-3
                        && self.drag_from.is_none() =>
                {
                    let (alt, az) = alt_az(v);
                    match &mut self.look {
                        Some(look) => {
                            look.az = az;
                            look.alt = alt;
                        }
                        None => {
                            self.camera.az = az;
                            self.camera.alt = alt;
                        }
                    }
                }
                _ => self.track = None,
            }
        }
        if let Some(look) = self.look {
            let k = 1.0 - (-dt * look.rate * 2.0).exp();
            let daz = turn(self.camera.az, look.az);
            self.camera.az += daz * k;
            self.camera.alt += (look.alt - self.camera.alt) * k;
            self.camera.fov *= (look.fov / self.camera.fov).powf(k);
            if daz.abs() < 0.01
                && (look.alt - self.camera.alt).abs() < 0.01
                && (look.fov / self.camera.fov - 1.0).abs() < 0.001
            {
                self.look = None;
            }
        }
        if self.placing() {
            self.camera.alt = self.camera.alt.min(25.0);
        }
        self.camera.update();
    }

    fn update_catch(
        &mut self,
        dt: f64,
        tempo: f64,
        real: UnixMs,
        now: UnixMs,
        hz: &Mat3,
        prec: &Mat3,
    ) {
        if !self.hunting() || self.card.is_some() {
            return;
        }
        let r = self.reticle_radius();
        let (cx, cy) = (self.camera.width / 2.0, self.camera.height / 2.0);
        let near = (0..self.finds.len())
            .filter(|&i| !self.caught[i] && self.catchable(i))
            .filter_map(|i| {
                let v = self.find_dir(i, now, hz, prec)?;
                if alt_az(v).0 < -0.5 {
                    return None;
                }
                let (x, y) = self.camera.project(v)?;
                let d = ((x - cx).powi(2) + (y - cy).powi(2)).sqrt();
                (d < r).then_some((i, d))
            })
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(i, _)| i);
        if self.catch.progress <= 0.0 {
            self.catch.target = near;
        }
        let Some(i) = self.catch.target else {
            self.catch.progress = 0.0;
            return;
        };
        if self.catch.holding {
            if self.catch.fov_before.is_none() {
                self.catch.fov_before = Some(self.camera.fov);
            }
            let hold = if matches!(self.finds[i].target, Target::Hop(_)) {
                0.8
            } else {
                1.4
            };
            self.catch.progress += dt / (hold / tempo.max(0.3));
            if let Some(v) = self.find_dir(i, now, hz, prec) {
                let (alt, az) = alt_az(v);
                let p = smoothstep(self.catch.progress);
                let zoom = self.zoom_for(i);
                let from = self.catch.fov_before.unwrap_or(self.camera.fov);
                self.look = Some(Look {
                    az,
                    alt,
                    fov: from * (zoom / from).powf(p),
                    rate: 3.0,
                });
            }
            if self.catch.progress >= 1.0 {
                self.catch.progress = 1.0;
                self.catch.holding = false;
                if !self.hop_step(i, real) {
                    self.caught_one(i, real);
                }
            }
        } else if self.catch.progress > 0.0 {
            self.catch.progress = (self.catch.progress - dt * 1.8).max(0.0);
            if self.catch.progress == 0.0 {
                if let Some(fov) = self.catch.fov_before.take() {
                    self.look = Some(Look {
                        az: self.camera.az,
                        alt: self.camera.alt,
                        fov,
                        rate: 1.4,
                    });
                }
                self.catch.target = None;
            }
        }
    }

    fn spawn_meteors(&mut self, real: UnixMs, now: UnixMs, hz: &Mat3) {
        self.meteors.retain(|m| real < m.start + m.duration + 600);
        let Some(shower) = self.finds.iter().find_map(|f| match f.target {
            Target::Meteor(s) => Some(s),
            _ => None,
        }) else {
            return;
        };
        if real < self.next_meteor || !matches!(self.session.phase(), Phase::Hunt | Phase::Dimming)
        {
            return;
        }
        let s = &self.sky.lists.showers[shower];
        let radiant = apply(hz, apply(&precession(now), unit(s.ra, s.dec)));
        let tempo = self.session.tempo(real);
        let wait = 9_000.0 + self.random() * 22_000.0;
        self.next_meteor = real + (wait / tempo) as UnixMs;
        // A direction at right angles to the radiant, then out along it.
        let seed = [
            self.random() - 0.5,
            self.random() - 0.5,
            self.random() - 0.5,
        ];
        let along = dot3(seed, radiant);
        let p = [
            seed[0] - along * radiant[0],
            seed[1] - along * radiant[1],
            seed[2] - along * radiant[2],
        ];
        let n = dot3(p, p).sqrt().max(1e-9);
        let p = [p[0] / n, p[1] / n, p[2] / n];
        let a = (12.0 + self.random() * 40.0).to_radians();
        let from = [
            radiant[0] * a.cos() + p[0] * a.sin(),
            radiant[1] * a.cos() + p[1] * a.sin(),
            radiant[2] * a.cos() + p[2] * a.sin(),
        ];
        if alt_az(from).0 < 8.0 {
            return;
        }
        let toward = [
            p[0] * a.cos() - radiant[0] * a.sin(),
            p[1] * a.cos() - radiant[1] * a.sin(),
            p[2] * a.cos() - radiant[2] * a.sin(),
        ];
        let length = (7.0 + self.random() * 14.0).to_radians();
        let brightness = 0.8 + self.random() as f32 * 0.9;
        let duration = 650 + (self.random() * 500.0) as UnixMs;
        self.meteors.push(Meteor {
            start: real,
            duration,
            from,
            toward,
            length,
            brightness,
        });
    }

    /// The slow-changing layer under the stars: the faint lattice, the air's
    /// glow near the horizon, twilight, the Milky Way and the ground. Worked
    /// out once per two-by-two block of dots, which it is smooth enough for.
    fn rebuild_base(&mut self, hz: &Mat3, sun: Vec3, sun_alt: f64, limit: f64) {
        let cam = self.camera;
        let (cols, rows) = (self.field.cols, self.field.rows);
        let pitch = self.field.pitch as f64;
        let to_eq = transpose(hz);
        // The Milky Way's frame: the north galactic pole and the galactic centre (J2000).
        let ngp = unit(192.859_48, 27.128_25);
        let gc = unit(266.405, -28.936_17);
        let gy = [
            ngp[1] * gc[2] - ngp[2] * gc[1],
            ngp[2] * gc[0] - ngp[0] * gc[2],
            ngp[0] * gc[1] - ngp[1] * gc[0],
        ];
        let milky = ((limit - 3.2) / 2.3).clamp(0.0, 1.0);
        let twilight = ((sun_alt + 18.0) / 24.0).clamp(0.0, 1.0);
        let twilight = (twilight * twilight) as f32;
        let base = self.field.base_mut();
        for br in (0..rows).step_by(2) {
            for bc in (0..cols).step_by(2) {
                let (x, y) = ((bc as f64 + 1.0) * pitch, (br as f64 + 1.0) * pitch);
                let v = cam.unproject(x, y);
                let (alt, az) = alt_az(v);
                let mut milky_here = 0f32;
                let cell: Rgb = if alt < hills(az) {
                    let ground = 0.018 + 0.03 * twilight;
                    [ground * 0.9, ground * 0.85, ground * 0.8]
                } else {
                    let mut m = 0.0;
                    if milky > 0.0 {
                        let eq = apply(&to_eq, v);
                        let b = dot3(eq, ngp).clamp(-1.0, 1.0).asin().to_degrees();
                        let l = dot3(eq, gy).atan2(dot3(eq, gc));
                        let core = ((1.0 + l.cos()) / 2.0).powi(3);
                        let width = 6.5 + 7.0 * core;
                        let band = (-(b / width).powi(2)).exp();
                        if band > 0.01 {
                            let grain = noise3(eq[0] * 9.0, eq[1] * 9.0, eq[2] * 9.0) * 0.6
                                + noise3(eq[0] * 23.0, eq[1] * 23.0, eq[2] * 23.0) * 0.4;
                            let ld = l.to_degrees();
                            let rift = if (15.0..80.0).contains(&ld) {
                                0.55 * (-((b - 2.0) / 3.0).powi(2)).exp()
                            } else {
                                0.0
                            };
                            m = band
                                * (0.35 + 0.65 * core)
                                * (0.45 + 0.9 * grain)
                                * (1.0 - rift)
                                * 0.14
                                * milky;
                        }
                    }
                    let m = m as f32;
                    milky_here = m;
                    let lattice = 0.024f32;
                    let air = (0.03 * (-(alt / 9.0)).exp()) as f32;
                    let sunward = ((dot3(v, sun) + 1.0) / 2.0).powi(3) as f32;
                    let dusk = twilight * (0.18 + 0.5 * sunward);
                    [
                        lattice * 0.8 + air * 0.7 + dusk * (0.45 + 0.5 * sunward) + m * 0.82,
                        lattice * 0.88 + air * 0.8 + dusk * (0.55 + 0.2 * sunward) + m * 0.87,
                        lattice * 1.1 + air * 1.0 + dusk * (0.95 - 0.3 * sunward) + m,
                    ]
                };
                for r in br..(br + 2).min(rows) {
                    for c in bc..(bc + 2).min(cols) {
                        base[r * cols + c] = cell;
                    }
                }
                // The Milky Way as a dust of faint dots rather than a fog: each
                // dot takes a share by where it points on the sky, so the dust
                // stays put as the view moves.
                // Zoomed in, the dust's cells would show as blotches: let it go smooth.
                let dusty = smoothstep((cam.fov - 8.0) / 30.0) as f32;
                if milky_here > 0.002 && dusty > 0.0 {
                    for r in br..(br + 2).min(rows) {
                        for c in bc..(bc + 2).min(cols) {
                            let v =
                                cam.unproject((c as f64 + 0.5) * pitch, (r as f64 + 0.5) * pitch);
                            let eq = apply(&to_eq, v);
                            let cell_of = |x: f64, prime: i64| (x * 420.0).floor() as i64 * prime;
                            let k = cell_of(eq[0], 73_856_093)
                                ^ cell_of(eq[1], 19_349_663)
                                ^ cell_of(eq[2], 83_492_791);
                            let h = ((k as u64).wrapping_mul(0x9e37_79b9_7f4a_7c15) >> 40) as f32
                                / (1u64 << 24) as f32;
                            let share = (0.15 + 2.2 * h * h * h) / 0.7 - 1.0;
                            let add = milky_here * share * dusty;
                            let cell = &mut base[r * cols + c];
                            cell[0] = (cell[0] + add * 0.82).max(0.0);
                            cell[1] = (cell[1] + add * 0.87).max(0.0);
                            cell[2] = (cell[2] + add).max(0.0);
                        }
                    }
                }
            }
        }
    }

    fn draw(
        &mut self,
        real: UnixMs,
        now: UnixMs,
        hz: &Mat3,
        prec: &Mat3,
        tempo: f64,
        dt: f64,
    ) -> Frame {
        let cam = self.camera;
        // Fine enough that the lattice reads as texture, not as pixels.
        let pitch = if cam.width > 2200.0 { 3.6 } else { 3.2 };
        let resized = self.field.fit(cam.width, cam.height, pitch);
        let sun_seen = see(Body::Sun, self.observer, now);
        let sun_alt = sun_seen.alt;
        let sun = from_alt_az(sun_seen.alt, sun_seen.az);
        let limit = limiting_magnitude(sun_alt);
        let lapsing =
            self.session.lapse_progress(real) > 0.0 && self.session.lapse_progress(real) < 1.0;
        let stamp = BaseStamp {
            az: (cam.az * 50.0).round(),
            alt: (cam.alt * 50.0).round(),
            fov: (cam.fov * 200.0).round(),
            width: cam.width,
            height: cam.height,
            sky_minute: now / 20_000,
            lapse: lapsing,
        };
        if resized || self.base != Some(stamp) || lapsing {
            self.rebuild_base(hz, sun, sun_alt, limit);
            self.base = Some(stamp);
        }
        self.field.begin();
        self.points.clear();
        let t = real as f64 / 1000.0;
        let arrival = if self.session.phase() == Phase::Arrival {
            1.0 - smoothstep(
                self.session.in_phase(real) as f64 / self.session.timings.arrival as f64,
            )
        } else {
            0.0
        };

        // Stars.
        let ppd = cam.px_per_degree();
        let shown = limit.min(5.6) + 0.6;
        let low_sin = 14f64.to_radians().sin();
        let algol = self
            .algol
            .map(|k| (k, westering_core::tours::algol_magnitude(now)));
        for (k, star) in self.prepared.iter().enumerate() {
            if star.mag > shown {
                break;
            }
            let (mag, star_light) = match algol {
                Some((a, m)) if a == k => (m, light(m)),
                _ => (star.mag, star.light),
            };
            let mut v = apply(hz, star.dir);
            if v[2] < -0.02 {
                continue;
            }
            if v[2] < 0.06 {
                let (alt, az) = alt_az(v);
                let lifted = alt + refraction(alt);
                if lifted < hills(az) {
                    continue;
                }
                v = from_alt_az(lifted, az);
            }
            let Some((mut x, mut y)) = cam.project(v) else {
                continue;
            };
            if !cam.on_screen(x, y, 60.0) {
                continue;
            }
            if arrival > 0.0 {
                let spread = arrival * arrival * 70.0;
                x += (star.phase * 12.9898).sin() * spread;
                y += (star.phase * 78.233).sin() * spread;
            }
            let fade = smoothstep((shown - star.mag) / 1.2) as f32;
            let low = 0.3 + 0.7 * smoothstep(v[2] / low_sin);
            let rate = star.rate * tempo;
            let depth = (0.12 + 0.22 * (1.0 - v[2]).powi(2))
                * if self.journal.settings.calm {
                    0.35
                } else {
                    1.0
                };
            let twinkle = 1.0
                + depth
                    * ((t * rate + star.phase).sin() * 0.6
                        + (t * rate * 1.7 + star.phase * 0.37).sin() * 0.4);
            let amount = star_light * fade * (low * twinkle) as f32;
            if mag < BRIGHT {
                self.points.push(point(x, y, star.tint, amount, 1.0));
            } else {
                // Single dots on the fine lattice need a little more light to hold the eye.
                self.field.star(x, y, star.tint, amount * 1.45);
            }
        }

        // A photograph taking over from the dots it replaces.
        self.update_eye(now, hz, prec, dt);
        let (photo_body, photo_piece) = match self.eye.map(|i| &self.finds[i].target) {
            Some(Target::Body(b)) => (Some(*b), None),
            Some(Target::Showpiece(p)) => (None, Some(*p)),
            _ => (None, None),
        };
        let keep = (1.0 - self.eye_alpha) as f32;

        // Showpieces' soft light. Zoomed in, the view gathers light like a
        // telescope and fainter things show.
        let gather = gather(cam.fov);
        for (pi, piece) in self.sky.lists.showpieces.iter().enumerate() {
            let fade = if photo_piece == Some(pi) { keep } else { 1.0 };
            if piece.mag > limit + gather + 0.5
                || !matches!(piece.kind, Kind::Cluster | Kind::Galaxy | Kind::Nebula)
            {
                continue;
            }
            let v = apply(hz, apply(prec, unit(piece.ra, piece.dec)));
            if alt_az(v).0 < 1.0 {
                continue;
            }
            let Some((x, y)) = cam.project(v) else {
                continue;
            };
            if !cam.on_screen(x, y, 200.0) {
                continue;
            }
            let true_radius = piece.size / 60.0 / 2.0 * ppd;
            let radius = true_radius.max(pitch as f64 * 0.8);
            let strength = (10f64.powf(-0.4 * (piece.mag - gather - 3.0))).min(2.0) as f32;
            // How comfortably it's within reach: faint things just in reach
            // still show, rather than fading into the lattice.
            let seen = smoothstep((limit + gather + 0.5 - piece.mag) / 1.5) as f32;
            if piece.kind != Kind::Cluster && (true_radius < 9.0 || piece.size < 3.0) {
                // Small nebulae and galaxies: a soft point, a little disc close up.
                let color = if piece.kind == Kind::Nebula {
                    [0.62, 0.95, 0.92]
                } else {
                    [0.95, 0.93, 0.86]
                };
                self.points.push(crate::view::Point {
                    x,
                    y,
                    radius: (true_radius as f32).clamp(1.3, 40.0),
                    color,
                    alpha: (0.3 + 0.4 * seen) * fade * if true_radius > 9.0 { 0.8 } else { 1.0 },
                    halo: 0.9 * seen * fade,
                });
                continue;
            }
            if piece.kind == Kind::Cluster {
                // Stars too faint to see one by one, sprinkled as single dots.
                let count = (piece.size / 4.0).clamp(8.0, 36.0) as usize;
                let seed = piece
                    .id
                    .bytes()
                    .fold(7u64, |h, b| h.wrapping_mul(31).wrapping_add(b as u64));
                for k in 0..count {
                    let h1 = ((seed.wrapping_add(k as u64 * 7919)) as f64 * 0.618_034).fract();
                    let h2 = ((seed.wrapping_add(k as u64 * 104_729)) as f64 * 0.414_214).fract();
                    let rr = radius * h1.sqrt();
                    let a = h2 * std::f64::consts::TAU;
                    let twinkle = 0.7 + 0.3 * (t * 1.3 * tempo + k as f64).sin();
                    self.field.dot(
                        x + rr * a.cos(),
                        y + rr * a.sin(),
                        [0.85, 0.9, 1.0],
                        0.07 * strength * twinkle as f32 * fade,
                    );
                }
            } else {
                let amount = (0.05 * strength).min(0.12).max(0.035 * seen);
                let amount = if radius > 120.0 {
                    amount * 120.0 / radius as f32
                } else {
                    amount
                };
                let color = if piece.kind == Kind::Nebula {
                    [0.8, 0.95, 0.95]
                } else {
                    [0.95, 0.93, 0.88]
                };
                self.field.glow(x, y, radius, color, amount * fade);
            }
        }

        // The Sun, the Moon and the planets.
        for body in [
            Body::Sun,
            Body::Moon,
            Body::Mercury,
            Body::Venus,
            Body::Mars,
            Body::Jupiter,
            Body::Saturn,
        ] {
            let s = if body == Body::Sun {
                sun_seen
            } else {
                see(body, self.observer, now)
            };
            if s.alt < hills(s.az) - 0.3 {
                continue;
            }
            let v = from_alt_az(s.alt, s.az);
            let Some((x, y)) = cam.project(v) else {
                continue;
            };
            if !cam.on_screen(x, y, 120.0) {
                continue;
            }
            let radius = s.position.diameter / 3600.0 / 2.0 * ppd;
            let fade = if photo_body == Some(body) { keep } else { 1.0 };
            match body {
                Body::Sun => {
                    self.field
                        .glow(x, y, radius.max(pitch as f64 * 2.2), [1.0, 0.95, 0.82], 3.0)
                }
                Body::Moon => {
                    // Drawn larger than life when the view is wide, so it reads as round.
                    let r = radius.max(pitch as f64 * 5.5);
                    self.lit_disc(
                        x,
                        y,
                        r,
                        v,
                        sun,
                        180.0 - s.position.elongation,
                        [1.0, 0.97, 0.9],
                        1.05 * fade,
                        true,
                    );
                }
                _ => {
                    let color = match body {
                        Body::Mars => [1.0, 0.66, 0.5],
                        Body::Jupiter => [1.0, 0.95, 0.86],
                        Body::Saturn => [1.0, 0.9, 0.72],
                        _ => [1.0, 0.97, 0.92],
                    };
                    if radius > pitch as f64 * 1.2 {
                        let phase_angle = s
                            .position
                            .phase
                            .mul_add(2.0, -1.0)
                            .clamp(-1.0, 1.0)
                            .acos()
                            .to_degrees();
                        self.lit_disc(x, y, radius, v, sun, phase_angle, color, 1.5 * fade, false);
                    } else {
                        let low = 0.4 + 0.6 * smoothstep(s.alt / 12.0);
                        let amount = light(s.position.magnitude) * 1.08 * low as f32;
                        let mut p = point(x, y, color, amount, 1.3);
                        p.alpha *= fade;
                        p.halo *= fade;
                        self.points.push(p);
                    }
                }
            }
        }

        // Jupiter's four big moons, close up.
        self.moon_labels.clear();
        let jup = see(Body::Jupiter, self.observer, now);
        if cam.fov < 1.5 && jup.alt > 0.0 {
            let v = from_alt_az(jup.alt, jup.az);
            if let Some((jx, jy)) = cam.project(v) {
                let toward = |w: Vec3| {
                    let along = dot3(w, v);
                    let t = [
                        w[0] - along * v[0],
                        w[1] - along * v[1],
                        w[2] - along * v[2],
                    ];
                    let n = dot3(t, t).sqrt().max(1e-12);
                    let nudge = [
                        v[0] + t[0] / n * 1e-5,
                        v[1] + t[1] / n * 1e-5,
                        v[2] + t[2] / n * 1e-5,
                    ];
                    cam.project(nudge).map(|(a, b)| {
                        let (dx, dy) = (a - jx, b - jy);
                        let l = (dx * dx + dy * dy).sqrt().max(1e-12);
                        (dx / l, dy / l)
                    })
                };
                let ra = jup.ra.to_radians();
                let east = apply(hz, [-ra.sin(), ra.cos(), 0.0]);
                let pole = apply(hz, apply(prec, westering_core::jupiter::pole()));
                if let (Some(e), Some(p)) = (toward(east), toward(pole)) {
                    // Along Jupiter's equator, the way that points to the sky's west.
                    let mut q = (-p.1, p.0);
                    if q.0 * -e.0 + q.1 * -e.1 < 0.0 {
                        q = (-q.0, -q.1);
                    }
                    let r = jup.position.diameter / 7200.0 * ppd;
                    let a = smoothstep((1.5 - cam.fov) / 1.0) as f32;
                    for (k, m) in westering_core::jupiter::moons(now).iter().enumerate() {
                        if m.behind && m.x.abs() < 1.0 {
                            continue;
                        }
                        let (mx, my) = (
                            jx + (m.x * q.0 + m.y * p.0) * r,
                            jy + (m.x * q.1 + m.y * p.1) * r,
                        );
                        let mut dot = point(mx, my, [1.0, 0.97, 0.9], 0.9, 0.6);
                        dot.alpha *= a;
                        dot.halo *= a;
                        dot.radius = dot.radius.min(2.2);
                        self.points.push(dot);
                        self.moon_labels
                            .push((mx, my, westering_core::jupiter::NAMES[k], a));
                    }
                }
            }
        }

        // Meteors.
        for m in &self.meteors {
            let age = (real - m.start) as f64 / m.duration as f64;
            if !(0.0..=1.3).contains(&age) {
                continue;
            }
            let head = age.min(1.0) * m.length;
            let tail = (head - m.length * 0.4).max(0.0);
            let fade = if age > 1.0 {
                1.0 - (age - 1.0) / 0.3
            } else {
                1.0
            } as f32;
            let at = |th: f64| {
                [
                    m.from[0] * th.cos() + m.toward[0] * th.sin(),
                    m.from[1] * th.cos() + m.toward[1] * th.sin(),
                    m.from[2] * th.cos() + m.toward[2] * th.sin(),
                ]
            };
            let steps = 18;
            let mut prev: Option<(f64, f64)> = None;
            for k in 0..=steps {
                let f = k as f64 / steps as f64;
                let th = tail + (head - tail) * f;
                let p = cam.project(at(th));
                if let (Some(a), Some(b)) = (prev, p) {
                    self.field.line(
                        a,
                        b,
                        [0.9, 0.95, 1.0],
                        m.brightness * fade * (0.08 + 0.5 * f as f32),
                    );
                }
                prev = p;
            }
        }

        self.draw_marks(real, hz);
        self.marks.clear();
        self.draw_tours(real, hz, prec);
        let lines = self.pattern_lines(real, hz, dt);
        self.draw_reticle(real, tempo);

        let brightness = self.session.brightness(real);
        let veil = self.finale_veil(real);
        let frame_texture = self.field.texture((brightness * (1.0 - veil)) as f32 * 1.0);
        let mut texts = self.labels(real, now, hz, prec, brightness);
        texts.extend(self.mark_labels(hz, brightness));
        for &(x, y, name, a) in &self.moon_labels {
            texts.push(Text::new(
                x + 8.0,
                y + 6.0,
                name,
                12.0,
                0.55 * a as f64 * brightness,
            ));
        }
        texts.extend(self.words(real, brightness));
        let (sprites, bubble, embers) = self.guide_frame(real, brightness);
        if let Some(i) = self.eye
            && self.eye_alpha > 0.05
            && let Some(c) = self
                .photo_id(i)
                .and_then(|id| self.photos.credit(&id).cloned())
        {
            // Laid out from the bottom up: the credit, and above it what the
            // picture shows that an eye wouldn't.
            let credit = format!("Photograph: {}", c.credit);
            let lines = |text: &str, per_line: f64| (text.chars().count() as f64 / per_line).ceil();
            let credit_y = cam.height - 30.0 - lines(&credit, 62.0) * 15.0;
            texts.push(
                Text::new(
                    cam.width - 420.0,
                    credit_y,
                    credit,
                    11.5,
                    0.45 * self.eye_alpha,
                )
                .wrap(400.0),
            );
            if let Some(caption) = c.caption {
                let y = credit_y - 10.0 - lines(&caption, 50.0) * 20.0;
                texts.push(
                    Text::new(cam.width - 420.0, y, caption, 14.5, 0.8 * self.eye_alpha)
                        .wrap(400.0)
                        .color([0.95, 0.92, 0.85]),
                );
            }
        }
        self.marks.extend(embers);
        let eyepiece = self.eyepiece(now, hz, prec);
        let card = self.card.as_ref().map(|c| crate::view::CardView {
            x: c.x,
            y: (self.camera.height / 2.0 - 70.0).max(70.0),
            kicker: c.kicker.clone(),
            title: c.title.clone(),
            body: c.body.clone(),
            keys: match &self.tour {
                Some(t) if t.on_last_page() => vec![("Space".into(), "carry on".into())],
                Some(_) => vec![
                    ("Space".into(), "go on".into()),
                    ("Esc".into(), "stop here".into()),
                ],
                None => vec![
                    ("Space".into(), "carry on".into()),
                    ("Tab".into(), "next".into()),
                ],
            },
            alpha: envelope(real - c.shown, 600, 600_000, 800),
        });
        let compass = matches!(
            self.session.phase(),
            Phase::Arrival | Phase::Weights | Phase::Hunt | Phase::Dimming
        )
        .then(|| crate::view::Compass {
            heading: cam.az,
            alpha: brightness.max(0.5),
        });
        Frame {
            eyepiece,
            card,
            compass,
            points: std::mem::take(&mut self.points),
            marks: std::mem::take(&mut self.marks),
            lines,
            sprites,
            bubble,
            texture: Some(frame_texture),
            cols: self.field.cols,
            rows: self.field.rows,
            pitch: self.field.pitch,
            background: [0.012, 0.014, 0.024],
            texts,
            veil: 0.0,
        }
    }

    /// The moment the time-lapse hands back to the present: a dip to dark.
    fn finale_veil(&self, real: UnixMs) -> f64 {
        if self.session.phase() != Phase::Finale {
            return 0.0;
        }
        let into = self.session.in_phase(real) - self.session.timings.lapse;
        let x = into as f64 / 1_300.0;
        if (-1.0..=1.0).contains(&x) {
            1.0 - x.abs()
        } else {
            0.0
        }
    }

    /// A disc lit from the Sun's side: the Moon, or a planet close up.
    #[allow(clippy::too_many_arguments)]
    fn lit_disc(
        &mut self,
        x: f64,
        y: f64,
        radius: f64,
        v: Vec3,
        sun: Vec3,
        phase_angle: f64,
        color: Rgb,
        amount: f32,
        mottled: bool,
    ) {
        // Which way the Sun is, on the screen.
        let along = dot3(sun, v);
        let t = [
            sun[0] - along * v[0],
            sun[1] - along * v[1],
            sun[2] - along * v[2],
        ];
        let n = dot3(t, t).sqrt().max(1e-9);
        let nudge = [
            v[0] + t[0] / n * 0.01,
            v[1] + t[1] / n * 0.01,
            v[2] + t[2] / n * 0.01,
        ];
        let (ux, uy) = match (self.camera.project(v), self.camera.project(nudge)) {
            (Some(a), Some(b)) => {
                let (dx, dy) = (b.0 - a.0, b.1 - a.1);
                let l = (dx * dx + dy * dy).sqrt().max(1e-9);
                (dx / l, dy / l)
            }
            _ => (1.0, 0.0),
        };
        let phase_angle = phase_angle.to_radians();
        let (si, ci) = (phase_angle.sin(), phase_angle.cos());
        let pitch = self.field.pitch as f64;
        let soft = (pitch / radius).min(0.5);
        self.field.disc(x, y, radius, |u, w| {
            let rho2 = u * u + w * w;
            if rho2 > 1.0 + soft {
                return None;
            }
            let edge = ((1.0 - rho2.sqrt()) / soft + 0.5).clamp(0.0, 1.0);
            let z = (1.0 - rho2.min(1.0)).sqrt();
            let lit = (u * ux + w * uy) * si + z * ci;
            let day = smoothstep(lit * 6.0 + 0.5);
            let texture = if mottled {
                let n = noise3(u * 3.1 + 7.0, w * 3.1 + 3.0, 1.7) * 0.6
                    + noise3(u * 7.3, w * 7.3, 4.2) * 0.4;
                0.5 + 0.62 * n
            } else {
                1.0
            };
            let limb = 0.8 + 0.2 * z;
            let value = (day * texture * limb) as f32 * amount + (1.0 - day as f32) * 0.03;
            Some((color, value * edge as f32))
        });
    }

    fn draw_reticle(&mut self, real: UnixMs, tempo: f64) {
        let ringed = matches!(self.session.phase(), Phase::Hunt | Phase::Arrival) || self.placing();
        if !ringed || self.card.is_some() {
            return;
        }
        let r = self.reticle_radius();
        let (cx, cy) = (self.camera.width / 2.0, self.camera.height / 2.0);
        let has = self.catch.target.is_some() || self.placing();
        let breathe = 0.5 + 0.5 * ((real as f64 / 1000.0) * 1.1 * tempo).sin();
        let points = 28;
        for k in 0..points {
            let f = k as f64 / points as f64;
            // Four arcs with gaps at the compass points.
            let quarter = (f * 4.0).fract();
            if !(0.12..0.88).contains(&quarter) {
                continue;
            }
            let a = f * std::f64::consts::TAU - std::f64::consts::FRAC_PI_2;
            let (x, y) = (cx + r * a.cos(), cy + r * a.sin());
            let filled = f < self.catch.progress;
            let (color, amount) = if filled {
                (WARM, 0.7)
            } else if has {
                (RETICLE, 0.22 + 0.1 * breathe as f32)
            } else {
                (RETICLE, 0.09)
            };
            self.field.splat(x, y, color, amount);
        }
    }

    /// Names beside things near the reticle, and the compass on the horizon.
    fn labels(
        &self,
        real: UnixMs,
        now: UnixMs,
        hz: &Mat3,
        prec: &Mat3,
        brightness: f64,
    ) -> Vec<Text> {
        let mut out = Vec::new();
        let cam = &self.camera;
        let (cx, cy) = (cam.width / 2.0, cam.height / 2.0);
        let r = self.reticle_radius();
        if matches!(
            self.session.phase(),
            Phase::Hunt | Phase::Arrival | Phase::Dimming
        ) {
            for (name, az) in [
                ("N", 0.0),
                ("E", 90.0),
                ("S", 180.0),
                ("W", 270.0),
                ("NE", 45.0),
                ("SE", 135.0),
                ("SW", 225.0),
                ("NW", 315.0),
            ] {
                if let Some((x, y)) = cam.project(from_alt_az(hills(az) + 0.8, az))
                    && cam.on_screen(x, y, -10.0)
                {
                    let (size, alpha) = if name.len() == 1 {
                        (17.0, 0.6)
                    } else {
                        (12.0, 0.4)
                    };
                    let mut label =
                        Text::new(x, y - 26.0, name, size, alpha * brightness).centred();
                    if name.len() == 1 {
                        label = label.bold();
                    }
                    if name == "N" {
                        label = label.color([1.0, 0.8, 0.58]);
                    }
                    out.push(label);
                }
            }
        }
        if self.hunting() && self.card.is_none() {
            if let Some(i) = self.catch.target
                && let Some(v) = self.find_dir(i, now, hz, prec)
                && let Some((x, y)) = cam.project(v)
            {
                let near = 1.0 - (((x - cx).powi(2) + (y - cy).powi(2)).sqrt() / r).min(1.0);
                let words = if self.catch.progress > 0.0 {
                    self.finds[i].name.clone()
                } else {
                    format!("{} · hold Space", self.finds[i].name)
                };
                let lx = cx + r.max(self.apparent_radius(i, cam.fov).min(r * 1.6)) + 14.0;
                out.push(Text::new(
                    lx,
                    cy - 9.0,
                    words,
                    14.0,
                    (0.35 + 0.5 * near) * brightness,
                ));
            }
            // Caught things keep a quiet name when the view passes them.
            for (i, f) in self.finds.iter().enumerate() {
                if !self.caught[i] {
                    continue;
                }
                if let Some(v) = self.find_dir(i, now, hz, prec)
                    && alt_az(v).0 > 0.0
                    && let Some((x, y)) = cam.project(v)
                {
                    let d = ((x - cx).powi(2) + (y - cy).powi(2)).sqrt();
                    if d < r * 2.5 {
                        let a = 0.3 * (1.0 - d / (r * 2.5));
                        out.push(Text::new(
                            x + 12.0,
                            y + 6.0,
                            f.name.clone(),
                            12.0,
                            a * brightness,
                        ));
                    }
                }
            }
        }
        let _ = real;
        out
    }

    /// The caption, the card, the hint, the last line and tonight's dots.
    fn words(&self, real: UnixMs, brightness: f64) -> Vec<Text> {
        let mut out = Vec::new();
        let (w, h) = (self.camera.width, self.camera.height);
        let text_light = brightness.max(0.55);
        if self.talk.caring {
            out.push(
                Text::new(w - 400.0, h - 130.0, westering_core::care::NOTE, 13.0, 0.75)
                    .wrap(360.0)
                    .color([1.0, 0.9, 0.8]),
            );
        }
        // The weight being hung keeps its words beside it.
        if let Some(words) = self.placing_words() {
            out.push(
                Text::new(w / 2.0 + 18.0, h / 2.0 - 10.0, words, 16.0, 0.85)
                    .wrap(320.0)
                    .color([1.0, 0.84, 0.68]),
            );
        }
        if let Some(c) = &self.caption {
            let a = envelope(real - c.shown, 1_500, c.hold, 2_000);
            out.push(
                Text::new(w / 2.0, 62.0, c.text.clone(), 16.0, 0.85 * a * text_light)
                    .centred()
                    .wrap(w * 0.8),
            );
        }
        if let Some(hint) = &self.hint {
            let a = envelope(real - hint.shown, 1_000, hint.hold, 1_800);
            out.push(Text::new(w / 2.0, h - 44.0, hint.text.clone(), 13.0, 0.5 * a).centred());
        }
        if let Some(handoff) = &self.handoff
            && self.session.last_line(real)
        {
            let into = self.session.in_phase(real) - self.session.timings.lapse - 900;
            let a = if self.session.phase() == Phase::LightsOut {
                1.0 - smoothstep(
                    self.session.in_phase(real) as f64 / self.session.timings.lights_out as f64
                        * 1.4,
                )
            } else {
                smoothstep(into as f64 / 2_000.0)
            };
            out.push(
                Text::new(w / 2.0, h * 0.72, handoff.line.clone(), 21.0, 0.92 * a)
                    .centred()
                    .wrap((w * 0.7).min(760.0)),
            );
        }
        out
    }

    /// How loud the music should be, 0 to 1: it arrives with the sky, eases
    /// down as the sky dims and goes with the lights.
    pub fn music_level(&self, real: UnixMs) -> f64 {
        let b = self.session.brightness(real);
        let dim = westering_core::session::DIM;
        match self.session.phase() {
            Phase::LightsOut | Phase::Over => 0.6 * b / dim,
            _ => 0.6 + 0.4 * ((b - dim) / (1.0 - dim)).max(0.0),
        }
    }

    pub fn quiet(&self) -> bool {
        self.journal.settings.quiet
    }

    pub fn volume(&self) -> f64 {
        self.journal.settings.volume.unwrap_or(1.0).clamp(0.0, 1.0)
    }

    pub(crate) fn calm(&self) -> bool {
        self.journal.settings.calm
    }

    /// Keeps the frame clock's pace honest: fast while things move, slow at rest.
    pub fn wants_fast_frames(&self, real: UnixMs) -> bool {
        self.look.is_some()
            || self.pan.0.abs() > 1e-3
            || self.pan.1.abs() > 1e-3
            || self.drag_from.is_some()
            || self.catch.progress > 0.0
            || self.meteors.iter().any(|m| real >= m.start - 50)
            || matches!(
                self.session.phase(),
                Phase::Arrival | Phase::Finale | Phase::LightsOut
            )
            || self.session.lapse_progress(real) > 0.0
    }
}
