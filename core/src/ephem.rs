//! Where the Sun, the Moon and the planets are, from mean orbital elements and
//! their largest perturbations, after Paul Schlyter's "How to compute
//! planetary positions". Good to an arcminute or two, which is finer than an
//! eye can point. Every angle here is in degrees; positions are geocentric and
//! referred to the equinox of the date.

use crate::time::{UnixMs, day_number};

const RAD: f64 = std::f64::consts::PI / 180.0;

pub fn rev(x: f64) -> f64 {
    x.rem_euclid(360.0)
}
pub(crate) fn sind(x: f64) -> f64 {
    (x * RAD).sin()
}
pub(crate) fn cosd(x: f64) -> f64 {
    (x * RAD).cos()
}
pub(crate) fn atan2d(y: f64, x: f64) -> f64 {
    y.atan2(x) / RAD
}
pub(crate) fn asind(x: f64) -> f64 {
    x.clamp(-1.0, 1.0).asin() / RAD
}
pub(crate) fn acosd(x: f64) -> f64 {
    x.clamp(-1.0, 1.0).acos() / RAD
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Body {
    Sun,
    Moon,
    Mercury,
    Venus,
    Mars,
    Jupiter,
    Saturn,
    Uranus,
    Neptune,
}

impl Body {
    pub const PLANETS: [Body; 7] = [
        Body::Mercury,
        Body::Venus,
        Body::Mars,
        Body::Jupiter,
        Body::Saturn,
        Body::Uranus,
        Body::Neptune,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Body::Sun => "the Sun",
            Body::Moon => "the Moon",
            Body::Mercury => "Mercury",
            Body::Venus => "Venus",
            Body::Mars => "Mars",
            Body::Jupiter => "Jupiter",
            Body::Saturn => "Saturn",
            Body::Uranus => "Uranus",
            Body::Neptune => "Neptune",
        }
    }

    pub fn id(self) -> &'static str {
        match self {
            Body::Sun => "sun",
            Body::Moon => "moon",
            Body::Mercury => "mercury",
            Body::Venus => "venus",
            Body::Mars => "mars",
            Body::Jupiter => "jupiter",
            Body::Saturn => "saturn",
            Body::Uranus => "uranus",
            Body::Neptune => "neptune",
        }
    }

    /// Apparent diameter in arcseconds at one astronomical unit.
    fn diameter_at_1au(self) -> f64 {
        match self {
            Body::Sun => 1919.26,
            Body::Moon => 0.0,
            Body::Mercury => 6.74,
            Body::Venus => 16.92,
            Body::Mars => 9.36,
            Body::Jupiter => 196.94,
            Body::Saturn => 165.6,
            Body::Uranus => 65.8,
            Body::Neptune => 62.2,
        }
    }
}

/// Where a body stands, seen from the centre of the Earth.
#[derive(Clone, Copy, Debug)]
pub struct Position {
    pub ra: f64,
    pub dec: f64,
    /// Astronomical units, or Earth radii for the Moon.
    pub distance: f64,
    pub ecl_lon: f64,
    pub ecl_lat: f64,
    pub magnitude: f64,
    /// The lit fraction of the disc, 0 to 1.
    pub phase: f64,
    /// Angle from the Sun.
    pub elongation: f64,
    /// Apparent diameter, arcseconds.
    pub diameter: f64,
    /// Saturn's ring opening angle; zero for everything else.
    pub ring_tilt: f64,
}

struct Elements {
    n: f64,
    i: f64,
    w: f64,
    a: f64,
    e: f64,
    m: f64,
}

fn elements(body: Body, d: f64) -> Elements {
    let el = |n, i, w, a, e, m| Elements { n, i, w, a, e, m };
    match body {
        Body::Sun => el(
            0.0,
            0.0,
            282.9404 + 4.70935e-5 * d,
            1.0,
            0.016709 - 1.151e-9 * d,
            356.0470 + 0.9856002585 * d,
        ),
        Body::Moon => el(
            125.1228 - 0.0529538083 * d,
            5.1454,
            318.0634 + 0.1643573223 * d,
            60.2666,
            0.054900,
            115.3654 + 13.0649929509 * d,
        ),
        Body::Mercury => el(
            48.3313 + 3.24587e-5 * d,
            7.0047 + 5.00e-8 * d,
            29.1241 + 1.01444e-5 * d,
            0.387098,
            0.205635 + 5.59e-10 * d,
            168.6562 + 4.0923344368 * d,
        ),
        Body::Venus => el(
            76.6799 + 2.46590e-5 * d,
            3.3946 + 2.75e-8 * d,
            54.8910 + 1.38374e-5 * d,
            0.723330,
            0.006773 - 1.302e-9 * d,
            48.0052 + 1.6021302244 * d,
        ),
        Body::Mars => el(
            49.5574 + 2.11081e-5 * d,
            1.8497 - 1.78e-8 * d,
            286.5016 + 2.92961e-5 * d,
            1.523688,
            0.093405 + 2.516e-9 * d,
            18.6021 + 0.5240207766 * d,
        ),
        Body::Jupiter => el(
            100.4542 + 2.76854e-5 * d,
            1.3030 - 1.557e-7 * d,
            273.8777 + 1.64505e-5 * d,
            5.20256,
            0.048498 + 4.469e-9 * d,
            19.8950 + 0.0830853001 * d,
        ),
        Body::Saturn => el(
            113.6634 + 2.38980e-5 * d,
            2.4886 - 1.081e-7 * d,
            339.3939 + 2.97661e-5 * d,
            9.55475,
            0.055546 - 9.499e-9 * d,
            316.9670 + 0.0334442282 * d,
        ),
        Body::Uranus => el(
            74.0005 + 1.3978e-5 * d,
            0.7733 + 1.9e-8 * d,
            96.6612 + 3.0565e-5 * d,
            19.18171 - 1.55e-8 * d,
            0.047318 + 7.45e-9 * d,
            142.5905 + 0.011725806 * d,
        ),
        Body::Neptune => el(
            131.7806 + 3.0173e-5 * d,
            1.7700 - 2.55e-7 * d,
            272.8461 - 6.027e-6 * d,
            30.05826 + 3.313e-8 * d,
            0.008606 + 2.15e-9 * d,
            260.2471 + 0.005995147 * d,
        ),
    }
}

/// The tilt of the Earth's axis to its orbit.
pub fn obliquity(d: f64) -> f64 {
    23.4393 - 3.563e-7 * d
}

/// Kepler's equation, solved for the eccentric anomaly.
fn eccentric_anomaly(m: f64, e: f64) -> f64 {
    let m = rev(m);
    let mut big_e = m + e / RAD * sind(m) * (1.0 + e * cosd(m));
    for _ in 0..20 {
        let next = big_e - (big_e - e / RAD * sind(big_e) - m) / (1.0 - e * cosd(big_e));
        let done = (next - big_e).abs() < 1e-6;
        big_e = next;
        if done {
            break;
        }
    }
    big_e
}

/// Distance and true anomaly in the orbit's own plane.
fn in_orbit(el: &Elements) -> (f64, f64) {
    let big_e = eccentric_anomaly(el.m, el.e);
    let xv = el.a * (cosd(big_e) - el.e);
    let yv = el.a * ((1.0 - el.e * el.e).sqrt() * sind(big_e));
    ((xv * xv + yv * yv).sqrt(), atan2d(yv, xv))
}

/// Ecliptic longitude, latitude and distance from the orbit's centre.
fn ecliptic(el: &Elements) -> (f64, f64, f64) {
    let (r, v) = in_orbit(el);
    let (sn, cn) = (sind(el.n), cosd(el.n));
    let (svw, cvw) = (sind(v + el.w), cosd(v + el.w));
    let ci = cosd(el.i);
    let xh = r * (cn * cvw - sn * svw * ci);
    let yh = r * (sn * cvw + cn * svw * ci);
    let zh = r * (svw * sind(el.i));
    (
        rev(atan2d(yh, xh)),
        atan2d(zh, (xh * xh + yh * yh).sqrt()),
        r,
    )
}

fn rect(lon: f64, lat: f64, r: f64) -> [f64; 3] {
    [
        r * cosd(lon) * cosd(lat),
        r * sind(lon) * cosd(lat),
        r * sind(lat),
    ]
}

fn to_equatorial(g: [f64; 3], d: f64) -> (f64, f64) {
    let ecl = obliquity(d);
    let xe = g[0];
    let ye = g[1] * cosd(ecl) - g[2] * sind(ecl);
    let ze = g[1] * sind(ecl) + g[2] * cosd(ecl);
    (rev(atan2d(ye, xe)), atan2d(ze, (xe * xe + ye * ye).sqrt()))
}

/// The Sun's geocentric ecliptic longitude and distance.
fn sun(d: f64) -> (f64, f64) {
    let el = elements(Body::Sun, d);
    let (r, v) = in_orbit(&el);
    (rev(v + el.w), r)
}

/// The Sun's apparent ecliptic longitude, which names the time of year
/// meteor showers are keyed to.
pub fn solar_longitude(at: UnixMs) -> f64 {
    sun(day_number(at)).0
}

/// Local sidereal time, in degrees, at east longitude `lon`.
pub fn sidereal_time(at: UnixMs, lon: f64) -> f64 {
    let d = day_number(at);
    let el = elements(Body::Sun, d);
    let ut_hours = (d - d.floor()) * 24.0;
    rev(el.m + el.w + 180.0 + ut_hours * 15.0 + lon)
}

pub fn position(body: Body, at: UnixMs) -> Position {
    let d = day_number(at);
    let (slon, sdist) = sun(d);
    match body {
        Body::Sun => {
            let (ra, dec) = to_equatorial(rect(slon, 0.0, sdist), d);
            Position {
                ra,
                dec,
                distance: sdist,
                ecl_lon: slon,
                ecl_lat: 0.0,
                magnitude: -26.7,
                phase: 1.0,
                elongation: 0.0,
                diameter: Body::Sun.diameter_at_1au() / sdist,
                ring_tilt: 0.0,
            }
        }
        Body::Moon => moon(d, slon, sdist),
        planet => planet_position(planet, d, slon, sdist),
    }
}

fn moon(d: f64, slon: f64, sdist: f64) -> Position {
    let el = elements(Body::Moon, d);
    let (mut lon, mut lat, mut r) = ecliptic(&el);
    let sun_el = elements(Body::Sun, d);
    let ms = rev(sun_el.m);
    let mm = rev(el.m);
    let ls = rev(sun_el.m + sun_el.w);
    let lm = rev(el.m + el.w + el.n);
    let dd = rev(lm - ls);
    let f = rev(lm - el.n);
    lon += -1.274 * sind(mm - 2.0 * dd) + 0.658 * sind(2.0 * dd)
        - 0.186 * sind(ms)
        - 0.059 * sind(2.0 * mm - 2.0 * dd)
        - 0.057 * sind(mm - 2.0 * dd + ms)
        + 0.053 * sind(mm + 2.0 * dd)
        + 0.046 * sind(2.0 * dd - ms)
        + 0.041 * sind(mm - ms)
        - 0.035 * sind(dd)
        - 0.031 * sind(mm + ms)
        - 0.015 * sind(2.0 * f - 2.0 * dd)
        + 0.011 * sind(mm - 4.0 * dd);
    lat += -0.173 * sind(f - 2.0 * dd)
        - 0.055 * sind(mm - f - 2.0 * dd)
        - 0.046 * sind(mm + f - 2.0 * dd)
        + 0.033 * sind(f + 2.0 * dd)
        + 0.017 * sind(2.0 * mm + f);
    r += -0.58 * cosd(mm - 2.0 * dd) - 0.46 * cosd(2.0 * dd);
    let lon = rev(lon);
    let (ra, dec) = to_equatorial(rect(lon, lat, r), d);
    let elongation = acosd(cosd(slon - lon) * cosd(lat));
    let fv = 180.0 - elongation;
    Position {
        ra,
        dec,
        distance: r,
        ecl_lon: lon,
        ecl_lat: lat,
        magnitude: -21.62 + 5.0 * (sdist * r).log10() + 0.026 * fv + 4.0e-9 * fv.powi(4),
        phase: (1.0 + cosd(fv)) / 2.0,
        elongation,
        diameter: 1873.7 * 60.0 / r,
        ring_tilt: 0.0,
    }
}

fn planet_position(body: Body, d: f64, slon: f64, sdist: f64) -> Position {
    let el = elements(body, d);
    let (mut lon, mut lat, r) = ecliptic(&el);
    let mj = rev(elements(Body::Jupiter, d).m);
    let msat = rev(elements(Body::Saturn, d).m);
    let mu = rev(elements(Body::Uranus, d).m);
    match body {
        Body::Jupiter => {
            lon += -0.332 * sind(2.0 * mj - 5.0 * msat - 67.6)
                - 0.056 * sind(2.0 * mj - 2.0 * msat + 21.0)
                + 0.042 * sind(3.0 * mj - 5.0 * msat + 21.0)
                - 0.036 * sind(mj - 2.0 * msat)
                + 0.022 * cosd(mj - msat)
                + 0.023 * sind(2.0 * mj - 3.0 * msat + 52.0)
                - 0.016 * sind(mj - 5.0 * msat - 69.0);
        }
        Body::Saturn => {
            lon += 0.812 * sind(2.0 * mj - 5.0 * msat - 67.6)
                - 0.229 * cosd(2.0 * mj - 4.0 * msat - 2.0)
                + 0.119 * sind(mj - 2.0 * msat - 3.0)
                + 0.046 * sind(2.0 * mj - 6.0 * msat - 69.0)
                + 0.014 * sind(mj - 3.0 * msat + 32.0);
            lat += -0.020 * cosd(2.0 * mj - 4.0 * msat - 2.0)
                + 0.018 * sind(2.0 * mj - 6.0 * msat - 49.0);
        }
        Body::Uranus => {
            lon += 0.040 * sind(msat - 2.0 * mu + 6.0) + 0.035 * sind(msat - 3.0 * mu + 33.0)
                - 0.015 * sind(mj - mu + 20.0);
        }
        _ => {}
    }
    let h = rect(lon, lat, r);
    let s = rect(slon, 0.0, sdist);
    let g = [h[0] + s[0], h[1] + s[1], h[2]];
    let big_r = (g[0] * g[0] + g[1] * g[1] + g[2] * g[2]).sqrt();
    let (ra, dec) = to_equatorial(g, d);
    let glon = rev(atan2d(g[1], g[0]));
    let glat = atan2d(g[2], (g[0] * g[0] + g[1] * g[1]).sqrt());
    let fv = acosd((r * r + big_r * big_r - sdist * sdist) / (2.0 * r * big_r));
    let elongation = acosd((sdist * sdist + big_r * big_r - r * r) / (2.0 * sdist * big_r));
    let base = 5.0 * (r * big_r).log10();
    let mut ring_tilt = 0.0;
    let magnitude = match body {
        Body::Mercury => -0.36 + base + 0.027 * fv + 2.2e-13 * fv.powi(6),
        Body::Venus => -4.34 + base + 0.013 * fv + 4.2e-7 * fv.powi(3),
        Body::Mars => -1.51 + base + 0.016 * fv,
        Body::Jupiter => -9.25 + base + 0.014 * fv,
        Body::Saturn => {
            let nr = 169.51 + 3.82e-5 * d;
            let b = asind(sind(glat) * cosd(28.06) - cosd(glat) * sind(28.06) * sind(glon - nr));
            ring_tilt = b;
            let ring = -2.6 * sind(b.abs()) + 1.2 * sind(b).powi(2);
            -9.0 + base + 0.044 * fv + ring
        }
        Body::Uranus => -7.15 + base + 0.001 * fv,
        _ => -6.90 + base + 0.001 * fv,
    };
    Position {
        ra,
        dec,
        distance: big_r,
        ecl_lon: glon,
        ecl_lat: glat,
        magnitude,
        phase: (1.0 + cosd(fv)) / 2.0,
        elongation,
        diameter: body.diameter_at_1au() / big_r,
        ring_tilt,
    }
}

/// The Moon's age as a fraction of its cycle: 0 new, 0.5 full.
pub fn moon_age(at: UnixMs) -> f64 {
    let m = position(Body::Moon, at);
    let s = position(Body::Sun, at);
    rev(m.ecl_lon - s.ecl_lon) / 360.0
}

/// Words for the Moon's phase.
pub fn moon_phase_name(age: f64) -> &'static str {
    match (age * 8.0 + 0.5).floor() as i32 % 8 {
        0 => "New Moon",
        1 => "Waxing crescent",
        2 => "First quarter",
        3 => "Waxing gibbous",
        4 => "Full Moon",
        5 => "Waning gibbous",
        6 => "Last quarter",
        _ => "Waning crescent",
    }
}
