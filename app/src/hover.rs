//! What's under the pointer. Moving the mouse over the sky names whatever
//! it's near, in a small line beside the pointer; a click on it tells more,
//! on a card. One of tonight's finds not yet caught keeps its secret: the
//! click turns the view to it instead, so it can be caught.

use crate::game::{Card, Game, envelope};
use crate::view::{Point, Text};
use westering_core::catalogues::Kind;
use westering_core::coords::{Mat3, alt_az, apply, from_alt_az, unit};
use westering_core::ephem::Body;
use westering_core::finds::Target;
use westering_core::sky::see;
use westering_core::time::{UnixMs, civil_date};

/// Something the pointer is over.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Hovered {
    x: f64,
    y: f64,
    /// How big it is on the screen, for the ring round it.
    radius: f64,
    name: String,
    kind: String,
    /// What a click tells, if there's more to tell.
    more: Option<String>,
    /// One of tonight's finds, which a click turns to instead.
    find: Option<usize>,
}

/// How close the pointer has to be, in pixels, beyond the thing's own size.
const REACH: f64 = 20.0;
/// Stars brighter than this answer to the pointer even without a name.
const NAMELESS: f32 = 4.5;
/// Stars drawn in a neighbour's figure, and the constellation they're
/// really in: nu Puppis in Carina's (the old ship Argo, drawn as one), and
/// delta and nu Ophiuchi in the Serpent's, which passes through Ophiuchus's
/// hands.
const BORROWED: [(u16, &str); 3] = [(2451, "Pup"), (6056, "Oph"), (6698, "Oph")];
/// A hint fades once the pointer has been still this long.
const STILL_MS: UnixMs = 4_000;

fn kind_of(kind: Kind) -> &'static str {
    match kind {
        Kind::Cluster => "Star cluster",
        Kind::Galaxy => "Galaxy",
        Kind::Nebula => "Nebula",
        Kind::Double => "Double star",
        Kind::Star => "Star",
        Kind::Dark => "Dark cloud",
        Kind::Cloud => "Star cloud",
        Kind::Asterism => "Asterism",
    }
}

impl Game {
    pub fn pointer_moved(&mut self, x: f64, y: f64, real: UnixMs) {
        if self
            .pointer
            .is_none_or(|(px, py, _)| (px - x).abs() + (py - y).abs() > 1.0)
        {
            self.pointer = Some((x, y, real));
            // Looking around with the pointer is being busy with the sky.
            self.last_input = real;
        }
    }

    pub fn pointer_left(&mut self) {
        self.pointer = None;
    }

    /// Whether hints make sense now: during the hunt, with nothing else
    /// asking for attention.
    fn hinting(&self) -> bool {
        matches!(
            self.session.phase(),
            westering_core::session::Phase::Weights
                | westering_core::session::Phase::Hunt
                | westering_core::session::Phase::Dimming
        ) && self.card.is_none()
            && self.drawing.is_none()
            && self.tour.is_none()
            && !self.placing()
    }

    /// Whether a click where the pointer is would do something, for the
    /// pointer's shape.
    pub fn clickable_at(&self, x: f64, y: f64, real: UnixMs) -> bool {
        if self.by_day() {
            return self.day_at(x, y).is_some();
        }
        if !self.hinting() {
            return false;
        }
        let now = self.sky_now(real);
        let hz = westering_core::coords::horizon(self.observer, now);
        let prec = westering_core::coords::precession(now);
        self.pick(x, y, now, &hz, &prec)
            .is_some_and(|h| h.find.is_some() || h.more.is_some())
    }

    /// The nearest thing to a point on the screen, if anything's near.
    pub(crate) fn pick(
        &self,
        x: f64,
        y: f64,
        now: UnixMs,
        hz: &Mat3,
        prec: &Mat3,
    ) -> Option<Hovered> {
        let cam = self.camera;
        let ppd = cam.px_per_degree();
        let mut best: Option<(f64, Hovered)> = None;
        let mut consider = |sx: f64, sy: f64, radius: f64, h: Hovered| {
            let d = ((sx - x).powi(2) + (sy - y).powi(2)).sqrt() - radius;
            if d < REACH && best.as_ref().is_none_or(|(bd, _)| d < *bd) {
                best = Some((
                    d,
                    Hovered {
                        x: sx,
                        y: sy,
                        radius,
                        ..h
                    },
                ));
            }
        };
        let up = |v: [f64; 3]| alt_az(v).0 > 0.0;
        let year = civil_date(now, self.offset_s).0;

        // Tonight's finds first: they're what the hunt is about.
        let mut taken: Vec<String> = Vec::new();
        for i in 0..self.finds.len() {
            if matches!(self.finds[i].target, Target::Meteor(_) | Target::Figure(_)) {
                continue;
            }
            let Some(v) = self.find_dir(i, now, hz, prec).filter(|v| up(*v)) else {
                continue;
            };
            let Some((sx, sy)) = cam.project(v) else {
                continue;
            };
            let f = &self.finds[i];
            taken.push(f.name.clone());
            let caught = self.caught[i];
            consider(
                sx,
                sy,
                3.0,
                Hovered {
                    x: 0.0,
                    y: 0.0,
                    radius: 0.0,
                    name: f.name.clone(),
                    kind: self.kind_word(i).to_owned(),
                    more: caught.then(|| f.fact.clone()),
                    find: (!caught).then_some(i),
                },
            );
        }

        // The Moon and the planets.
        for body in [
            Body::Moon,
            Body::Mercury,
            Body::Venus,
            Body::Mars,
            Body::Jupiter,
            Body::Saturn,
        ] {
            if self.finds.iter().any(|f| f.target == Target::Body(body)) {
                continue;
            }
            let s = see(body, self.observer, now);
            if s.alt < 0.0 {
                continue;
            }
            if let Some((sx, sy)) = cam.project(from_alt_az(s.alt, s.az)) {
                let radius = s.position.diameter / 7200.0 * ppd;
                let more = self
                    .sky
                    .lists
                    .bodies
                    .iter()
                    .find(|b| b.id == body.id())
                    .and_then(|b| b.facts.first().cloned());
                consider(
                    sx,
                    sy,
                    radius,
                    Hovered {
                        x: 0.0,
                        y: 0.0,
                        radius: 0.0,
                        name: {
                            // "the Moon" as a title reads "The Moon".
                            let n = body.name();
                            let mut c = n.chars();
                            c.next()
                                .map(|f| f.to_uppercase().collect::<String>() + c.as_str())
                                .unwrap_or_default()
                        },
                        kind: if body == Body::Moon {
                            "Our Moon"
                        } else {
                            "Planet"
                        }
                        .to_owned(),
                        more,
                        find: None,
                    },
                );
            }
        }

        // Any star bright enough to pick out: which figure it belongs to.
        let named: Vec<u16> = self.sky.lists.stars.iter().map(|s| s.hr).collect();
        for (k, star) in self.sky.stars.stars.iter().enumerate() {
            if star.mag > NAMELESS {
                break;
            }
            if named.contains(&star.hr) {
                continue;
            }
            let v = apply(hz, self.star_dirs[k]);
            if !up(v) {
                continue;
            }
            let Some((sx, sy)) = cam.project(v) else {
                continue;
            };
            if (sx - x).abs() > 40.0 || (sy - y).abs() > 40.0 {
                continue;
            }
            // A few figures borrow stars from their neighbours; those stars
            // are named for the constellation they're really in.
            let home = BORROWED
                .iter()
                .find(|(hr, _)| *hr == star.hr)
                .map(|(_, abbrev)| *abbrev);
            let figure = self
                .sky
                .figures
                .iter()
                .find(|f| match home {
                    Some(abbrev) => f.abbrev == abbrev,
                    None => f.edges.iter().any(|&(a, b)| a == star.hr || b == star.hr),
                })
                .map(|f| f.name.split(',').next().unwrap_or(&f.name).to_owned());
            let brightness = match star.mag {
                m if m < 1.5 => "One of the brightest stars",
                m if m < 3.0 => "A bright star",
                _ => "A star you can see from a town",
            };
            consider(
                sx,
                sy,
                1.5,
                Hovered {
                    x: 0.0,
                    y: 0.0,
                    radius: 0.0,
                    name: match &figure {
                        Some(f) => format!("A star in {f}"),
                        None => "A star".to_owned(),
                    },
                    kind: brightness.to_owned(),
                    more: None,
                    find: None,
                },
            );
        }

        // Named stars.
        for s in &self.sky.lists.stars {
            if taken.iter().any(|n| n == &s.name) {
                continue;
            }
            let Some(idx) = self.sky.stars.index_of(s.hr) else {
                continue;
            };
            let v = apply(hz, self.star_dirs[idx]);
            if !up(v) {
                continue;
            }
            if let Some((sx, sy)) = cam.project(v) {
                consider(
                    sx,
                    sy,
                    2.0,
                    Hovered {
                        x: 0.0,
                        y: 0.0,
                        radius: 0.0,
                        name: s.name.clone(),
                        kind: "Star".into(),
                        more: s.describe(year),
                        find: None,
                    },
                );
            }
        }

        // Clusters, galaxies and nebulae bright enough to show at this zoom.
        let limit = westering_core::sky::limiting_magnitude(see(Body::Sun, self.observer, now).alt);
        let reach = limit + crate::game::gather(cam.fov) + 0.5;
        for p in &self.sky.lists.showpieces {
            if p.mag > reach || taken.iter().any(|n| n == &p.name) {
                continue;
            }
            let v = apply(hz, apply(prec, unit(p.ra, p.dec)));
            if !up(v) {
                continue;
            }
            if let Some((sx, sy)) = cam.project(v) {
                let radius = (p.size / 120.0 * ppd).min(60.0);
                consider(
                    sx,
                    sy,
                    radius,
                    Hovered {
                        x: 0.0,
                        y: 0.0,
                        radius: 0.0,
                        name: p.name.clone(),
                        kind: kind_of(p.kind).to_owned(),
                        more: Some(p.fact.clone()),
                        find: None,
                    },
                );
            }
        }

        // Tonight's weights, and stars carrying someone's name.
        for w in &self.page.weights {
            let v = apply(hz, unit(w.ra, w.dec));
            if let (Some((sx, sy)), Some(weight)) = (cam.project(v), self.journal.weight(w.weight))
            {
                consider(
                    sx,
                    sy,
                    3.0,
                    Hovered {
                        x: 0.0,
                        y: 0.0,
                        radius: 0.0,
                        name: weight.text.clone(),
                        kind: "Set down tonight".into(),
                        more: Some(
                            "Something you set down tonight. At the end of the visit you'll watch it set in the west."
                                .into(),
                        ),
                        find: None,
                    },
                );
            }
        }
        for person in &self.journal.people {
            for &hr in &person.stars {
                let Some(idx) = self.sky.stars.index_of(hr) else {
                    continue;
                };
                let v = apply(hz, self.star_dirs[idx]);
                if !up(v) {
                    continue;
                }
                if let Some((sx, sy)) = cam.project(v) {
                    consider(
                        sx,
                        sy,
                        2.0,
                        Hovered {
                            x: 0.0,
                            y: 0.0,
                            radius: 0.0,
                            name: format!("{}'s star", person.name),
                            kind: "A star with a name".into(),
                            more: None,
                            find: None,
                        },
                    );
                }
            }
        }
        best.map(|(_, h)| h)
    }

    /// The hint beside the pointer, and a faint ring round what it names.
    /// It waits for the pointer to settle a moment, so sweeping across the
    /// sky doesn't flicker names.
    pub(crate) fn hover_frame(
        &mut self,
        real: UnixMs,
        now: UnixMs,
        hz: &Mat3,
        prec: &Mat3,
    ) -> Vec<Text> {
        let mut out = Vec::new();
        let Some((px, py, since)) = self.pointer.filter(|_| self.hinting()) else {
            self.hovered = None;
            return out;
        };
        self.hovered = self.pick(px, py, now, hz, prec);
        let Some(h) = &self.hovered else {
            return out;
        };
        // What's in the ring already has its own label.
        if h.find.is_some() && h.find == self.catch.target {
            return out;
        }
        // Up a moment after the pointer settles, gone a while after it stops.
        let a = envelope(real - since - 150, 200, STILL_MS, 1_200);
        if a <= 0.0 {
            return out;
        }
        let r = h.radius.max(3.0) + 7.0;
        for k in 0..18 {
            let t = k as f64 / 18.0 * std::f64::consts::TAU;
            self.marks.push(Point {
                x: h.x + r * t.cos(),
                y: h.y + r * t.sin(),
                radius: 1.0,
                color: [0.85, 0.9, 1.0],
                alpha: (0.45 * a) as f32,
                halo: 0.0,
            });
        }
        // Beside the pointer, kept on the screen.
        let (w, hgt) = (self.camera.width, self.camera.height);
        // Clear of the Tonight list on the right when it's showing.
        let right = if self.show_tonight() { w - 310.0 } else { w };
        let x = if px + 280.0 > right {
            px - 270.0
        } else {
            px + 18.0
        };
        let y = (py + 14.0).min(hgt - 60.0);
        out.push(Text::new(x, y, h.name.clone(), 14.0, 0.92 * a).bold());
        let action = match (h.find.is_some(), h.more.is_some()) {
            (true, _) => format!("{} · one of tonight's finds · click to turn to it", h.kind),
            (false, true) => format!("{} · click for more", h.kind),
            (false, false) => h.kind.clone(),
        };
        out.push(Text::new(x, y + 19.0, action, 12.0, 0.6 * a).color([0.82, 0.86, 0.95]));
        out
    }

    /// A click on something named: more about it, or a turn to one of
    /// tonight's finds. Says whether it was on something.
    pub(crate) fn click_hovered(&mut self, x: f64, y: f64, real: UnixMs) -> bool {
        if !self.hinting() {
            return false;
        }
        let now = self.sky_now(real);
        let hz = westering_core::coords::horizon(self.observer, now);
        let prec = westering_core::coords::precession(now);
        let Some(h) = self.pick(x, y, now, &hz, &prec) else {
            return false;
        };
        if let Some(i) = h.find {
            self.turn_to(i, real);
            return true;
        }
        let Some(body) = h.more else {
            return false;
        };
        self.card = Some(Card {
            picture: None,
            find: None,
            x: self.beside_centre(),
            kicker: h.kind,
            title: h.name,
            body,
            shown: real,
        });
        // Look at it, without zooming.
        let v = self.camera.unproject(h.x, h.y);
        let (alt, az) = alt_az(v);
        self.look = Some(crate::game::Look {
            az,
            alt,
            fov: self.camera.fov,
            rate: 1.8,
        });
        self.track = None;
        true
    }
}
