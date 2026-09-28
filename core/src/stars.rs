//! The Yale Bright Star Catalogue: every star to about magnitude 6.5, which
//! is every star an eye can see from somewhere dark.

use crate::coords::{Mat3, Vec3, apply, unit};
use std::collections::HashMap;

const RECORD: usize = 14;

#[derive(Clone, Copy, Debug)]
pub struct Star {
    pub hr: u16,
    /// Direction for J2000.
    pub dir: Vec3,
    pub mag: f32,
    /// B-V colour index, if measured.
    pub bv: Option<f32>,
}

pub struct Catalogue {
    /// Brightest first.
    pub stars: Vec<Star>,
    by_hr: HashMap<u16, usize>,
}

impl Catalogue {
    pub fn bundled() -> Catalogue {
        Catalogue::parse(include_bytes!("../data/stars.bin"))
    }

    pub fn parse(bytes: &[u8]) -> Catalogue {
        let stars: Vec<Star> = bytes
            .as_chunks::<RECORD>()
            .0
            .iter()
            .map(|r| {
                let f32_at = |i: usize| f32::from_le_bytes([r[i], r[i + 1], r[i + 2], r[i + 3]]);
                let i16_at = |i: usize| i16::from_le_bytes([r[i], r[i + 1]]);
                let bv = i16_at(12);
                Star {
                    hr: u16::from_le_bytes([r[0], r[1]]),
                    dir: unit(f32_at(2) as f64, f32_at(6) as f64),
                    mag: i16_at(10) as f32 / 100.0,
                    bv: (bv != 9999).then_some(bv as f32 / 100.0),
                }
            })
            .collect();
        let by_hr = stars.iter().enumerate().map(|(i, s)| (s.hr, i)).collect();
        Catalogue { stars, by_hr }
    }

    pub fn get(&self, hr: u16) -> Option<&Star> {
        self.by_hr.get(&hr).map(|&i| &self.stars[i])
    }

    pub fn index_of(&self, hr: u16) -> Option<usize> {
        self.by_hr.get(&hr).copied()
    }

    /// Every star's direction for the equinox of a date.
    pub fn precessed(&self, precession: &Mat3) -> Vec<Vec3> {
        self.stars
            .iter()
            .map(|s| apply(precession, s.dir))
            .collect()
    }
}

/// A gentle colour for a B-V index: real stars look nearly white, with blue
/// and orange hints, so this stays close to white.
pub fn tint(bv: Option<f32>) -> [f32; 3] {
    let Some(bv) = bv else {
        return [1.0, 1.0, 1.0];
    };
    const STOPS: [(f32, [f32; 3]); 5] = [
        (-0.3, [0.72, 0.80, 1.0]),
        (0.0, [0.90, 0.93, 1.0]),
        (0.6, [1.0, 0.97, 0.90]),
        (1.1, [1.0, 0.88, 0.72]),
        (1.8, [1.0, 0.76, 0.56]),
    ];
    let bv = bv.clamp(STOPS[0].0, STOPS[4].0);
    for pair in STOPS.windows(2) {
        let (a, ca) = pair[0];
        let (b, cb) = pair[1];
        if bv <= b {
            let t = (bv - a) / (b - a);
            return [
                ca[0] + (cb[0] - ca[0]) * t,
                ca[1] + (cb[1] - ca[1]) * t,
                ca[2] + (cb[2] - ca[2]) * t,
            ];
        }
    }
    STOPS[4].1
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::coords::angles;

    #[test]
    fn the_catalogue_holds_the_naked_eye_sky() {
        let cat = Catalogue::bundled();
        assert!(cat.stars.len() > 9000);
        let sirius = cat.get(2491).expect("Sirius");
        assert!((sirius.mag + 1.46).abs() < 0.01);
        let (ra, dec) = angles(sirius.dir);
        assert!(
            (ra - 101.287).abs() < 0.01 && (dec + 16.716).abs() < 0.01,
            "{ra} {dec}"
        );
        assert_eq!(cat.stars[0].hr, 2491, "brightest first");
        let polaris = cat.get(424).expect("Polaris");
        assert!(angles(polaris.dir).1 > 89.2);
    }
}
