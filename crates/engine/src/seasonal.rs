//! Moonsighting Committee Worldwide's season-adjusted twilight (Khalid Shaukat).
//!
//! Instead of one depression angle all year, twilight lasts a number of minutes that varies
//! piecewise-linearly through the year between four values anchored at the winter solstice,
//! each growing with latitude. The coefficients are the committee's published ones, as
//! adhan-js implements them.

use crate::date::{Date, days_from_civil, is_leap_year};
use crate::method::Shafaq;
use crate::solar::Side;

/// Days since the local winter solstice, taken as 21 December in the north and 21 June in the
/// south. Always within `0..365` (or `0..366` in a leap year); `None` is unreachable for a
/// valid [`Date`].
pub(crate) fn days_since_solstice(date: Date, latitude: f64) -> Option<i32> {
    let year = date.year();
    let leap = is_leap_year(year);
    let day_of_year = date
        .days_since_unix_epoch()
        .checked_sub(days_from_civil(year, 1, 1)?)?
        .checked_add(1)?;
    let shifted = if latitude >= 0.0_f64 {
        day_of_year.checked_add(10)?
    } else {
        day_of_year.checked_sub(if leap { 173 } else { 172 })?
    };
    shifted.checked_rem_euclid(if leap { 366 } else { 365 })
}

/// Twilight length in minutes, before rounding.
pub(crate) fn minutes(side: Side, shafaq: Shafaq, latitude: f64, days_since_solstice: i32) -> f64 {
    // (base, per-55°-of-latitude growth at the solstice, 91 days on, 137 days on, 183 days on)
    let (base, [a, b, c, d]) = match (side, shafaq) {
        (Side::Morning, _) => (75.0_f64, [28.65_f64, 19.44_f64, 32.74_f64, 48.1_f64]),
        (Side::Evening, Shafaq::General) => (75.0_f64, [25.6_f64, 2.05_f64, -9.21_f64, 6.14_f64]),
        (Side::Evening, Shafaq::Red) => (62.0_f64, [17.4_f64, -7.16_f64, 5.12_f64, 19.44_f64]),
        (Side::Evening, Shafaq::White) => (75.0_f64, [25.6_f64, 7.16_f64, 36.84_f64, 81.84_f64]),
    };
    let scale = libm::fabs(latitude) / 55.0_f64;
    let [a, b, c, d] = [a, b, c, d].map(|growth| base + growth * scale);
    let day = f64::from(days_since_solstice);
    // Solstice → equinox → … → solstice → … → back, as adhan-js segments it.
    let (from, to, start, span) = if day < 91.0_f64 {
        (a, b, 0.0_f64, 91.0_f64)
    } else if day < 137.0_f64 {
        (b, c, 91.0_f64, 46.0_f64)
    } else if day < 183.0_f64 {
        (c, d, 137.0_f64, 46.0_f64)
    } else if day < 229.0_f64 {
        (d, c, 183.0_f64, 46.0_f64)
    } else if day < 275.0_f64 {
        (c, b, 229.0_f64, 46.0_f64)
    } else {
        (b, a, 275.0_f64, 91.0_f64)
    };
    from + (to - from) / span * (day - start)
}

/// Twilight length in hours, rounded to the whole second as the committee's tables are.
pub(crate) fn twilight_hours(side: Side, shafaq: Shafaq, latitude: f64, date: Date) -> Option<f64> {
    let days = days_since_solstice(date, latitude)?;
    Some(libm::round(minutes(side, shafaq, latitude, days) * 60.0_f64) / 3600.0_f64)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn date(year: i16, month: u8, day: u8) -> Date {
        Date::new(year, month, day).expect("valid date")
    }

    #[test]
    fn solstice_offsets() {
        assert_eq!(
            days_since_solstice(date(2026, 12, 21), 51.5),
            Some(0_i32),
            "north, 21 Dec"
        );
        assert_eq!(
            days_since_solstice(date(2026, 1, 1), 51.5),
            Some(11_i32),
            "north, 1 Jan"
        );
        assert_eq!(
            days_since_solstice(date(2026, 6, 21), -33.9),
            Some(0_i32),
            "south, 21 Jun"
        );
        assert_eq!(
            days_since_solstice(date(2024, 6, 21), -33.9),
            Some(0_i32),
            "south, leap year"
        );
        assert_eq!(
            days_since_solstice(date(2026, 1, 1), -33.9),
            Some(194_i32),
            "south, 1 Jan"
        );
        assert_eq!(
            days_since_solstice(date(2024, 12, 31), 0.0),
            Some(10_i32),
            "leap year end"
        );
    }

    #[test]
    fn segment_anchors() {
        // At latitude 0 every value is the base.
        for day in [0_i32, 91_i32, 137_i32, 183_i32, 229_i32, 275_i32, 364_i32] {
            assert!(
                (minutes(Side::Morning, Shafaq::General, 0.0, day) - 75.0).abs() < 1e-9_f64,
                "{day}"
            );
            assert!(
                (minutes(Side::Evening, Shafaq::Red, 0.0, day) - 62.0).abs() < 1e-9_f64,
                "{day}"
            );
        }
        // At 55°, the anchors are base + coefficient.
        let at = |side, shafaq, day| minutes(side, shafaq, 55.0, day);
        let close = |x: f64, y: f64| (x - y).abs() < 1e-9_f64;
        assert!(
            close(at(Side::Morning, Shafaq::General, 0_i32), 103.65_f64),
            "a"
        );
        assert!(
            close(at(Side::Morning, Shafaq::General, 91_i32), 94.44_f64),
            "b"
        );
        assert!(
            close(at(Side::Morning, Shafaq::General, 137_i32), 107.74_f64),
            "c"
        );
        assert!(
            close(at(Side::Morning, Shafaq::General, 183_i32), 123.1_f64),
            "d"
        );
        assert!(
            close(at(Side::Morning, Shafaq::General, 229_i32), 107.74_f64),
            "c again"
        );
        assert!(
            close(at(Side::Morning, Shafaq::General, 275_i32), 94.44_f64),
            "b again"
        );
        assert!(
            close(at(Side::Evening, Shafaq::General, 137_i32), 65.79_f64),
            "general c"
        );
        assert!(
            close(at(Side::Evening, Shafaq::Red, 91_i32), 54.84_f64),
            "red b"
        );
        assert!(
            close(at(Side::Evening, Shafaq::White, 183_i32), 156.84_f64),
            "white d"
        );
    }

    #[test]
    fn continuous_across_the_year() {
        for shafaq in [Shafaq::General, Shafaq::Red, Shafaq::White] {
            for side in [Side::Morning, Side::Evening] {
                for day in 0_i32..366_i32 {
                    let next = if day == 365_i32 { 0_i32 } else { day + 1_i32 };
                    let step = minutes(side, shafaq, 55.0, next) - minutes(side, shafaq, 55.0, day);
                    assert!(
                        step.abs() < 1.0_f64,
                        "{side:?} {shafaq:?} day {day}: {step}"
                    );
                }
            }
        }
    }
}
