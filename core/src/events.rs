//! The sky's calendar: things ahead worth looking forward to. Full Moons,
//! meteor shower peaks, planets at their closest and the Moon passing a
//! planet, all worked out from the same arithmetic as the sky itself.

use crate::catalogues::Shower;
use crate::coords::separation;
use crate::ephem::{Body, moon_age, position, rev, solar_longitude};
use crate::time::{DAY, HOUR, UnixMs};

#[derive(Clone, Debug, PartialEq)]
pub enum Kind {
    FullMoon,
    NewMoon,
    Shower { id: String, zhr: u32 },
    Opposition(Body),
    Pairing(Body),
}

#[derive(Clone, Debug, PartialEq)]
pub struct SkyEvent {
    pub at: UnixMs,
    pub kind: Kind,
    /// How it reads in a sentence: "the full Moon", "the Geminids".
    pub title: String,
}

/// Everything from `from` for `days` days, soonest first.
pub fn upcoming(showers: &[Shower], from: UnixMs, days: i64) -> Vec<SkyEvent> {
    let to = from + days * DAY;
    let mut out = Vec::new();
    moon_phases(from, to, &mut out);
    shower_peaks(showers, from, to, &mut out);
    for body in [Body::Mars, Body::Jupiter, Body::Saturn] {
        opposition(body, from, to, &mut out);
    }
    for body in [Body::Venus, Body::Mars, Body::Jupiter, Body::Saturn] {
        pairings(body, from, to, &mut out);
    }
    out.sort_by_key(|e| e.at);
    out
}

/// The moment in `lo..hi` where `f` climbs through zero, by halving.
fn refine(mut lo: UnixMs, mut hi: UnixMs, f: impl Fn(UnixMs) -> f64) -> UnixMs {
    while hi - lo > 60_000 {
        let mid = (lo + hi) / 2;
        if f(mid) < 0.0 {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    hi
}

/// Signed distance from `target` on a circle of 360, in -180..180.
fn around(value: f64, target: f64) -> f64 {
    rev(value - target + 180.0) - 180.0
}

fn moon_phases(from: UnixMs, to: UnixMs, out: &mut Vec<SkyEvent>) {
    let step = 6 * HOUR;
    for (target, kind, title) in [
        (0.5, Kind::FullMoon, "the full Moon"),
        (0.0, Kind::NewMoon, "the new Moon"),
    ] {
        let f = |t: UnixMs| around(moon_age(t) * 360.0, target * 360.0);
        let mut t = from;
        while t < to {
            let (a, b) = (f(t), f(t + step));
            if a < 0.0 && b >= 0.0 && b - a < 90.0 {
                out.push(SkyEvent {
                    at: refine(t, t + step, f),
                    kind: kind.clone(),
                    title: title.to_owned(),
                });
            }
            t += step;
        }
    }
}

fn shower_peaks(showers: &[Shower], from: UnixMs, to: UnixMs, out: &mut Vec<SkyEvent>) {
    let step = 12 * HOUR;
    for s in showers {
        let f = |t: UnixMs| around(solar_longitude(t), s.peak_sol);
        let mut t = from;
        while t < to {
            let (a, b) = (f(t), f(t + step));
            if a < 0.0 && b >= 0.0 && b - a < 10.0 {
                out.push(SkyEvent {
                    at: refine(t, t + step, f),
                    kind: Kind::Shower {
                        id: s.id.clone(),
                        zhr: s.zhr,
                    },
                    title: format!("the {}", s.name),
                });
            }
            t += step;
        }
    }
}

/// A planet opposite the Sun, closest and brightest, up all night.
fn opposition(body: Body, from: UnixMs, to: UnixMs, out: &mut Vec<SkyEvent>) {
    let elong = |t: UnixMs| position(body, t).elongation;
    let mut t = from + DAY;
    while t < to {
        let (a, b, c) = (elong(t - DAY), elong(t), elong(t + DAY));
        if b > 170.0 && b >= a && b > c {
            out.push(SkyEvent {
                at: t,
                kind: Kind::Opposition(body),
                title: format!("{} at its closest", body.name()),
            });
        }
        t += DAY;
    }
}

/// The Moon passing within three degrees of a bright planet.
fn pairings(body: Body, from: UnixMs, to: UnixMs, out: &mut Vec<SkyEvent>) {
    let step = 2 * HOUR;
    let gap = |t: UnixMs| {
        let (m, p) = (position(Body::Moon, t), position(body, t));
        separation(m.ra, m.dec, p.ra, p.dec)
    };
    let mut t = from + step;
    while t < to {
        let (a, b, c) = (gap(t - step), gap(t), gap(t + step));
        // Only when both are far enough from the Sun to be seen in a dark sky.
        let seen =
            || position(body, t).elongation > 25.0 && position(Body::Moon, t).elongation > 25.0;
        if b < 3.0 && b <= a && b < c && seen() {
            out.push(SkyEvent {
                at: t,
                kind: Kind::Pairing(body),
                title: format!("the Moon beside {}", body.name()),
            });
        }
        t += step;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalogues::Catalogues;
    use crate::time::{MINUTE, midnight_utc};

    #[test]
    fn full_moons_land_on_the_published_dates() {
        // Full Moon: 26 October 2026, 04:12 UTC (US Naval Observatory).
        let events = upcoming(&[], midnight_utc(2026, 10, 10), 30);
        let full = events
            .iter()
            .find(|e| e.kind == Kind::FullMoon)
            .expect("a full Moon");
        let expected = midnight_utc(2026, 10, 26) + 4 * HOUR + 12 * MINUTE;
        assert!(
            (full.at - expected).abs() < 3 * HOUR,
            "{} hours off",
            (full.at - expected) / HOUR
        );
    }

    #[test]
    fn the_geminids_peak_in_mid_december() {
        let showers = Catalogues::bundled().showers;
        let events = upcoming(&showers, midnight_utc(2026, 11, 20), 40);
        let gem = events
            .iter()
            .find(|e| matches!(&e.kind, Kind::Shower { id, .. } if id == "geminids"))
            .expect("Geminids");
        let (_, m, d) = crate::time::civil_date(gem.at, 0);
        assert_eq!(m, 12);
        assert!((13..=15).contains(&d), "peak on {d} December");
    }
}
