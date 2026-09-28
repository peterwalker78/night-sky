//! The last line of a visit: one real thing to go outside and see.

use crate::coords::{Observer, compass, separation};
use crate::coords::{alt_az, apply, horizon, precession};
use crate::ephem::Body;
use crate::sky::{Sky, crossing, see, sun_altitude};
use crate::time::{HOUR, UnixMs, clock};

#[derive(Clone, Debug, PartialEq)]
pub struct Handoff {
    pub line: String,
    /// Where to turn the view for it.
    pub alt: f64,
    pub az: f64,
    /// The object, if it is up now.
    pub id: Option<String>,
}

/// "low in the east", "high in the south-west", "almost overhead".
pub fn whereabouts(alt: f64, az: f64) -> String {
    let dir = compass(az);
    if alt > 75.0 {
        "almost overhead".to_owned()
    } else if alt > 50.0 {
        format!("high in the {dir}")
    } else if alt < 20.0 {
        format!("low in the {dir}")
    } else {
        format!("in the {dir}")
    }
}

struct Candidate {
    id: String,
    name: String,
    mag: f64,
    alt: f64,
    az: f64,
    ra: f64,
    dec: f64,
    planet: bool,
}

fn bright_things(sky: &Sky, observer: Observer, at: UnixMs) -> Vec<Candidate> {
    let mut out = Vec::new();
    for body in [
        Body::Moon,
        Body::Venus,
        Body::Mars,
        Body::Jupiter,
        Body::Saturn,
        Body::Mercury,
    ] {
        let s = see(body, observer, at);
        out.push(Candidate {
            id: body.id().into(),
            name: body.name().into(),
            mag: s.position.magnitude,
            alt: s.alt,
            az: s.az,
            ra: s.ra,
            dec: s.dec,
            planet: body != Body::Moon,
        });
    }
    let hz = horizon(observer, at);
    let prec = precession(at);
    for named in &sky.lists.stars {
        let Some(star) = sky.stars.get(named.hr) else {
            continue;
        };
        if star.mag > 1.0 {
            continue;
        }
        let v = apply(&prec, star.dir);
        let (alt, az) = alt_az(apply(&hz, v));
        let (ra, dec) = crate::coords::angles(v);
        out.push(Candidate {
            id: format!("star:{}", named.hr),
            name: named.name.clone(),
            mag: star.mag as f64,
            alt,
            az,
            ra,
            dec,
            planet: false,
        });
    }
    out
}

fn pick(things: &[Candidate], min_alt: f64) -> Option<&Candidate> {
    let up = |c: &&Candidate| c.alt > min_alt;
    things
        .iter()
        .filter(up)
        .filter(|c| c.planet && c.mag < 1.5)
        .min_by(|a, b| a.mag.total_cmp(&b.mag))
        .or_else(|| things.iter().filter(up).find(|c| c.id == "moon"))
        .or_else(|| {
            things
                .iter()
                .filter(up)
                .filter(|c| !c.planet && c.id != "moon")
                .min_by(|a, b| a.mag.total_cmp(&b.mag))
        })
}

fn describe(c: &Candidate, things: &[Candidate]) -> String {
    let place = whereabouts(c.alt, c.az);
    if c.id == "moon" {
        return format!(
            "The Moon is up {place}. Give your eyes twenty minutes, then look for the stars around it."
        );
    }
    let brighter_nearby = things.iter().any(|o| {
        o.id != c.id && o.alt > 0.0 && o.mag < c.mag && separation(o.ra, o.dec, c.ra, c.dec) < 90.0
    });
    let how = if c.planet {
        if brighter_nearby {
            "a steady light that doesn't twinkle"
        } else {
            "the brightest thing there, and it doesn't twinkle"
        }
    } else if brighter_nearby {
        "one of the brightest stars"
    } else {
        "the brightest star there"
    };
    format!(
        "{} is up {place}, {how}. Give your eyes twenty minutes.",
        c.name
    )
}

fn hhmm(at: UnixMs, offset_s: i32) -> String {
    let (h, m) = clock(at, offset_s);
    format!("{h:02}:{m:02}")
}

/// Tonight's hand-off. `at` is the real clock, not the time-lapse.
pub fn handoff(sky: &Sky, observer: Observer, at: UnixMs, offset_s: i32) -> Handoff {
    let sun = sun_altitude(observer, at);
    if sun > -6.0 {
        let dark = crossing(at, 24 * HOUR, -8.0, false, |t| sun_altitude(observer, t));
        if let Some(dark) = dark {
            let later = bright_things(sky, observer, dark);
            if let Some(c) = pick(&later, 12.0) {
                let when = hhmm(dark, offset_s);
                let line = format!(
                    "It'll be dark by about {when}. Then look {}: {} will be there, if it's clear.",
                    whereabouts(c.alt, c.az),
                    c.name
                );
                return Handoff {
                    line,
                    alt: c.alt,
                    az: c.az,
                    id: None,
                };
            }
        }
        return Handoff {
            line: "Come back after dark, and give your eyes twenty minutes outside.".into(),
            alt: 20.0,
            az: 180.0,
            id: None,
        };
    }
    let now = bright_things(sky, observer, at);
    if let Some(c) = pick(&now, 10.0) {
        return Handoff {
            line: format!("If it's clear: {}", describe(c, &now)),
            alt: c.alt,
            az: c.az,
            id: Some(c.id.clone()),
        };
    }
    for body in [
        Body::Moon,
        Body::Venus,
        Body::Jupiter,
        Body::Mars,
        Body::Saturn,
    ] {
        let rise = crossing(at, 3 * HOUR, 10.0, true, |t| see(body, observer, t).alt);
        if let Some(rise) = rise {
            let s = see(body, observer, rise);
            let name = body.name();
            let name = if body == Body::Moon { "The Moon" } else { name };
            return Handoff {
                line: format!(
                    "{name} will be up {} by {}. Give your eyes twenty minutes before then.",
                    whereabouts(s.alt, s.az),
                    hhmm(rise, offset_s)
                ),
                alt: 8.0,
                az: s.az,
                id: None,
            };
        }
    }
    Handoff {
        line: "If it's clear, just look up. Give your eyes twenty minutes.".into(),
        alt: 60.0,
        az: 180.0,
        id: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::time::midnight_utc;

    const LONDON: Observer = Observer {
        lat: 51.5072,
        lon: -0.1276,
    };

    #[test]
    fn a_dark_night_names_something_that_is_really_up() {
        let sky = Sky::bundled();
        for day in 1..=28 {
            let at = midnight_utc(2026, 11, day) + 22 * HOUR;
            let h = handoff(&sky, LONDON, at, 0);
            assert!(h.line.contains("twenty minutes"), "{}", h.line);
            assert!(!h.line.contains("grateful"));
            if h.id.is_some() {
                assert!(h.alt > 10.0, "{} at {}", h.line, h.alt);
            }
        }
    }

    #[test]
    fn daytime_points_to_the_evening() {
        let sky = Sky::bundled();
        let at = midnight_utc(2026, 11, 3) + 13 * HOUR;
        let h = handoff(&sky, LONDON, at, 0);
        assert!(h.line.starts_with("It'll be dark by about 1"), "{}", h.line);
    }

    #[test]
    fn whereabouts_read_naturally() {
        assert_eq!(whereabouts(12.0, 91.0), "low in the east");
        assert_eq!(whereabouts(35.0, 180.0), "in the south");
        assert_eq!(whereabouts(80.0, 10.0), "almost overhead");
    }
}
