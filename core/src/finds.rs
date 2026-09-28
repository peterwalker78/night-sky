//! Tonight's handful of finds, chosen from what is really up. The set depends
//! only on the night, the place and what has been found before, so opening
//! the app again the same night never rerolls it.

use crate::catalogues::Kind;
use crate::coords::{Observer, alt_az, apply, horizon, precession, unit};
use crate::ephem::Body;
use crate::sky::{Sky, limiting_magnitude, see, sun_altitude};
use crate::time::{HOUR, UnixMs, civil_date};

#[derive(Clone, Debug, PartialEq)]
pub enum Target {
    Body(Body),
    /// Index into the showpiece list.
    Showpiece(usize),
    /// A catalogue star, by HR number.
    Star(u16),
    /// Index into the shower list.
    Meteor(usize),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Find {
    pub id: String,
    pub target: Target,
    pub name: String,
    pub fact: String,
}

pub const MOST: usize = 7;

/// The night a moment belongs to: until noon, it is still last night.
pub fn night_of(at: UnixMs, offset_s: i32) -> (i32, u32, u32) {
    civil_date(at - 12 * HOUR, offset_s)
}

pub fn night_key(night: (i32, u32, u32)) -> String {
    format!("{:04}-{:02}-{:02}", night.0, night.1, night.2)
}

/// A stable pseudo-random number for a night and a name.
pub fn stable_hash(night: (i32, u32, u32), name: &str) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    let key = format!("{}-{}-{}:{}", night.0, night.1, night.2, name);
    for b in key.bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(0x100_0000_01b3);
    }
    h
}

/// Groups digits in threes: 384400 becomes "384,400".
pub fn thousands(n: u64) -> String {
    let s = n.to_string();
    let mut out = String::new();
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (s.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    out
}

fn body_fact(body: Body, distance: f64, phase_name: &str) -> String {
    if body == Body::Moon {
        let km = (distance * 6378.14 / 100.0).round() as u64 * 100;
        return format!(
            "{phase_name}, {} km away tonight. It always turns the same face towards us.",
            thousands(km)
        );
    }
    let minutes = (distance * 8.3168).round() as u64;
    let light = if minutes < 2 {
        "Its light left it about a minute ago.".to_owned()
    } else if minutes < 120 {
        format!("Its light left it {minutes} minutes ago.")
    } else {
        format!(
            "Its light left it about {} hours ago.",
            (minutes as f64 / 60.0).round()
        )
    };
    let more = match body {
        Body::Mercury => "It never strays far from the Sun, so it only shows near dawn or dusk.",
        Body::Venus => "It spins so slowly that its day is longer than its year.",
        Body::Mars => "A day on Mars lasts about forty minutes longer than ours.",
        Body::Jupiter => "More than a thousand Earths would fit inside it.",
        Body::Saturn => "It is less dense than water.",
        _ => "",
    };
    format!("{light} {more}").trim().to_owned()
}

/// Tonight's set. `found_before` says whether an id has been found on an
/// earlier night; unfound things are preferred.
pub fn tonight(
    sky: &Sky,
    observer: Observer,
    at: UnixMs,
    offset_s: i32,
    found_before: &dyn Fn(&str) -> bool,
) -> Vec<Find> {
    let night = night_of(at, offset_s);
    let sun_alt = sun_altitude(observer, at);
    let limit = limiting_magnitude(sun_alt);
    let mut out = Vec::new();

    let moon = see(Body::Moon, observer, at);
    if moon.alt > 3.0 {
        let age = crate::ephem::moon_age(at);
        out.push(Find {
            id: "moon".into(),
            target: Target::Body(Body::Moon),
            name: "The Moon".into(),
            fact: body_fact(
                Body::Moon,
                moon.position.distance,
                crate::ephem::moon_phase_name(age),
            ),
        });
    }
    for body in [
        Body::Mercury,
        Body::Venus,
        Body::Mars,
        Body::Jupiter,
        Body::Saturn,
    ] {
        let seen = see(body, observer, at);
        if seen.alt > 5.0 && seen.position.magnitude <= limit.min(3.0) {
            out.push(Find {
                id: body.id().into(),
                target: Target::Body(body),
                name: body.name().into(),
                fact: body_fact(body, seen.position.distance, ""),
            });
        }
    }

    let horizon = horizon(observer, at);
    let prec = precession(at);
    let alt_of = |ra: f64, dec: f64| alt_az(apply(&horizon, apply(&prec, unit(ra, dec)))).0;
    let order = |id: &str| (found_before(id), stable_hash(night, id));

    let mut pieces: Vec<(usize, String)> = sky
        .lists
        .showpieces
        .iter()
        .enumerate()
        .filter(|(_, p)| p.kind != Kind::Dark && p.mag <= limit && alt_of(p.ra, p.dec) > 15.0)
        .map(|(i, p)| (i, format!("showpiece:{}", p.id)))
        .collect();
    pieces.sort_by_key(|(_, id)| order(id));
    let room = if out.len() >= 4 { 1 } else { 2 };
    for (i, id) in pieces.into_iter().take(room) {
        let p = &sky.lists.showpieces[i];
        out.push(Find {
            id,
            target: Target::Showpiece(i),
            name: p.name.clone(),
            fact: p.fact.clone(),
        });
    }

    let mut named: Vec<(u16, String)> = sky
        .lists
        .stars
        .iter()
        .filter(|s| s.fact.is_some())
        .filter_map(|s| {
            let star = sky.stars.get(s.hr)?;
            let (alt, _) = alt_az(apply(&horizon, apply(&prec, star.dir)));
            (alt > 15.0 && (star.mag as f64) <= limit).then(|| (s.hr, format!("star:{}", s.hr)))
        })
        .collect();
    named.sort_by_key(|(_, id)| order(id));
    if let Some((hr, id)) = named.into_iter().next() {
        let s = sky.lists.star_name(hr).expect("listed star");
        out.push(Find {
            id,
            target: Target::Star(hr),
            name: s.name.clone(),
            fact: s.fact.clone().unwrap_or_default(),
        });
    }

    let (_, month, day) = civil_date(at, offset_s);
    if sun_alt < -12.0
        && let Some((i, shower)) = sky
            .lists
            .showers
            .iter()
            .enumerate()
            .filter(|(_, s)| s.zhr >= 10 && s.active_on(month, day) && alt_of(s.ra, s.dec) > 0.0)
            .max_by_key(|(_, s)| s.zhr)
    {
        out.push(Find {
            id: format!("meteor:{}", shower.id),
            target: Target::Meteor(i),
            name: format!("A {} meteor", shower.name.trim_end_matches('s')),
            fact: format!(
                "A grain of dust burning up about a hundred kilometres overhead, arriving at {} km a second.",
                shower.speed
            ),
        });
    }

    while out.len() > MOST {
        match out
            .iter()
            .rposition(|f| matches!(f.target, Target::Showpiece(_)))
        {
            Some(i) => {
                out.remove(i);
            }
            None => {
                out.truncate(MOST);
            }
        }
    }
    out
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
    fn a_dark_night_has_a_handful_of_finds_and_keeps_them() {
        let sky = Sky::bundled();
        let at = midnight_utc(2026, 12, 14) + 21 * HOUR;
        let never = |_: &str| false;
        let finds = tonight(&sky, LONDON, at, 0, &never);
        assert!((4..=MOST).contains(&finds.len()), "{finds:#?}");
        assert!(
            finds.iter().any(|f| f.id == "meteor:geminids"),
            "Geminids night: {finds:#?}"
        );
        let later = tonight(&sky, LONDON, at + 5 * 60_000, 0, &never);
        assert_eq!(
            finds.iter().map(|f| &f.id).collect::<Vec<_>>(),
            later.iter().map(|f| &f.id).collect::<Vec<_>>()
        );
    }

    #[test]
    fn found_things_give_way_to_new_ones() {
        let sky = Sky::bundled();
        let at = midnight_utc(2026, 12, 14) + 21 * HOUR;
        let first = tonight(&sky, LONDON, at, 0, &|_| false);
        let piece = first
            .iter()
            .find(|f| matches!(f.target, Target::Showpiece(_)))
            .expect("a showpiece")
            .id
            .clone();
        let again = tonight(&sky, LONDON, at, 0, &|id| id == piece);
        assert!(again.iter().all(|f| f.id != piece), "{piece} came back");
    }

    #[test]
    fn midday_has_few_finds() {
        let sky = Sky::bundled();
        let at = midnight_utc(2026, 6, 21) + 12 * HOUR;
        let finds = tonight(&sky, LONDON, at, 0, &|_| false);
        assert!(finds.len() <= 2, "{finds:#?}");
    }

    #[test]
    fn digits_group() {
        assert_eq!(thousands(384_400), "384,400");
        assert_eq!(thousands(999), "999");
    }
}
