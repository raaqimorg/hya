//! Calendar dates (the input) and UTC instants (the output).

use core::fmt;

use crate::input::InvalidInput;

/// A Gregorian calendar date within [`Date::MIN_YEAR`]..=[`Date::MAX_YEAR`].
///
/// The range is bounded by the bundled Umm al-Qura Ramadan table. Ordering follows the calendar.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Date {
    // Field order matters: the derived `Ord` compares `days` first, which is calendar order.
    days: i32,
    year: i16,
    month: u8,
    day: u8,
}

impl Date {
    /// Earliest supported year.
    pub const MIN_YEAR: i16 = 1900;
    /// Latest supported year.
    pub const MAX_YEAR: i16 = 2100;

    /// Validates a calendar date.
    ///
    /// # Errors
    ///
    /// [`InvalidInput::DateOutOfRange`] outside the supported years,
    /// [`InvalidInput::NonexistentDate`] for impossible month/day combinations.
    pub fn new(year: i16, month: u8, day: u8) -> Result<Self, InvalidInput> {
        if !(Self::MIN_YEAR..=Self::MAX_YEAR).contains(&year) {
            return Err(InvalidInput::DateOutOfRange);
        }
        let last = days_in_month(year, month).ok_or(InvalidInput::NonexistentDate)?;
        if day == 0 || day > last {
            return Err(InvalidInput::NonexistentDate);
        }
        let days = days_from_civil(year, month, day).ok_or(InvalidInput::DateOutOfRange)?;
        Ok(Self {
            days,
            year,
            month,
            day,
        })
    }

    /// The date `days` after 1970-01-01.
    ///
    /// # Errors
    ///
    /// [`InvalidInput::DateOutOfRange`] when the result falls outside the supported years.
    pub fn from_days_since_unix_epoch(days: i32) -> Result<Self, InvalidInput> {
        let (year, month, day) = civil_from_days(days).ok_or(InvalidInput::DateOutOfRange)?;
        Self::new(year, month, day)
    }

    /// The following calendar day.
    ///
    /// # Errors
    ///
    /// [`InvalidInput::DateOutOfRange`] after 31 December of [`Date::MAX_YEAR`].
    pub fn next(self) -> Result<Self, InvalidInput> {
        let days = self
            .days
            .checked_add(1)
            .ok_or(InvalidInput::DateOutOfRange)?;
        Self::from_days_since_unix_epoch(days)
    }

    /// Calendar year.
    #[must_use]
    pub const fn year(self) -> i16 {
        self.year
    }

    /// Month, `1..=12`.
    #[must_use]
    pub const fn month(self) -> u8 {
        self.month
    }

    /// Day of month, `1..=31`.
    #[must_use]
    pub const fn day(self) -> u8 {
        self.day
    }

    /// Days since 1970-01-01.
    #[must_use]
    pub const fn days_since_unix_epoch(self) -> i32 {
        self.days
    }
}

impl fmt::Display for Date {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:04}-{:02}-{:02}", self.year, self.month, self.day)
    }
}

pub(crate) const fn is_leap_year(year: i16) -> bool {
    (year % 4 == 0 && year % 100 != 0) || year % 400 == 0
}

pub(crate) const fn days_in_month(year: i16, month: u8) -> Option<u8> {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => Some(31),
        4 | 6 | 9 | 11 => Some(30),
        2 if is_leap_year(year) => Some(29),
        2 => Some(28),
        _ => None,
    }
}

/// Howard Hinnant's `days_from_civil`, with every step checked.
pub(crate) fn days_from_civil(year: i16, month: u8, day: u8) -> Option<i32> {
    let year = i32::from(year);
    let shifted_year = if month <= 2 {
        year.checked_sub(1)?
    } else {
        year
    };
    let era = shifted_year.checked_div_euclid(400)?;
    let year_of_era = shifted_year.checked_rem_euclid(400)?;
    let shifted_month = i32::from(month).checked_add(9)?.checked_rem(12)?;
    let day_of_year = shifted_month
        .checked_mul(153)?
        .checked_add(2)?
        .checked_div(5)?
        .checked_add(i32::from(day))?
        .checked_sub(1)?;
    let day_of_era = year_of_era
        .checked_mul(365)?
        .checked_add(year_of_era.checked_div(4)?)?
        .checked_sub(year_of_era.checked_div(100)?)?
        .checked_add(day_of_year)?;
    era.checked_mul(146_097)?
        .checked_add(day_of_era)?
        .checked_sub(719_468)
}

/// Howard Hinnant's `civil_from_days`, with every step checked.
pub(crate) fn civil_from_days(days: i32) -> Option<(i16, u8, u8)> {
    let shifted = days.checked_add(719_468)?;
    let era = shifted.checked_div_euclid(146_097)?;
    let day_of_era = shifted.checked_rem_euclid(146_097)?;
    let year_of_era = day_of_era
        .checked_sub(day_of_era.checked_div(1460)?)?
        .checked_add(day_of_era.checked_div(36_524)?)?
        .checked_sub(day_of_era.checked_div(146_096)?)?
        .checked_div(365)?;
    let day_of_year = day_of_era.checked_sub(
        year_of_era
            .checked_mul(365)?
            .checked_add(year_of_era.checked_div(4)?)?
            .checked_sub(year_of_era.checked_div(100)?)?,
    )?;
    let shifted_month = day_of_year
        .checked_mul(5)?
        .checked_add(2)?
        .checked_div(153)?;
    let day = day_of_year
        .checked_sub(
            shifted_month
                .checked_mul(153)?
                .checked_add(2)?
                .checked_div(5)?,
        )?
        .checked_add(1)?;
    let month = if shifted_month < 10_i32 {
        shifted_month.checked_add(3)?
    } else {
        shifted_month.checked_sub(9)?
    };
    let year = year_of_era.checked_add(era.checked_mul(400)?)?;
    let year = if month <= 2_i32 {
        year.checked_add(1)?
    } else {
        year
    };
    Some((
        i16::try_from(year).ok()?,
        u8::try_from(month).ok()?,
        u8::try_from(day).ok()?,
    ))
}

/// A UTC instant, rounded to the minute.
///
/// The core never deals in time zones. Convert with [`Instant::unix_seconds`] or
/// [`Instant::unix_millis`] and format in the caller's zone.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Instant {
    unix_minutes: i64,
}

impl Instant {
    pub(crate) const fn from_unix_minutes(unix_minutes: i64) -> Self {
        Self { unix_minutes }
    }

    /// Whole minutes since 1970-01-01T00:00Z.
    #[must_use]
    pub const fn unix_minutes(self) -> i64 {
        self.unix_minutes
    }

    /// Seconds since 1970-01-01T00:00Z.
    ///
    /// Instants only come from [`crate::compute`] for years 1900..=2100, so this never saturates.
    #[must_use]
    pub const fn unix_seconds(self) -> i64 {
        self.unix_minutes.saturating_mul(60)
    }

    /// Milliseconds since 1970-01-01T00:00Z, the unit JavaScript's `Date` uses.
    #[must_use]
    pub const fn unix_millis(self) -> i64 {
        self.unix_minutes.saturating_mul(60_000)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unix_epoch_is_day_zero() {
        assert_eq!(days_from_civil(1970, 1, 1), Some(0_i32), "epoch");
        assert_eq!(civil_from_days(0), Some((1970, 1, 1)), "epoch inverse");
    }

    #[test]
    fn known_dates() {
        assert_eq!(days_from_civil(2000, 3, 1), Some(11_017_i32), "2000-03-01");
        assert_eq!(days_from_civil(1900, 1, 1), Some(-25_567_i32), "1900-01-01");
        assert_eq!(
            days_from_civil(2100, 12, 31),
            Some(47_846_i32),
            "2100-12-31"
        );
    }

    #[test]
    fn leap_years() {
        assert!(is_leap_year(2000), "divisible by 400");
        assert!(!is_leap_year(1900), "divisible by 100 only");
        assert!(is_leap_year(2024), "divisible by 4");
        assert!(!is_leap_year(2026), "ordinary year");
    }

    #[test]
    fn rejects_impossible_dates() {
        assert_eq!(
            Date::new(2026, 2, 29),
            Err(InvalidInput::NonexistentDate),
            "feb 29"
        );
        assert_eq!(
            Date::new(2026, 13, 1),
            Err(InvalidInput::NonexistentDate),
            "month 13"
        );
        assert_eq!(
            Date::new(2026, 4, 0),
            Err(InvalidInput::NonexistentDate),
            "day 0"
        );
        assert_eq!(
            Date::new(1899, 12, 31),
            Err(InvalidInput::DateOutOfRange),
            "too early"
        );
        assert_eq!(
            Date::new(2101, 1, 1),
            Err(InvalidInput::DateOutOfRange),
            "too late"
        );
    }

    #[test]
    fn next_crosses_boundaries() {
        let last = Date::new(2024, 2, 28).expect("valid");
        assert_eq!(last.next(), Date::new(2024, 2, 29), "leap day");
        assert_eq!(
            Date::new(2100, 12, 31).expect("valid").next(),
            Err(InvalidInput::DateOutOfRange),
            "end of range"
        );
    }
}
