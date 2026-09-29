//! Keeping someone company in the background: the music plays on, the sky
//! rests, and now and then the wisp looks in. It always comes to an end.

use serde::Deserialize;

use crate::finds::stable_hash;
use crate::time::{HOUR, MINUTE, UnixMs};

/// The longest it keeps company before winding the evening down.
pub const LONGEST: UnixMs = 2 * HOUR;

#[derive(Deserialize)]
pub struct Lines {
    pub start: String,
    pub back: String,
    #[serde(rename = "not-yet")]
    pub not_yet: String,
    care: Vec<String>,
    #[serde(rename = "care-day")]
    care_day: Vec<String>,
    #[serde(rename = "care-night")]
    care_night: Vec<String>,
    people: Vec<String>,
    pub risen: String,
    pub late: String,
    pub closing: String,
}

#[derive(Deserialize)]
struct File {
    company: Lines,
}

impl Lines {
    pub fn bundled() -> Lines {
        toml::from_str::<File>(include_str!("../data/wisp-lines.toml"))
            .expect("wisp-lines.toml")
            .company
    }
}

/// What the wisp says when it looks in.
#[derive(Clone, Debug, PartialEq)]
pub enum Look {
    /// A word about looking after yourself.
    Care(String),
    /// Something that has come up in the sky since.
    Risen(String),
    /// Past eleven: time to think about bed.
    Late,
    /// The end: it winds the evening down.
    Closing,
}

pub struct Company {
    pub since: UnixMs,
    next: UnixMs,
    looks: u32,
    said_late: bool,
    seed: String,
}

impl Company {
    pub fn new(now: UnixMs, night: &str) -> Company {
        let mut c = Company {
            since: now,
            next: now,
            looks: 0,
            said_late: false,
            seed: night.to_owned(),
        };
        // The first look in comes a little sooner than the rest.
        c.next = now + c.gap() / 2;
        c
    }

    /// Between twenty and thirty-five minutes.
    fn gap(&self) -> UnixMs {
        let roll = stable_hash((0, self.looks, 7), &self.seed) % 16;
        20 * MINUTE + roll as UnixMs * MINUTE
    }

    /// Whether `tick` has something to say now, without working it out.
    pub fn due(&self, now: UnixMs, (hour, minute): (u32, u32)) -> bool {
        let small_hours = hour < 5 && (hour > 0 || minute >= 30);
        now >= self.next
            || now - self.since >= LONGEST
            || (small_hours && now - self.since >= 10 * MINUTE)
    }

    /// Whether it's time to look in, and what to say. `hour` and `minute`
    /// are the local clock; `risen` is something that has come up since,
    /// if anything; `person` someone named on more than one night.
    pub fn tick(
        &mut self,
        now: UnixMs,
        (hour, minute): (u32, u32),
        risen: Option<&str>,
        person: Option<&str>,
        by_day: bool,
        lines: &Lines,
    ) -> Option<Look> {
        let small_hours = hour < 5 && (hour > 0 || minute >= 30);
        if now - self.since >= LONGEST || (small_hours && now - self.since >= 10 * MINUTE) {
            return Some(Look::Closing);
        }
        if now < self.next {
            return None;
        }
        self.looks += 1;
        self.next = now + self.gap();
        let late = !(5..23).contains(&hour);
        if late && !self.said_late {
            self.said_late = true;
            return Some(Look::Late);
        }
        if let Some(name) = risen {
            return Some(Look::Risen(name.to_owned()));
        }
        let roll = stable_hash((self.looks as i32, 0, 3), &self.seed) as usize;
        if let Some(name) = person
            && roll.is_multiple_of(4)
        {
            let line = &lines.people[roll / 4 % lines.people.len()];
            return Some(Look::Care(line.replace("{name}", name)));
        }
        if roll % 4 == 1 {
            let line = lines.people.iter().find(|l| !l.contains("{name}"));
            if let Some(line) = line {
                return Some(Look::Care(line.clone()));
            }
        }
        // Words for any time, and some for the time of day.
        let pool: Vec<&String> = lines
            .care
            .iter()
            .chain(if by_day {
                &lines.care_day
            } else {
                &lines.care_night
            })
            .collect();
        Some(Look::Care(
            pool[(roll / 4 + self.looks as usize) % pool.len()].clone(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::questions::UNSAID;

    #[test]
    fn it_looks_in_now_and_then_and_always_ends() {
        let lines = Lines::bundled();
        let mut c = Company::new(0, "2026-10-01");
        let mut looks = Vec::new();
        let mut t = 0;
        let end = loop {
            let hour = 20 + (t / HOUR) as u32;
            if let Some(look) = c.tick(t, (hour, 0), None, Some("Sam"), false, &lines) {
                if look == Look::Closing {
                    break t;
                }
                looks.push((t, look));
            }
            t += MINUTE;
        };
        assert_eq!(end, LONGEST);
        assert!((3..=6).contains(&looks.len()), "{looks:?}");
        for pair in looks.windows(2) {
            let gap = pair[1].0 - pair[0].0;
            assert!((20 * MINUTE..=35 * MINUTE).contains(&gap), "{gap}");
        }
    }

    #[test]
    fn late_it_says_so_once_and_ends_in_the_small_hours() {
        let lines = Lines::bundled();
        let mut c = Company::new(0, "2026-10-01");
        let mut lates = 0;
        let mut t = 0;
        let end = loop {
            // Starting at 23:40.
            let minutes = 23 * 60 + 40 + t / MINUTE;
            let clock = (((minutes / 60) % 24) as u32, (minutes % 60) as u32);
            match c.tick(t, clock, None, None, false, &lines) {
                Some(Look::Closing) => break t,
                Some(Look::Late) => lates += 1,
                _ => {}
            }
            t += MINUTE;
        };
        assert_eq!(lates, 1);
        assert!(end <= 55 * MINUTE, "{end}");
    }

    #[test]
    fn its_words_never_press_or_name_the_feeling() {
        let text = include_str!("../data/wisp-lines.toml").to_lowercase();
        let company = text.split("[company]").nth(1).expect("company");
        for word in UNSAID {
            assert!(!company.contains(word), "{word}");
        }
        let words: Vec<&str> = company
            .split(|c: char| !c.is_alphanumeric() && c != '\'')
            .collect();
        for word in ["should", "must", "forget", "need"] {
            assert!(!words.contains(&word), "{word}");
        }
    }
}
