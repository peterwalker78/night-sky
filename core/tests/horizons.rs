//! The ephemeris against JPL Horizons. Planets and the Sun are compared as
//! seen from the Earth's centre; the Moon as seen from three real places,
//! because its parallax moves it by up to a degree. The truncated lunar
//! series leaves up to about four arcminutes, a seventh of the Moon's width.

use night_sky_core::coords::{Observer, separation, topocentric};
use night_sky_core::ephem::{Body, position};

/// (unix ms, body, right ascension, declination), apparent, equinox of date,
/// from https://ssd.jpl.nasa.gov/api/horizons.api (tools/horizons-fixtures.py).
const FIXTURES: &[(i64, &str, f64, f64)] = &[
    (1790625600000, "sun", 185.24271, -2.26833), // 2026-09-28 20:00
    (1790625600000, "mercury", 205.23461, -12.06333), // 2026-09-28 20:00
    (1790625600000, "venus", 213.38385, -20.68862), // 2026-09-28 20:00
    (1790625600000, "mars", 122.84358, 21.01514), // 2026-09-28 20:00
    (1790625600000, "jupiter", 141.81391, 15.62631), // 2026-09-28 20:00
    (1790625600000, "saturn", 11.86163, 2.14953), // 2026-09-28 20:00
    (1790625600000, "uranus", 63.67848, 21.07924), // 2026-09-28 20:00
    (1790625600000, "neptune", 3.24538, -0.14136), // 2026-09-28 20:00
    (1790625600000, "moon@london", 29.88554, 16.20040), // 2026-09-28 20:00
    (1790625600000, "moon@sydney", 28.44957, 17.51275), // 2026-09-28 20:00
    (1790625600000, "moon@tromso", 29.57046, 16.07360), // 2026-09-28 20:00
    (1805079600000, "sun", 354.76976, -2.26294), // 2027-03-15 03:00
    (1805079600000, "mercury", 329.20949, -13.12707), // 2027-03-15 03:00
    (1805079600000, "venus", 319.00820, -15.97329), // 2027-03-15 03:00
    (1805079600000, "mars", 146.55084, 17.58133), // 2027-03-15 03:00
    (1805079600000, "jupiter", 141.07101, 16.36182), // 2027-03-15 03:00
    (1805079600000, "saturn", 14.43200, 3.73289), // 2027-03-15 03:00
    (1805079600000, "uranus", 60.14513, 20.48160), // 2027-03-15 03:00
    (1805079600000, "neptune", 3.94219, 0.23887), // 2027-03-15 03:00
    (1805079600000, "moon@london", 74.97751, 26.52433), // 2027-03-15 03:00
    (1805079600000, "moon@sydney", 76.24555, 28.05095), // 2027-03-15 03:00
    (1805079600000, "moon@tromso", 75.27344, 26.45947), // 2027-03-15 03:00
    (1909173600000, "sun", 101.00352, 23.04892), // 2030-07-01 22:00
    (1909173600000, "mercury", 110.99263, 23.86016), // 2030-07-01 22:00
    (1909173600000, "venus", 69.86607, 20.83740), // 2030-07-01 22:00
    (1909173600000, "mars", 90.20610, 24.06423), // 2030-07-01 22:00
    (1909173600000, "jupiter", 225.86452, -16.19748), // 2030-07-01 22:00
    (1909173600000, "saturn", 62.11553, 19.08113), // 2030-07-01 22:00
    (1909173600000, "uranus", 79.08128, 23.11636), // 2030-07-01 22:00
    (1909173600000, "neptune", 12.85008, 3.86062), // 2030-07-01 22:00
    (1909173600000, "moon@london", 112.34196, 17.91508), // 2030-07-01 22:00
    (1909173600000, "moon@sydney", 113.51436, 19.27946), // 2030-07-01 22:00
    (1909173600000, "moon@tromso", 112.61370, 17.81929), // 2030-07-01 22:00
    (2082088800000, "sun", 272.42741, -23.41323), // 2035-12-24 06:00
    (2082088800000, "mercury", 294.20215, -23.15012), // 2035-12-24 06:00
    (2082088800000, "venus", 308.96620, -20.57336), // 2035-12-24 06:00
    (2082088800000, "mars", 10.17526, 4.51173),  // 2035-12-24 06:00
    (2082088800000, "jupiter", 38.90475, 13.99533), // 2035-12-24 06:00
    (2082088800000, "saturn", 141.45909, 16.09419), // 2035-12-24 06:00
    (2082088800000, "uranus", 104.69704, 23.13808), // 2035-12-24 06:00
    (2082088800000, "neptune", 20.82343, 6.93355), // 2035-12-24 06:00
    (2082088800000, "moon@london", 212.38325, -8.86228), // 2035-12-24 06:00
    (2082088800000, "moon@sydney", 211.43304, -7.50620), // 2035-12-24 06:00
    (2082088800000, "moon@tromso", 212.15168, -8.97649), // 2035-12-24 06:00
    (2377598400000, "sun", 42.93929, 16.45144),  // 2045-05-05 12:00
    (2377598400000, "mercury", 23.95379, 7.31381), // 2045-05-05 12:00
    (2377598400000, "venus", 55.35763, 19.56843), // 2045-05-05 12:00
    (2377598400000, "mars", 42.25646, 16.13689), // 2045-05-05 12:00
    (2377598400000, "jupiter", 339.61277, -9.53955), // 2045-05-05 12:00
    (2377598400000, "saturn", 248.86936, -20.01556), // 2045-05-05 12:00
    (2377598400000, "uranus", 145.35612, 14.62620), // 2045-05-05 12:00
    (2377598400000, "neptune", 42.62686, 14.57494), // 2045-05-05 12:00
    (2377598400000, "moon@london", 275.95898, -27.77778), // 2045-05-05 12:00
    (2377598400000, "moon@sydney", 277.31633, -26.90449), // 2045-05-05 12:00
    (2377598400000, "moon@tromso", 276.26896, -27.93938), // 2045-05-05 12:00
];

fn site(name: &str) -> Observer {
    match name {
        "london" => Observer {
            lat: 51.5072,
            lon: -0.1276,
        },
        "sydney" => Observer {
            lat: -33.8688,
            lon: 151.2093,
        },
        "tromso" => Observer {
            lat: 69.6492,
            lon: 18.9553,
        },
        other => panic!("no site {other}"),
    }
}

fn body(name: &str) -> Body {
    match name {
        "sun" => Body::Sun,
        "mercury" => Body::Mercury,
        "venus" => Body::Venus,
        "mars" => Body::Mars,
        "jupiter" => Body::Jupiter,
        "saturn" => Body::Saturn,
        "uranus" => Body::Uranus,
        "neptune" => Body::Neptune,
        other => panic!("no body {other}"),
    }
}

#[test]
fn planets_and_the_sun_are_within_a_fifth_of_a_degree() {
    for &(at, name, ra, dec) in FIXTURES.iter().filter(|f| !f.1.starts_with("moon")) {
        let p = position(body(name), at);
        let off = separation(p.ra, p.dec, ra, dec);
        assert!(off < 0.2, "{name} at {at}: off by {off:.3} degrees");
    }
}

#[test]
fn the_moon_seen_from_the_ground_is_within_a_tenth_of_a_degree() {
    for &(at, name, ra, dec) in FIXTURES.iter().filter(|f| f.1.starts_with("moon")) {
        let place = site(name.split('@').nth(1).unwrap());
        let m = position(Body::Moon, at);
        let (tra, tdec) = topocentric(place, at, m.ra, m.dec, m.distance);
        let off = separation(tra, tdec, ra, dec);
        assert!(off < 0.1, "{name} at {at}: off by {off:.3} degrees");
    }
}
