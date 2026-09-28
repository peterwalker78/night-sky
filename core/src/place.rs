//! Where the user is, without asking and without a location service: the
//! time zone they already chose names a place in the tz database's zone table.
//! A zone is a region, so this is a town-sized guess, a few hundred kilometres
//! at worst. That moves the sky by a few degrees, which a naked eye forgives.

use crate::coords::Observer;
use std::fs;

/// The coordinates of the local time zone's principal place. `zone` is the
/// zone's name if the caller already knows it.
pub fn here(zone: Option<&str>) -> Option<Observer> {
    let zone = zone.map(str::to_owned).or_else(zone_name)?;
    coords_of(&zone, &read_zone_table()?)
}

fn zone_name() -> Option<String> {
    if let Ok(tz) = std::env::var("TZ")
        && !tz.is_empty()
    {
        return Some(tz.trim_start_matches(':').to_owned());
    }
    let link = fs::read_link("/etc/localtime").ok()?;
    let path = link.to_str()?;
    let (_, zone) = path.split_once("zoneinfo/")?;
    Some(zone.to_owned())
}

fn read_zone_table() -> Option<String> {
    [
        "/usr/share/zoneinfo/zone1970.tab",
        "/usr/share/zoneinfo/zone.tab",
    ]
    .into_iter()
    .find_map(|path| fs::read_to_string(path).ok())
}

/// The coordinates listed for `zone` in a tz database zone table.
pub fn coords_of(zone: &str, table: &str) -> Option<Observer> {
    table
        .lines()
        .filter(|line| !line.starts_with('#'))
        .find_map(|line| {
            let mut fields = line.split('\t');
            let position = fields.nth(1)?;
            (fields.next()? == zone).then_some(position)
        })
        .and_then(parse_iso6709)
}

/// `+513030-0000731` (degrees, minutes and optional seconds) as degrees.
fn parse_iso6709(text: &str) -> Option<Observer> {
    let split = text.get(1..)?.find(['+', '-'])? + 1;
    let (lat, lon) = text.split_at(split);
    Some(Observer {
        lat: degrees(lat, 2)?,
        lon: degrees(lon, 3)?,
    })
}

/// A signed `DDMM[SS]` field, `width` digits of degrees.
fn degrees(text: &str, width: usize) -> Option<f64> {
    let sign = match text.as_bytes().first()? {
        b'+' => 1.0,
        b'-' => -1.0,
        _ => return None,
    };
    let digits = &text[1..];
    if !digits.bytes().all(|b| b.is_ascii_digit()) || digits.len() < width + 2 {
        return None;
    }
    let part = |from: usize, len: usize| digits.get(from..from + len)?.parse::<f64>().ok();
    let seconds = if digits.len() >= width + 4 {
        part(width + 2, 2)?
    } else {
        0.0
    };
    Some(sign * (part(0, width)? + part(width, 2)? / 60.0 + seconds / 3600.0))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zone_coordinates_are_read_from_the_table() {
        let table = "\
#code\tcoordinates\tTZ\tcomments
GB,GG,IM,JE\t+513030-0000731\tEurope/London
AU\t-3352+15113\tAustralia/Sydney\tNew South Wales
";
        let london = coords_of("Europe/London", table).expect("London");
        assert!((london.lat - 51.5083).abs() < 0.001, "{london:?}");
        assert!((london.lon + 0.1253).abs() < 0.001, "{london:?}");
        let sydney = coords_of("Australia/Sydney", table).expect("Sydney");
        assert!((sydney.lat + 33.8667).abs() < 0.001, "{sydney:?}");
        assert!((sydney.lon - 151.2167).abs() < 0.001, "{sydney:?}");
        assert_eq!(coords_of("Mars/Olympus", table), None);
    }
}
