//! The constellations' stick figures, as pairs of catalogue stars.

pub struct Figure {
    pub abbrev: String,
    pub name: String,
    /// 1 for the famous figures, 3 for the faint ones.
    pub rank: u8,
    pub edges: Vec<(u16, u16)>,
}

impl Figure {
    pub fn stars(&self) -> Vec<u16> {
        let mut hrs: Vec<u16> = self.edges.iter().flat_map(|&(a, b)| [a, b]).collect();
        hrs.sort_unstable();
        hrs.dedup();
        hrs
    }
}

/// A true line about a constellation worth finding by its shape.
#[derive(Clone, Debug, serde::Deserialize)]
pub struct Note {
    pub abbrev: String,
    pub fact: String,
    #[serde(default)]
    pub more: Vec<String>,
}

#[derive(serde::Deserialize)]
struct Notes {
    note: Vec<Note>,
}

pub fn notes() -> Vec<Note> {
    toml::from_str::<Notes>(include_str!("../data/constellation-notes.toml"))
        .expect("constellation-notes.toml")
        .note
}

impl Figure {
    /// The middle of the figure and how far its stars reach from it (degrees),
    /// for the equinox the catalogue directions are given in.
    pub fn centre(&self, cat: &crate::stars::Catalogue) -> Option<(crate::coords::Vec3, f64)> {
        let dirs: Vec<crate::coords::Vec3> = self
            .stars()
            .iter()
            .filter_map(|hr| cat.get(*hr).map(|s| s.dir))
            .collect();
        if dirs.is_empty() {
            return None;
        }
        let mut c = [0.0; 3];
        for d in &dirs {
            for k in 0..3 {
                c[k] += d[k];
            }
        }
        let n = (c[0] * c[0] + c[1] * c[1] + c[2] * c[2]).sqrt().max(1e-12);
        let c = [c[0] / n, c[1] / n, c[2] / n];
        let reach = dirs
            .iter()
            .map(|d| {
                (d[0] * c[0] + d[1] * c[1] + d[2] * c[2])
                    .clamp(-1.0, 1.0)
                    .acos()
                    .to_degrees()
            })
            .fold(0.0, f64::max);
        Some((c, reach))
    }
}

pub fn bundled() -> Vec<Figure> {
    parse(include_str!("../data/constellations.txt"))
}

pub fn parse(text: &str) -> Vec<Figure> {
    text.lines()
        .filter(|l| !l.starts_with('#') && !l.trim().is_empty())
        .filter_map(|line| {
            let mut parts = line.split('|');
            let abbrev = parts.next()?.to_owned();
            let name = parts.next()?.to_owned();
            let rank = parts.next()?.parse().ok()?;
            let edges = parts
                .next()?
                .split_whitespace()
                .filter_map(|pair| {
                    let (a, b) = pair.split_once('-')?;
                    Some((a.parse().ok()?, b.parse().ok()?))
                })
                .collect();
            Some(Figure {
                abbrev,
                name,
                rank,
                edges,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    #[test]
    fn figures_use_the_stars_people_see() {
        let figures = super::bundled();
        let stars = crate::stars::Catalogue::bundled();
        let get = |abbrev: &str| figures.iter().find(|f| f.abbrev == abbrev).unwrap();
        for f in &figures {
            for hr in f.stars() {
                let star = stars
                    .get(hr)
                    .unwrap_or_else(|| panic!("{}: HR {hr}", f.abbrev));
                assert!(star.mag < 6.0, "{}: HR {hr} is too faint to see", f.abbrev);
            }
        }
        let has = |abbrev: &str, a: u16, b: u16| {
            get(abbrev)
                .edges
                .iter()
                .any(|&(x, y)| (x, y) == (a, b) || (y, x) == (a, b))
        };
        // The Plough, all seven stars.
        for (a, b) in [
            (5191, 5054),
            (5054, 4905),
            (4905, 4660),
            (4660, 4301),
            (4301, 4295),
            (4295, 4554),
            (4554, 4660),
        ] {
            assert!(has("UMa", a, b), "Plough {a}-{b}");
        }
        // The bright one of each close pair: Cor Caroli, gamma-2 Delphini.
        assert!(get("CVn").stars().contains(&4915));
        assert!(get("Del").stars().contains(&7948));
    }

    #[test]
    fn every_constellation_is_there() {
        let figures = super::bundled();
        assert!(figures.len() >= 88);
        let orion = figures.iter().find(|f| f.name == "Orion").expect("Orion");
        assert!(orion.stars().contains(&2061), "Betelgeuse");
        assert!(orion.stars().contains(&1713), "Rigel");
        let cas = figures
            .iter()
            .find(|f| f.abbrev == "Cas")
            .expect("Cassiopeia");
        assert_eq!(cas.stars().len(), 5);
    }
}
