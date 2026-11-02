//! The active prayer window: how long after its own adhan a prayer is still shown as current.

use hya_engine::{Prayer, PrayerTimes};

/// The window length, in minutes, for `prayer`: the adhan→iqama gap plus a buffer for praying
/// it, chosen from the baseline or Ramadan table. `None` for [`Prayer::Sunrise`], which is
/// never active.
pub(crate) const fn window_minutes(prayer: Prayer, ramadan: bool) -> Option<u32> {
    match (prayer, ramadan) {
        (Prayer::Sunrise, _) => None,
        (Prayer::Fajr, false) => Some(40),
        (Prayer::Fajr, true) => Some(30),
        (Prayer::Dhuhr, _) => Some(30),
        (Prayer::Asr, _) => Some(30),
        (Prayer::Maghrib, false) => Some(20),
        (Prayer::Maghrib, true) => Some(25),
        (Prayer::Isha, false) => Some(35),
        (Prayer::Isha, true) => Some(30),
    }
}

/// The Unix-second instant after which `prayer`'s own waqt has ended — the hard cap on its
/// active window. `None` when there is no earlier boundary than the window's own length
/// (`Prayer::Sunrise`, which is moot since it's never active, and `Prayer::Isha` when
/// `times.midnight()` is `None`).
pub(crate) fn waqt_end(prayer: Prayer, times: &PrayerTimes) -> Option<i64> {
    match prayer {
        Prayer::Sunrise => None,
        Prayer::Fajr => Some(times.sunrise().unix_seconds()),
        Prayer::Dhuhr => Some(times.asr().unix_seconds()),
        Prayer::Asr => Some(times.maghrib().unix_seconds()),
        Prayer::Maghrib => Some(times.isha().unix_seconds()),
        Prayer::Isha => times.midnight().map(|instant| instant.unix_seconds()),
    }
}

/// The active window's end instant: `start` plus `window_minutes`, capped at `boundary` when
/// one applies.
pub(crate) fn capped_end(start: i64, window_minutes: u32, boundary: Option<i64>) -> i64 {
    let natural_end = start.saturating_add(i64::from(window_minutes).saturating_mul(60));
    match boundary {
        Some(boundary) => natural_end.min(boundary),
        None => natural_end,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hya_engine::{Coordinates, Date, Parameters, PrayerTimes, compute};

    fn riyadh() -> PrayerTimes {
        compute(
            Date::new(2026, 10, 29).expect("valid"),
            Coordinates::new(24.7136, 46.6753).expect("valid"),
            Parameters::default(),
        )
        .expect("computable")
    }

    #[test]
    fn baseline_minutes_match_the_table() {
        assert_eq!(window_minutes(Prayer::Fajr, false), Some(40));
        assert_eq!(window_minutes(Prayer::Dhuhr, false), Some(30));
        assert_eq!(window_minutes(Prayer::Asr, false), Some(30));
        assert_eq!(window_minutes(Prayer::Maghrib, false), Some(20));
        assert_eq!(window_minutes(Prayer::Isha, false), Some(35));
    }

    #[test]
    fn ramadan_minutes_match_the_table() {
        assert_eq!(window_minutes(Prayer::Fajr, true), Some(30));
        assert_eq!(window_minutes(Prayer::Dhuhr, true), Some(30));
        assert_eq!(window_minutes(Prayer::Asr, true), Some(30));
        assert_eq!(window_minutes(Prayer::Maghrib, true), Some(25));
        assert_eq!(window_minutes(Prayer::Isha, true), Some(30));
    }

    #[test]
    fn sunrise_is_never_active() {
        assert_eq!(window_minutes(Prayer::Sunrise, false), None);
        assert_eq!(window_minutes(Prayer::Sunrise, true), None);
    }

    // capped_end: the three worked examples from the design doc, in seconds-from-start terms.
    #[test]
    fn capped_end_is_inert_when_the_boundary_is_later() {
        // Riyadh: window 40m, Sunrise 78m after start.
        assert_eq!(capped_end(0, 40, Some(78 * 60)), 40 * 60);
    }

    #[test]
    fn capped_end_shortens_the_window_when_the_boundary_is_earlier() {
        // Short summer night: window 40m, Sunrise only 35m after start.
        assert_eq!(capped_end(0, 40, Some(35 * 60)), 35 * 60);
    }

    #[test]
    fn capped_end_is_the_natural_length_without_a_boundary() {
        assert_eq!(capped_end(0, 35, None), 35 * 60);
    }

    // waqt_end: picks the right field out of a real schedule.
    #[test]
    fn waqt_end_boundaries_match_the_table() {
        let times = riyadh();
        assert_eq!(
            waqt_end(Prayer::Fajr, &times),
            Some(times.sunrise().unix_seconds())
        );
        assert_eq!(
            waqt_end(Prayer::Dhuhr, &times),
            Some(times.asr().unix_seconds())
        );
        assert_eq!(
            waqt_end(Prayer::Asr, &times),
            Some(times.maghrib().unix_seconds())
        );
        assert_eq!(
            waqt_end(Prayer::Maghrib, &times),
            Some(times.isha().unix_seconds())
        );
        assert_eq!(
            waqt_end(Prayer::Isha, &times),
            Some(times.midnight().expect("riyadh has one").unix_seconds())
        );
    }

    #[test]
    fn isha_is_uncapped_without_a_midnight() {
        // Same fixture the existing `without_a_next_fajr_there_is_no_midnight` test uses.
        let last_day = compute(
            Date::new(2100, 12, 31).expect("valid"),
            Coordinates::new(24.7136, 46.6753).expect("valid"),
            Parameters::default(),
        )
        .expect("computable");
        assert_eq!(last_day.midnight(), None);
        assert_eq!(waqt_end(Prayer::Isha, &last_day), None);
    }
}
