//! Umm al-Qura Ramadan lookup.

use crate::date::Date;
use crate::input::InvalidInput;
use crate::ramadan_table::{RAMADANS, Ramadan};

/// How far an announced start may sit from the calculated one. Sighting moves it by a day or
/// two; anything further is almost certainly a typo (such as the wrong year).
const MAX_SHIFT_DAYS: i32 = 3;

/// Which days count as Ramadan for the Ramadan Isha interval.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RamadanDates {
    /// The Umm al-Qura calendar, as calculated.
    #[default]
    Calculated,
    /// Announced dates for one Ramadan; every other year stays calculated.
    Announced(AnnouncedRamadan),
}

impl RamadanDates {
    /// Whether `date` falls in Ramadan under these dates.
    #[must_use]
    pub fn contains(self, date: Date) -> bool {
        self.contains_day(date.days_since_unix_epoch())
    }

    pub(crate) fn contains_day(self, day: i32) -> bool {
        match self {
            Self::Calculated => contains_day(day),
            Self::Announced(announced) => announced.contains_day(day),
        }
    }
}

/// The announced first and last day of one Ramadan, replacing the calculated dates it is
/// nearest to.
///
/// Only constructible through [`AnnouncedRamadan::new`], which checks that the month has 29 or
/// 30 days and starts within three days of a calculated Ramadan.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AnnouncedRamadan {
    first: Date,
    last: Date,
    replaces: Ramadan,
}

impl AnnouncedRamadan {
    /// Builds an override from the announced first and last day (both inclusive).
    ///
    /// # Errors
    ///
    /// [`InvalidInput::RamadanLength`] unless `first..=last` spans 29 or 30 days;
    /// [`InvalidInput::RamadanTooFarFromCalendar`] unless `first` is within three days of the
    /// start of a calculated Ramadan.
    pub fn new(first: Date, last: Date) -> Result<Self, InvalidInput> {
        let first_day = first.days_since_unix_epoch();
        let end_day = last
            .days_since_unix_epoch()
            .checked_add(1)
            .ok_or(InvalidInput::RamadanLength)?;
        let length = end_day
            .checked_sub(first_day)
            .ok_or(InvalidInput::RamadanLength)?;
        if !(29_i32..=30_i32).contains(&length) {
            return Err(InvalidInput::RamadanLength);
        }
        let replaces = nearest(first_day)
            .filter(|ramadan| {
                ramadan
                    .first_day
                    .checked_sub(first_day)
                    .is_some_and(|shift| shift.abs() <= MAX_SHIFT_DAYS)
            })
            .ok_or(InvalidInput::RamadanTooFarFromCalendar)?;
        Ok(Self {
            first,
            last,
            replaces,
        })
    }

    /// The announced 1 Ramadan.
    #[must_use]
    pub const fn first(self) -> Date {
        self.first
    }

    /// The announced last day of Ramadan (the eve of Eid).
    #[must_use]
    pub const fn last(self) -> Date {
        self.last
    }

    /// Whether both announcements are for the same calculated Ramadan.
    #[must_use]
    pub fn same_month_as(self, other: Self) -> bool {
        self.replaces == other.replaces
    }

    fn contains_day(self, day: i32) -> bool {
        let first = self.first.days_since_unix_epoch();
        let last = self.last.days_since_unix_epoch();
        // Days the calculated month and the announced one both cover or dispute are decided by
        // the announcement; everything else falls through to the calendar.
        let disputed = first.min(self.replaces.first_day) <= day
            && day < (last.saturating_add(1)).max(self.replaces.end_day);
        if disputed {
            first <= day && day <= last
        } else {
            contains_day(day)
        }
    }
}

/// The calculated Ramadan whose start is closest to `day`.
fn nearest(day: i32) -> Option<Ramadan> {
    let index = RAMADANS.partition_point(|ramadan| ramadan.first_day < day);
    let after = RAMADANS.get(index).copied();
    let before = index.checked_sub(1).and_then(|i| RAMADANS.get(i)).copied();
    let distance = |r: Ramadan| r.first_day.abs_diff(day);
    match (before, after) {
        (Some(b), Some(a)) => Some(if distance(b) <= distance(a) { b } else { a }),
        (one, None) | (None, one) => one,
    }
}

/// Whether `date` falls in Ramadan according to the Umm al-Qura calendar.
#[must_use]
pub fn is_ramadan(date: Date) -> bool {
    contains_day(date.days_since_unix_epoch())
}

/// Whether the day index (days since 1970-01-01) falls in Ramadan.
pub(crate) fn contains_day(day: i32) -> bool {
    let index = RAMADANS.partition_point(|ramadan| ramadan.end_day <= day);
    RAMADANS
        .get(index)
        .is_some_and(|ramadan| ramadan.first_day <= day)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn date(year: i16, month: u8, day: u8) -> Date {
        Date::new(year, month, day).expect("valid date")
    }

    #[test]
    fn ramadan_1446() {
        assert!(!is_ramadan(date(2025, 2, 28)), "29 Sha'ban 1446");
        assert!(is_ramadan(date(2025, 3, 1)), "1 Ramadan 1446");
        assert!(is_ramadan(date(2025, 3, 29)), "29 Ramadan 1446");
        assert!(!is_ramadan(date(2025, 3, 30)), "1 Shawwal 1446");
    }

    #[test]
    fn ramadan_1447() {
        assert!(!is_ramadan(date(2026, 2, 17)), "29 Sha'ban 1447");
        assert!(is_ramadan(date(2026, 2, 18)), "1 Ramadan 1447");
        assert!(is_ramadan(date(2026, 3, 19)), "30 Ramadan 1447");
        assert!(!is_ramadan(date(2026, 3, 20)), "1 Shawwal 1447");
    }

    #[test]
    fn table_is_sorted_disjoint_and_plausible() {
        for pair in RAMADANS.windows(2) {
            let [a, b] = pair else { unreachable_pair() };
            assert!(a.end_day <= b.first_day, "overlap at {}", a.hijri_year);
            assert_eq!(
                a.hijri_year.checked_add(1),
                Some(b.hijri_year),
                "gap after {}",
                a.hijri_year
            );
        }
        for ramadan in RAMADANS {
            let length = ramadan.end_day - ramadan.first_day;
            assert!(
                (29_i32..=30_i32).contains(&length),
                "{} has {length} days",
                ramadan.hijri_year
            );
        }
    }

    fn announced(first: Date, last: Date) -> RamadanDates {
        RamadanDates::Announced(AnnouncedRamadan::new(first, last).expect("valid override"))
    }

    #[test]
    fn announcing_the_calculated_dates_changes_nothing() {
        for ramadan in RAMADANS {
            let first = Date::from_days_since_unix_epoch(ramadan.first_day).expect("in range");
            let last = Date::from_days_since_unix_epoch(ramadan.end_day - 1).expect("in range");
            let same = announced(first, last);
            for day in (ramadan.first_day - 40_i32)..(ramadan.end_day + 40_i32) {
                assert_eq!(
                    same.contains_day(day),
                    contains_day(day),
                    "{} day {day}",
                    ramadan.hijri_year
                );
            }
        }
    }

    #[test]
    fn a_late_sighting_moves_both_edges() {
        // Calculated 1447: 2026-02-18..=2026-03-19. Announced one day later, still 30 days.
        let late = announced(date(2026, 2, 19), date(2026, 3, 20));
        assert!(!late.contains(date(2026, 2, 18)), "calculated first day");
        assert!(late.contains(date(2026, 2, 19)), "announced first day");
        assert!(late.contains(date(2026, 3, 19)), "calculated last day");
        assert!(late.contains(date(2026, 3, 20)), "announced last day");
        assert!(!late.contains(date(2026, 3, 21)), "Eid");
        // Other years keep the calendar.
        assert!(late.contains(date(2025, 3, 1)), "1 Ramadan 1446");
        assert!(!late.contains(date(2025, 2, 28)), "29 Sha'ban 1446");
    }

    #[test]
    fn a_short_month_ends_early() {
        let short = announced(date(2026, 2, 18), date(2026, 3, 18));
        assert!(short.contains(date(2026, 3, 18)), "29th");
        assert!(!short.contains(date(2026, 3, 19)), "calculated 30th is Eid");
    }

    #[test]
    fn same_month_means_same_calculated_ramadan() {
        let new = |a, b| AnnouncedRamadan::new(a, b).expect("valid");
        let early = new(date(2026, 2, 17), date(2026, 3, 18));
        let late = new(date(2026, 2, 20), date(2026, 3, 20));
        let next_year = new(date(2027, 2, 8), date(2027, 3, 9));
        assert!(early.same_month_as(late), "both 1447");
        assert!(!early.same_month_as(next_year), "1447 vs 1448");
    }

    #[test]
    fn rejects_impossible_overrides() {
        let new = |a, b| AnnouncedRamadan::new(a, b);
        assert_eq!(
            new(date(2026, 2, 18), date(2026, 3, 17)),
            Err(InvalidInput::RamadanLength),
            "28 days"
        );
        assert_eq!(
            new(date(2026, 2, 18), date(2026, 3, 20)),
            Err(InvalidInput::RamadanLength),
            "31 days"
        );
        assert_eq!(
            new(date(2026, 3, 19), date(2026, 2, 18)),
            Err(InvalidInput::RamadanLength),
            "reversed"
        );
        assert!(
            new(date(2026, 2, 21), date(2026, 3, 21)).is_ok(),
            "3 days late"
        );
        assert!(
            new(date(2026, 2, 15), date(2026, 3, 15)).is_ok(),
            "3 days early"
        );
        assert_eq!(
            new(date(2026, 2, 22), date(2026, 3, 22)),
            Err(InvalidInput::RamadanTooFarFromCalendar),
            "4 days late"
        );
        assert_eq!(
            new(date(2027, 2, 18), date(2027, 3, 19)),
            Err(InvalidInput::RamadanTooFarFromCalendar),
            "wrong year"
        );
    }

    fn unreachable_pair() -> ! {
        panic!("windows(2) always yields pairs")
    }
}
