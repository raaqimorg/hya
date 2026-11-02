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

/// A prayer whose active window contains `now`: `at` is its own adhan instant, `ends` the
/// (possibly capped) instant its window closes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Active {
    pub(crate) prayer: Prayer,
    pub(crate) at: i64,
    pub(crate) now: i64,
    pub(crate) ends: i64,
}

/// Resolves `prayer`'s window against `times`/`ramadan` and returns `Active` if `now` still
/// falls inside it.
fn active_in(
    prayer: Prayer,
    at: i64,
    now: i64,
    times: &PrayerTimes,
    ramadan: bool,
) -> Option<Active> {
    let minutes = window_minutes(prayer, ramadan).expect("real prayer");
    let ends = capped_end(at, minutes, waqt_end(prayer, times));
    (now < ends).then_some(Active {
        prayer,
        at,
        now,
        ends,
    })
}

/// The prayer, if any, whose active window contains `now`. Checks today's `times` first (the
/// most recently started real prayer); if nothing in today has started yet, falls back to
/// yesterday's Isha across midnight via `yesterday()`.
pub(crate) fn active_at(
    now: i64,
    times: &PrayerTimes,
    ramadan: bool,
    yesterday: impl FnOnce() -> Option<(PrayerTimes, bool)>,
) -> Option<Active> {
    let started_today = Prayer::ALL
        .into_iter()
        .filter(|&prayer| prayer != Prayer::Sunrise)
        .filter(|&prayer| times.get(prayer).unix_seconds() <= now)
        .max_by_key(|&prayer| times.get(prayer).unix_seconds());

    match started_today {
        Some(prayer) => active_in(
            prayer,
            times.get(prayer).unix_seconds(),
            now,
            times,
            ramadan,
        ),
        None => {
            let (yesterday_times, yesterday_ramadan) = yesterday()?;
            let at = yesterday_times.get(Prayer::Isha).unix_seconds();
            active_in(Prayer::Isha, at, now, &yesterday_times, yesterday_ramadan)
        }
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

    #[test]
    fn asr_is_active_just_after_its_own_adhan() {
        let times = riyadh();
        let at = times.get(Prayer::Asr).unix_seconds();
        let active = active_at(at + 5 * 60, &times, false, || None).expect("active");
        assert_eq!(active.prayer, Prayer::Asr);
        assert_eq!(active.at, at);
    }

    #[test]
    fn asr_closes_after_its_window_and_before_maghrib_begins() {
        let times = riyadh();
        let at = times.get(Prayer::Asr).unix_seconds();
        // 30m window: at the boundary instant, it has just closed.
        assert!(active_at(at + 30 * 60, &times, false, || None).is_none());
    }

    #[test]
    fn asr_is_still_active_one_second_before_its_window_closes() {
        let times = riyadh();
        let at = times.get(Prayer::Asr).unix_seconds();
        let active = active_at(at + 30 * 60 - 1, &times, false, || None).expect("active");
        assert_eq!(active.prayer, Prayer::Asr);
    }

    #[test]
    fn nothing_is_active_before_fajr_with_no_prior_isha() {
        let times = riyadh();
        let before_fajr = times.get(Prayer::Fajr).unix_seconds() - 60;
        assert!(active_at(before_fajr, &times, false, || None).is_none());
    }

    #[test]
    fn yesterdays_isha_is_active_across_midnight() {
        let today = riyadh();
        let yesterday = compute(
            Date::new(2026, 10, 28).expect("valid"),
            Coordinates::new(24.7136, 46.6753).expect("valid"),
            Parameters::default(),
        )
        .expect("computable");
        let yesterday_isha = yesterday.get(Prayer::Isha).unix_seconds();
        let before_todays_fajr = today.get(Prayer::Fajr).unix_seconds() - 60;
        assert!(
            before_todays_fajr > yesterday_isha,
            "sanity: still yesterday's window"
        );
        let active = active_at(yesterday_isha + 5 * 60, &today, false, || {
            Some((yesterday, false))
        })
        .expect("active");
        assert_eq!(active.prayer, Prayer::Isha);
        assert_eq!(active.at, yesterday_isha);
    }

    #[test]
    fn todays_ramadan_flag_shortens_todays_own_window() {
        let times = riyadh();
        let at = times.get(Prayer::Isha).unix_seconds();
        // Isha: 35m baseline, 30m in Ramadan. 32 minutes in is still open at baseline, already
        // closed if the Ramadan flag passed to `active_at` is honored.
        assert!(active_at(at + 32 * 60, &times, false, || None).is_some());
        assert!(active_at(at + 32 * 60, &times, true, || None).is_none());
    }

    #[test]
    fn isha_is_active_through_active_at_when_there_is_no_midnight_to_cap_it() {
        let last_day = compute(
            Date::new(2100, 12, 31).expect("valid"),
            Coordinates::new(24.7136, 46.6753).expect("valid"),
            Parameters::default(),
        )
        .expect("computable");
        assert_eq!(last_day.midnight(), None);
        let at = last_day.get(Prayer::Isha).unix_seconds();
        let active = active_at(at + 10 * 60, &last_day, false, || None).expect("active");
        assert_eq!(active.prayer, Prayer::Isha);
    }

    #[test]
    fn yesterdays_ramadan_status_is_used_for_yesterdays_isha_not_todays() {
        let today = riyadh(); // not in Ramadan
        let yesterday = compute(
            Date::new(2026, 10, 28).expect("valid"),
            Coordinates::new(24.7136, 46.6753).expect("valid"),
            Parameters::default(),
        )
        .expect("computable");
        let yesterday_isha = yesterday.get(Prayer::Isha).unix_seconds();
        // Yesterday was (hypothetically) in Ramadan: a 30m window, not 35m. 32 minutes in should
        // already be closed if yesterday's Ramadan status is honored, still open if today's
        // (non-Ramadan, 35m) status is wrongly used instead.
        let active = active_at(yesterday_isha + 32 * 60, &today, false, || {
            Some((yesterday, true))
        });
        assert!(active.is_none(), "must use yesterday's own Ramadan status");
    }
}
