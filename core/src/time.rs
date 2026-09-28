//! Moments in time. The app passes the clock in; nothing here reads it.

/// Milliseconds since the Unix epoch, UTC.
pub type UnixMs = i64;

pub const MINUTE: UnixMs = 60_000;
pub const HOUR: UnixMs = 60 * MINUTE;
pub const DAY: UnixMs = 24 * HOUR;

/// Days since 2000 January 0.0 UT, the day number the ephemeris is written
/// in. Universal Time stands in for Terrestrial Time: the minute or so between
/// them moves the Moon by well under an arcminute.
pub fn day_number(at: UnixMs) -> f64 {
    at as f64 / DAY as f64 - 10_956.0
}

/// The civil date at `at`, shifted by `offset_s` seconds from UTC.
pub fn civil_date(at: UnixMs, offset_s: i32) -> (i32, u32, u32) {
    let days = (at + offset_s as i64 * 1000).div_euclid(DAY);
    civil_from_days(days)
}

/// Hours and minutes of the local clock at `at`.
pub fn clock(at: UnixMs, offset_s: i32) -> (u32, u32) {
    let ms = (at + offset_s as i64 * 1000).rem_euclid(DAY);
    ((ms / HOUR) as u32, ((ms % HOUR) / MINUTE) as u32)
}

/// The Unix time of 00:00 UTC on a civil date.
pub fn midnight_utc(year: i32, month: u32, day: u32) -> UnixMs {
    days_from_civil(year, month, day) * DAY
}

/// Howard Hinnant's algorithm: days since 1970-01-01 to (year, month, day).
fn civil_from_days(z: i64) -> (i32, u32, u32) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    ((if m <= 2 { y + 1 } else { y }) as i32, m, d)
}

fn days_from_civil(year: i32, month: u32, day: u32) -> i64 {
    let y = if month <= 2 { year - 1 } else { year } as i64;
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let m = month as i64;
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + day as i64 - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

pub const MONTHS: [&str; 12] = [
    "January",
    "February",
    "March",
    "April",
    "May",
    "June",
    "July",
    "August",
    "September",
    "October",
    "November",
    "December",
];

pub const WEEKDAYS: [&str; 7] = [
    "Thursday",
    "Friday",
    "Saturday",
    "Sunday",
    "Monday",
    "Tuesday",
    "Wednesday",
];

/// The weekday name at `at` on the local clock.
pub fn weekday(at: UnixMs, offset_s: i32) -> &'static str {
    let days = (at + offset_s as i64 * 1000).div_euclid(DAY);
    WEEKDAYS[days.rem_euclid(7) as usize]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn day_zero_is_the_last_day_of_1999() {
        let at = midnight_utc(1999, 12, 31);
        assert!(day_number(at).abs() < 1e-9);
        assert!((day_number(midnight_utc(2000, 1, 1) + 12 * HOUR) - 1.5).abs() < 1e-9);
    }

    #[test]
    fn dates_round_trip() {
        for (y, m, d) in [(1970, 1, 1), (2000, 2, 29), (2026, 9, 28), (2049, 12, 31)] {
            let at = midnight_utc(y, m, d) + 5 * HOUR;
            assert_eq!(civil_date(at, 0), (y, m, d));
        }
        assert_eq!(
            civil_date(midnight_utc(2026, 9, 28) + 23 * HOUR, 3600),
            (2026, 9, 29)
        );
    }

    #[test]
    fn weekdays_are_right() {
        assert_eq!(weekday(midnight_utc(2026, 9, 28) + HOUR, 0), "Monday");
        assert_eq!(weekday(midnight_utc(1970, 1, 1), 0), "Thursday");
    }
}
