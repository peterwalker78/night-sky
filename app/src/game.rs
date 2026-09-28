//! One visit: the real sky drawn as dots, the hunt, and the ending. Owns the
//! session, the camera and the dot field, takes input, and turns each tick
//! into a frame for the view.

use crate::camera::Camera;
use crate::field::{Field, Rgb, noise3};
use crate::view::{Frame, Text};
use gtk::gdk;
use night_sky_core::catalogues::Kind;
use night_sky_core::coords::{
    Mat3, Observer, Vec3, alt_az, apply, from_alt_az, horizon, precession, refraction, unit,
};
use night_sky_core::ephem::{Body, moon_age, moon_phase_name};
use night_sky_core::finale::{Handoff, handoff};
use night_sky_core::finds::{Find, Target, night_key, night_of, tonight};
use night_sky_core::journal::{Journal, Night};
use night_sky_core::session::{Phase, Session, Timings};
use night_sky_core::sky::{Sky, limiting_magnitude, see};
use night_sky_core::time::{MONTHS, UnixMs, civil_date, weekday};

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
struct Held {
    space: bool,
    left: bool,
    right: bool,
    up: bool,
    down: bool,
    zoom_in: bool,
    zoom_out: bool,
    fast: bool,
}

/// Easing the view towards somewhere.
#[derive(Clone, Copy)]
struct Look {
    az: f64,
    alt: f64,
    fov: f64,
    /// Per second, 0 to 1: how much of the gap closes.
    rate: f64,
}

#[derive(Default)]
struct Catch {
    target: Option<usize>,
    holding: bool,
    progress: f64,
    /// The field of view before the zoom, to go back to.
    fov_before: Option<f64>,
}

struct Card {
    x: f64,
    title: String,
    body: String,
    shown: UnixMs,
}

struct Timed {
    text: String,
    shown: UnixMs,
    hold: UnixMs,
}

struct Meteor {
    start: UnixMs,
    duration: UnixMs,
    from: Vec3,
    toward: Vec3,
    length: f64,
    brightness: f32,
}

/// What the slow-changing base layer was last drawn for.
#[derive(Clone, Copy, PartialEq)]
struct BaseStamp {
    az: f64,
    alt: f64,
    fov: f64,
    width: f64,
    height: f64,
    sky_minute: i64,
    lapse: bool,
}

/// A star with everything that doesn't change during a visit worked out.
struct Prepared {
    dir: Vec3,
    mag: f64,
    light: f32,
    tint: Rgb,
    rate: f64,
    phase: f64,
}

pub struct Game {
    sky: Sky,
    prepared: Vec<Prepared>,
    observer: Observer,
    clock: Clock,
    offset_s: i32,
    night: String,
    session: Session,
    finds: Vec<Find>,
    caught: Vec<bool>,
    journal: Journal,
    page: Night,
    camera: Camera,
    look: Option<Look>,
    field: Field,
    base: Option<BaseStamp>,
    star_dirs: Vec<Vec3>,
    held: Held,
    pan: (f64, f64),
    drag_from: Option<(f64, f64, f64, f64)>,
    catch: Catch,
    card: Option<Card>,
    caption: Option<Timed>,
    hint: Option<Timed>,
    meteors: Vec<Meteor>,
    next_meteor: UnixMs,
    handoff: Option<Handoff>,
    finale_turned: bool,
    last_input: UnixMs,
    /// Key releases wait a moment: X11's auto-repeat sends a release before
    /// every repeated press, and a real release has no press behind it.
    releases: Vec<(gdk::Key, UnixMs)>,
    esc_armed: UnixMs,
    last_real: UnixMs,
    rng: u64,
    pub quit: bool,
}

const WARM: Rgb = [1.0, 0.86, 0.66];
const RETICLE: Rgb = [0.78, 0.84, 1.0];

fn smoothstep(x: f64) -> f64 {
    let x = x.clamp(0.0, 1.0);
    x * x * (3.0 - 2.0 * x)
}

/// Fades in over `rise`, holds, and fades out over `fall`.
fn envelope(age: UnixMs, rise: UnixMs, hold: UnixMs, fall: UnixMs) -> f64 {
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
fn hills(az: f64) -> f64 {
    let a = az.to_radians();
    (0.75
        + 0.45 * (3.0 * a + 0.7).sin()
        + 0.3 * (7.0 * a + 2.1).sin()
        + 0.18 * (13.0 * a + 0.3).sin())
    .max(0.15)
}

/// How much light a star of magnitude `mag` puts on its dot.
fn light(mag: f64) -> f32 {
    (1.6 * 10f64.powf(-0.4 * (mag - 1.0)).powf(0.55)) as f32
}

fn transpose(m: &Mat3) -> Mat3 {
    [
        [m[0][0], m[1][0], m[2][0]],
        [m[0][1], m[1][1], m[2][1]],
        [m[0][2], m[1][2], m[2][2]],
    ]
}

fn dot3(a: Vec3, b: Vec3) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn angle_between(a: Vec3, b: Vec3) -> f64 {
    dot3(a, b).clamp(-1.0, 1.0).acos().to_degrees()
}

/// Shortest signed turn from one azimuth to another.
fn turn(from: f64, to: f64) -> f64 {
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
            journal.found_before(id, &night)
        });
        let caught: Vec<bool> = finds
            .iter()
            .map(|f| journal.found_on(&f.id, &night))
            .collect();
        let still = caught.iter().filter(|c| !**c).count();
        let mut session = Session::new(real_now, timings, still, false);
        if still == 0 {
            session.found_one(real_now);
        }
        let page = journal.night(&night).unwrap_or_else(|| Night {
            key: night.clone(),
            ..Night::default()
        });
        let star_dirs = sky.stars.precessed(&precession(now));
        let prepared = sky
            .stars
            .stars
            .iter()
            .zip(&star_dirs)
            .take_while(|(s, _)| s.mag <= 6.2)
            .map(|(s, &dir)| Prepared {
                dir,
                mag: s.mag as f64,
                light: light(s.mag as f64),
                tint: night_sky_core::stars::tint(s.bv),
                rate: 0.9 + (s.hr % 7) as f64 * 0.23,
                phase: s.hr as f64,
            })
            .collect();
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

    fn sky_now(&self, real: UnixMs) -> UnixMs {
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
        self.caption = Some(Timed {
            text: line,
            shown: real + 900,
            hold: 8_000,
        });
    }

    /// Where find `i` is, as a horizon vector.
    fn find_dir(&self, i: usize, now: UnixMs, hz: &Mat3, prec: &Mat3) -> Option<Vec3> {
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
        }
    }

    /// How narrow the view goes to show a find close up.
    fn zoom_for(&self, i: usize) -> f64 {
        match self.finds[i].target {
            Target::Body(Body::Moon) => 2.4,
            Target::Body(Body::Jupiter) | Target::Body(Body::Saturn) => 1.2,
            Target::Body(_) => 0.9,
            Target::Showpiece(p) => {
                (self.sky.lists.showpieces[p].size / 60.0 * 3.2).clamp(1.2, 40.0)
            }
            Target::Star(_) => 8.0,
            Target::Meteor(_) => self.camera.fov,
        }
    }

    /// How big find `i` looks, in pixels, at a field of view.
    fn apparent_radius(&self, i: usize, fov: f64) -> f64 {
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
            _ => 0.0,
        };
        degrees * ppd
    }

    /// Where the words about something go: clear of it, to its right.
    fn beside(&self, i: Option<usize>, fov: f64) -> f64 {
        let r = self.reticle_radius();
        let object = i.map(|i| self.apparent_radius(i, fov)).unwrap_or(0.0);
        (self.camera.width / 2.0 + r.max(object) + 34.0)
            .min(self.camera.width - 420.0)
            .max(24.0)
    }

    fn reticle_radius(&self) -> f64 {
        (self.camera.width.min(self.camera.height) * 0.055).max(26.0)
    }

    pub fn resize(&mut self, width: f64, height: f64) {
        self.camera.set_size(width, height);
    }

    fn input(&mut self, real: UnixMs) {
        self.last_input = real;
    }

    fn hunting(&self) -> bool {
        self.session.phase() == Phase::Hunt
    }

    pub fn key_pressed(&mut self, key: gdk::Key, real: UnixMs) -> bool {
        self.input(real);
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
        match key {
            gdk::Key::Left | gdk::Key::h => self.held.left = true,
            gdk::Key::Right | gdk::Key::l => self.held.right = true,
            gdk::Key::Up | gdk::Key::k => self.held.up = true,
            gdk::Key::Down | gdk::Key::j => self.held.down = true,
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
            gdk::Key::Left | gdk::Key::h => self.held.left = false,
            gdk::Key::Right | gdk::Key::l => self.held.right = false,
            gdk::Key::Up | gdk::Key::k => self.held.up = false,
            gdk::Key::Down | gdk::Key::j => self.held.down = false,
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
                let da = angle_between(here, a.1);
                let db = angle_between(here, b.1);
                // Skip the one already under the reticle.
                let da = if da < 1.0 { 999.0 } else { da };
                let db = if db < 1.0 { 999.0 } else { db };
                da.total_cmp(&db)
            });
        if let Some((_, v)) = next {
            let (alt, az) = alt_az(v);
            self.look = Some(Look {
                az,
                alt,
                fov: self.camera.fov.max(60.0),
                rate: 1.6,
            });
        } else if self
            .finds
            .iter()
            .any(|f| matches!(f.target, Target::Meteor(_)))
            && self
                .caught
                .iter()
                .zip(&self.finds)
                .any(|(c, f)| !c && matches!(f.target, Target::Meteor(_)))
        {
            self.hint = Some(Timed {
                text: "The last one is a meteor: watch, and press Space when one flies".into(),
                shown: real,
                hold: 5_000,
            });
        }
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
        self.session.found_one(real);
        let find = self.finds[i].clone();
        if let Err(e) = self.journal.mark_found(&find.id, &self.night) {
            eprintln!("night-sky: couldn't save what was found: {e}");
        }
        if !self.page.finds.contains(&find.name) {
            self.page.finds.push(find.name.clone());
        }
        self.save_page();
        let x = self.beside(Some(i), self.zoom_for(i));
        self.card = Some(Card {
            x,
            title: find.name,
            body: find.fact,
            shown: real,
        });
    }

    fn save_page(&self) {
        if let Err(e) = self.journal.save_night(&self.page) {
            eprintln!("night-sky: couldn't save tonight's page: {e}");
        }
    }

    fn dismiss_card(&mut self, real: UnixMs) {
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
            self.hint = Some(Timed {
                text: "That's tonight's sky.".into(),
                shown: real,
                hold: 6_000,
            });
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
        let tempo = self.session.tempo(real);
        self.steer(dt, tempo);
        let now = self.sky_now(real);
        let hz = horizon(self.observer, now);
        let prec = precession(now);
        self.update_catch(dt, tempo, real, now, &hz, &prec);
        self.spawn_meteors(real, now, &hz);
        self.draw(real, now, &hz, &prec, tempo)
    }

    fn entered(&mut self, phase: Phase, real: UnixMs) {
        match phase {
            Phase::Hunt => {
                self.hint = Some(Timed {
                    text: "Arrows to look around · hold Space to catch · Tab turns towards the next · Esc when you're done".into(),
                    shown: real + 2_500,
                    hold: 16_000,
                });
            }
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
                self.handoff = Some(handoff(&self.sky, self.observer, now, self.offset_s));
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
        let free = matches!(phase, Phase::Arrival | Phase::Hunt | Phase::Dimming);
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
            .filter(|&i| !self.caught[i])
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
            self.catch.progress += dt / (1.4 / tempo.max(0.3));
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
                self.caught_one(i, real);
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
            }
        }
    }

    fn draw(&mut self, real: UnixMs, now: UnixMs, hz: &Mat3, prec: &Mat3, tempo: f64) -> Frame {
        let cam = self.camera;
        let pitch = if cam.width > 2200.0 { 6.0 } else { 5.0 };
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
        for star in &self.prepared {
            if star.mag > shown {
                break;
            }
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
            let depth = 0.12 + 0.22 * (1.0 - v[2]).powi(2);
            let twinkle = 1.0
                + depth
                    * ((t * rate + star.phase).sin() * 0.6
                        + (t * rate * 1.7 + star.phase * 0.37).sin() * 0.4);
            let amount = star.light * fade * (low * twinkle) as f32;
            self.field.star(x, y, star.tint, amount);
        }

        // Showpieces' soft light.
        for piece in &self.sky.lists.showpieces {
            if piece.mag > limit + 0.5
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
            let radius = (piece.size / 60.0 / 2.0 * ppd).max(pitch as f64 * 0.8);
            let strength = (10f64.powf(-0.4 * (piece.mag - 3.0))).min(2.0) as f32;
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
                        0.07 * strength * twinkle as f32,
                    );
                }
            } else {
                let amount = (0.05 * strength).min(0.12);
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
                self.field.glow(x, y, radius, color, amount);
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
                        1.05,
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
                        self.lit_disc(x, y, radius, v, sun, phase_angle, color, 1.5, false);
                    } else {
                        let low = 0.4 + 0.6 * smoothstep(s.alt / 12.0);
                        self.field.star(
                            x,
                            y,
                            color,
                            light(s.position.magnitude) * 1.08 * low as f32,
                        );
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

        self.draw_reticle(real, tempo);

        let brightness = self.session.brightness(real);
        let veil = self.finale_veil(real);
        let frame_texture = self.field.texture((brightness * (1.0 - veil)) as f32 * 1.0);
        let mut texts = self.labels(real, now, hz, prec, brightness);
        texts.extend(self.words(real, brightness));
        Frame {
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
        if !matches!(self.session.phase(), Phase::Hunt | Phase::Arrival) || self.card.is_some() {
            return;
        }
        let r = self.reticle_radius();
        let (cx, cy) = (self.camera.width / 2.0, self.camera.height / 2.0);
        let has = self.catch.target.is_some();
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
                    let size = if name.len() == 1 { 13.0 } else { 11.0 };
                    out.push(Text::new(x, y - 22.0, name, size, 0.32 * brightness).centred());
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
                let lx = self.beside(Some(i), cam.fov) - 20.0;
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
        if let Some(c) = &self.caption {
            let a = envelope(real - c.shown, 1_500, c.hold, 2_000);
            out.push(
                Text::new(w / 2.0, 40.0, c.text.clone(), 17.0, 0.85 * a * text_light)
                    .centred()
                    .wrap(w * 0.8),
            );
        }
        if let Some(card) = &self.card {
            let a = envelope(real - card.shown, 700, 60_000, 1_000);
            let x = card.x;
            let y = h / 2.0 - 44.0;
            out.push(Text::new(x, y, card.title.clone(), 20.0, 0.95 * a).bold());
            out.push(Text::new(x, y + 32.0, card.body.clone(), 15.0, 0.8 * a).wrap(380.0));
            out.push(Text::new(
                x,
                y + 32.0 + 22.0 * 4.0,
                "Space to carry on",
                12.0,
                0.35 * a,
            ));
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
        if matches!(self.session.phase(), Phase::Hunt | Phase::Arrival) && !self.finds.is_empty() {
            // Tonight's set, as quiet dots: open until found.
            let dots: String = self
                .caught
                .iter()
                .map(|c| if *c { "●" } else { "○" })
                .collect::<Vec<_>>()
                .join(" ");
            out.push(Text::new(28.0, h - 40.0, dots, 11.0, 0.32 * brightness));
        }
        out
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
