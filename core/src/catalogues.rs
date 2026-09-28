//! The hand-kept lists: showpieces worth finding, named stars with a line
//! about each, and the year's meteor showers.

use serde::Deserialize;

#[derive(Clone, Debug, Deserialize)]
pub struct Showpiece {
    pub id: String,
    pub name: String,
    pub kind: Kind,
    pub ra: f64,
    pub dec: f64,
    /// Integrated magnitude; a dark nebula has none.
    #[serde(default = "no_light")]
    pub mag: f64,
    /// Apparent size in arcminutes; zero for a point.
    pub size: f64,
    pub fact: String,
}

fn no_light() -> f64 {
    99.0
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Cluster,
    Galaxy,
    Nebula,
    Double,
    Star,
    Dark,
}

#[derive(Clone, Debug, Deserialize)]
pub struct NamedStar {
    pub hr: u16,
    pub name: String,
    pub bayer: String,
    #[serde(default)]
    pub fact: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Shower {
    pub id: String,
    pub name: String,
    /// The Sun's ecliptic longitude at the peak.
    pub peak_sol: f64,
    /// Month and day, "MM-DD".
    pub start: String,
    pub end: String,
    pub ra: f64,
    pub dec: f64,
    pub zhr: u32,
    /// Entry speed, km/s.
    pub speed: u32,
}

impl Shower {
    /// Whether `(month, day)` falls in the activity window, which may run
    /// over the new year.
    pub fn active_on(&self, month: u32, day: u32) -> bool {
        let parse = |s: &str| -> (u32, u32) {
            let (m, d) = s.split_once('-').unwrap_or(("1", "1"));
            (m.parse().unwrap_or(1), d.parse().unwrap_or(1))
        };
        let (start, end, today) = (parse(&self.start), parse(&self.end), (month, day));
        if start <= end {
            start <= today && today <= end
        } else {
            today >= start || today <= end
        }
    }
}

#[derive(Deserialize)]
struct ShowpieceFile {
    showpiece: Vec<Showpiece>,
}
#[derive(Deserialize)]
struct StarFile {
    star: Vec<NamedStar>,
}
#[derive(Deserialize)]
struct ShowerFile {
    shower: Vec<Shower>,
}

pub struct Catalogues {
    pub showpieces: Vec<Showpiece>,
    pub stars: Vec<NamedStar>,
    pub showers: Vec<Shower>,
}

impl Catalogues {
    pub fn bundled() -> Catalogues {
        Catalogues {
            showpieces: toml::from_str::<ShowpieceFile>(include_str!("../data/showpieces.toml"))
                .expect("showpieces.toml")
                .showpiece,
            stars: toml::from_str::<StarFile>(include_str!("../data/star-names.toml"))
                .expect("star-names.toml")
                .star,
            showers: toml::from_str::<ShowerFile>(include_str!("../data/showers.toml"))
                .expect("showers.toml")
                .shower,
        }
    }

    pub fn star_name(&self, hr: u16) -> Option<&NamedStar> {
        self.stars.iter().find(|s| s.hr == hr)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stars::Catalogue;

    #[test]
    fn the_lists_load_and_agree_with_the_catalogue() {
        let c = Catalogues::bundled();
        assert!(c.showpieces.len() >= 30);
        assert!(c.stars.len() >= 60);
        assert!(c.showers.len() >= 10);
        let cat = Catalogue::bundled();
        for s in &c.stars {
            assert!(
                cat.get(s.hr).is_some(),
                "{} (HR {}) not in the catalogue",
                s.name,
                s.hr
            );
        }
        let polaris = c
            .stars
            .iter()
            .find(|s| s.name == "Polaris")
            .expect("Polaris");
        assert_eq!(polaris.hr, 424);
    }

    #[test]
    fn showers_can_span_the_new_year() {
        let c = Catalogues::bundled();
        let q = c
            .showers
            .iter()
            .find(|s| s.id == "quadrantids")
            .expect("Quadrantids");
        assert!(q.active_on(1, 3));
        assert!(q.active_on(12, 30));
        assert!(!q.active_on(6, 1));
    }
}
