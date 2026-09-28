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
    fn every_constellation_is_there() {
        let figures = super::bundled();
        assert!(figures.len() >= 88);
        let orion = figures.iter().find(|f| f.abbrev == "Ori").expect("Orion");
        assert!(orion.stars().contains(&2061), "Betelgeuse");
        assert!(orion.stars().contains(&1713), "Rigel");
        let cas = figures
            .iter()
            .find(|f| f.abbrev == "Cas")
            .expect("Cassiopeia");
        assert_eq!(cas.stars().len(), 5);
    }
}
