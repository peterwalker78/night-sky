//! The day: while the Sun is up there are no stars to find, so the view
//! looks down at the earth instead. What it offers is true where the user
//! is today: where the Sun is and how long their shadow falls, how much the
//! day has grown or shrunk since yesterday, the Moon if it's out by day, and
//! a few small things on the ground that are really there this time of
//! year.

use serde::Deserialize;

use crate::coords::Observer;
use crate::ephem::{Body, moon_age, moon_phase_name};
use crate::finds::stable_hash;
use crate::sky::{crossing, horizon_altitude, see};
use crate::time::{DAY, MINUTE, UnixMs};

/// The outline a thing is drawn with.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Shape {
    Leaf,
    Acorn,
    Seed,
    Mushroom,
    Feather,
    Flower,
    Web,
    Bird,
    Snowflake,
    Bud,
    Stone,
    Drop,
    Tree,
    Worm,
    Bee,
    Ant,
    Moss,
    Cloud,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Thing {
    pub id: String,
    pub name: String,
    #[serde(rename = "where")]
    pub place: String,
    pub shape: Shape,
    pub months: Vec<u32>,
    pub zone: String,
    #[serde(default)]
    pub north: bool,
    pub fact: String,
    #[serde(default)]
    pub more: Vec<String>,
    pub thread: String,
    pub mirror: String,
}

/// A card for one of the day's own finds.
#[derive(Clone, Debug, Deserialize)]
pub struct Card {
    pub name: String,
    pub fact: String,
    #[serde(default)]
    pub more: Vec<String>,
    #[serde(default)]
    pub mirror: String,
    /// The turning year's two endings, for days growing or shrinking.
    #[serde(default)]
    pub longer: String,
    #[serde(default)]
    pub shorter: String,
}

#[derive(Clone, Debug, Deserialize)]
pub struct DayCards {
    pub sun: Card,
    pub shadow: Card,
    pub moon: Card,
    pub year: Card,
}

#[derive(Deserialize)]
struct File {
    thing: Vec<Thing>,
    day: DayCards,
}

pub struct Ground {
    pub things: Vec<Thing>,
    pub day: DayCards,
}

/// How many things on the ground a day offers.
pub const ON_THE_GROUND: usize = 3;

impl Ground {
    pub fn bundled() -> Ground {
        let file =
            toml::from_str::<File>(include_str!("../data/ground.toml")).expect("ground.toml");
        Ground {
            things: file.thing,
            day: file.day,
        }
    }

    /// Today's things: two of the season's where the year has seasons, and
    /// one that's true anywhere, chosen afresh each day.
    pub fn today(&self, month: u32, lat: f64, day: &str) -> Vec<&Thing> {
        let temperate = (23.5..66.0).contains(&lat.abs());
        // Months are written for the north; the south is six months on.
        let m = if lat < 0.0 {
            (month + 5) % 12 + 1
        } else {
            month
        };
        let seasonal: Vec<&Thing> = self
            .things
            .iter()
            .filter(|t| {
                temperate
                    && t.zone == "temperate"
                    && t.months.contains(&m)
                    && (lat >= 0.0 || !t.north)
            })
            .collect();
        let seasonal = pick(seasonal, ON_THE_GROUND - 1, 1, day);
        let anywhere: Vec<&Thing> = self.things.iter().filter(|t| t.zone == "any").collect();
        let mut out = seasonal;
        let rest = ON_THE_GROUND - out.len();
        out.extend(pick(anywhere, rest, 2, day));
        out
    }
}

/// `n` of `pool`, shuffled afresh each day.
fn pick<'a>(mut pool: Vec<&'a Thing>, n: usize, salt: u32, day: &str) -> Vec<&'a Thing> {
    pool.sort_by_key(|t| stable_hash((0, salt, 0), &format!("{day}:{}", t.id)));
    pool.truncate(n);
    pool
}

/// The Moon when it can be seen by day: up, and far enough from the Sun.
#[derive(Clone, Debug)]
pub struct DayMoon {
    pub alt: f64,
    pub az: f64,
    /// Fraction of the cycle, 0 new, 0.5 full.
    pub age: f64,
    pub phase: &'static str,
}

/// What's true about today, where the user is.
#[derive(Clone, Debug)]
pub struct Today {
    pub sun_alt: f64,
    pub sun_az: f64,
    /// Shadow length as a multiple of height, while the Sun is high enough
    /// to cast a proper one.
    pub shadow: Option<f64>,
    /// Which way shadows point, as an azimuth.
    pub shadow_az: f64,
    /// Today's length from sunrise to sunset, and how it compares with
    /// yesterday's, in milliseconds.
    pub length: Option<UnixMs>,
    pub change: Option<UnixMs>,
    pub moon: Option<DayMoon>,
}

fn day_length(observer: Observer, midnight: UnixMs) -> Option<UnixMs> {
    let h0 = horizon_altitude(Body::Sun);
    let alt = |t| see(Body::Sun, observer, t).alt;
    let rise = crossing(midnight, DAY, h0, true, alt)?;
    let set = crossing(rise, DAY, h0, false, alt)?;
    Some(set - rise)
}

pub fn today(observer: Observer, now: UnixMs, offset_s: i32) -> Today {
    let sun = see(Body::Sun, observer, now);
    let off = offset_s as i64 * 1000;
    let midnight = (now + off).div_euclid(DAY) * DAY - off;
    let length = day_length(observer, midnight);
    let yesterday = day_length(observer, midnight - DAY);
    let moon = {
        let m = see(Body::Moon, observer, now);
        let age = moon_age(now);
        // Near new it's lost in the glare; it needs to be well up too.
        let apart = (age - 0.5).abs() < 0.42;
        (m.alt > 8.0 && apart).then(|| DayMoon {
            alt: m.alt,
            az: m.az,
            age,
            phase: moon_phase_name(age),
        })
    };
    Today {
        sun_alt: sun.alt,
        sun_az: sun.az,
        shadow: (sun.alt > 3.0).then(|| 1.0 / sun.alt.to_radians().tan()),
        shadow_az: (sun.az + 180.0) % 360.0,
        length,
        change: length.zip(yesterday).map(|(a, b)| a - b),
        moon,
    }
}

/// A shadow's length, next to the height of whoever casts it.
pub fn shadow_words(ratio: f64) -> &'static str {
    match ratio {
        r if r < 0.35 => "barely there, pooled round your feet",
        r if r < 0.7 => "about half as long as you are tall",
        r if r < 0.9 => "a little shorter than you are tall",
        r if r < 1.12 => "about as long as you are tall",
        r if r < 1.4 => "a little longer than you are tall",
        r if r < 1.75 => "about one and a half times your height",
        r if r < 2.5 => "about twice your height",
        r if r < 3.5 => "about three times your height",
        _ => "stretched out many times your height",
    }
}

/// Today against yesterday, in words.
pub fn change_words(change: UnixMs) -> String {
    let minutes = (change.abs() as f64 / MINUTE as f64).round() as i64;
    let count = match minutes {
        0 => return "Today is almost exactly as long as yesterday: the days have all but stopped changing.".to_owned(),
        1 => "a minute".to_owned(),
        2 => "two minutes".to_owned(),
        3 => "three minutes".to_owned(),
        4 => "four minutes".to_owned(),
        5 => "five minutes".to_owned(),
        n => format!("{n} minutes"),
    };
    let way = if change > 0 { "longer" } else { "shorter" };
    format!("Today is about {count} {way} than yesterday.")
}

/// Hours and minutes of daylight, in words.
pub fn length_words(length: UnixMs) -> String {
    let minutes = length / MINUTE;
    let (h, m) = (minutes / 60, minutes % 60);
    format!("{h} hours and {m} minutes of daylight")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::questions::UNSAID;
    use crate::time::{HOUR, midnight_utc};

    const LONDON: Observer = Observer {
        lat: 51.5,
        lon: -0.13,
    };
    const SYDNEY: Observer = Observer {
        lat: -33.9,
        lon: 151.2,
    };

    #[test]
    fn every_thing_is_well_made() {
        let ground = Ground::bundled();
        assert!(ground.things.len() >= 40);
        for t in &ground.things {
            let words = format!("{} {} {}", t.fact, t.mirror, t.more.join(" ")).to_lowercase();
            for w in UNSAID {
                assert!(!words.contains(w), "{}: {w}", t.id);
            }
            for opening in ["It ", "This ", "He ", "She ", "They "] {
                assert!(!t.mirror.starts_with(opening), "{}", t.id);
            }
            assert!(["people", "plans", "perspective"].contains(&t.thread.as_str()));
        }
        let d = &ground.day;
        for c in [&d.sun, &d.shadow, &d.moon, &d.year] {
            let words = format!(
                "{} {} {} {} {}",
                c.fact,
                c.mirror,
                c.more.join(" "),
                c.longer,
                c.shorter
            )
            .to_lowercase();
            for w in UNSAID {
                assert!(!words.contains(w), "{}: {w}", c.name);
            }
        }
        for t in &ground.things {
            match t.zone.as_str() {
                "any" => assert!(t.months.is_empty(), "{}", t.id),
                "temperate" => assert!(!t.months.is_empty(), "{}", t.id),
                z => panic!("{}: zone {z}", t.id),
            }
        }
    }

    #[test]
    fn every_month_has_things_in_either_hemisphere() {
        let ground = Ground::bundled();
        for month in 1..=12 {
            for lat in [51.5, -33.9, 1.3] {
                let day = format!("2026-{month:02}-10");
                let today = ground.today(month, lat, &day);
                assert_eq!(today.len(), ON_THE_GROUND, "{month} {lat}");
                if lat.abs() > 23.5 {
                    assert!(today.iter().any(|t| t.zone == "temperate"), "{month} {lat}");
                }
                if lat < 0.0 {
                    assert!(today.iter().all(|t| !t.north), "{month} {lat}");
                }
            }
        }
        // Snowdrops in a London February, never in a Sydney one.
        let feb = |lat: f64| {
            (1..=28)
                .flat_map(|d| {
                    ground
                        .today(2, lat, &format!("2026-02-{d:02}"))
                        .into_iter()
                        .map(|t| t.id.clone())
                        .collect::<Vec<_>>()
                })
                .collect::<Vec<_>>()
        };
        assert!(feb(51.5).iter().any(|id| id == "snowdrops"));
        assert!(!feb(-33.9).iter().any(|id| id == "snowdrops"));
    }

    #[test]
    fn shadows_and_days_match_the_sun() {
        // London, noon at the September equinox: the Sun about 38.5° up in
        // the south, so a shadow about 1.26 times as long, pointing north.
        let noon = midnight_utc(2026, 9, 23) + 12 * HOUR;
        let t = today(LONDON, noon, 3600);
        assert!((t.sun_alt - 38.3).abs() < 1.0, "{}", t.sun_alt);
        let ratio = t.shadow.expect("a shadow");
        assert!((ratio - 1.26).abs() < 0.08, "{ratio}");
        assert!(t.shadow_az < 20.0 || t.shadow_az > 340.0, "{}", t.shadow_az);
        assert_eq!(shadow_words(ratio), "a little longer than you are tall");
        // Around the equinox London's days shorten by nearly four minutes a day.
        let change = t.change.expect("a change");
        assert!((-5 * MINUTE..-3 * MINUTE).contains(&change), "{change}");
        assert!(change_words(change).contains("shorter"));
        let length = t.length.expect("a length");
        assert!((11 * HOUR + 50 * MINUTE..12 * HOUR + 30 * MINUTE).contains(&length));
        // Sydney at the same moment is heading into summer.
        let s = today(SYDNEY, noon - 10 * HOUR, 36_000);
        assert!(s.change.expect("a change") > 0);
    }
}
