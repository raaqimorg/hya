//! The active prayer window: how long after its own adhan a prayer is still shown as current.

use hya_engine::{Instant, Prayer, PrayerTimes};

/// The window length, in minutes, for `prayer`: the adhan→iqama gap plus a buffer for praying
/// it, chosen from the baseline or Ramadan table. `None` for [`Prayer::Sunrise`], which is
/// never active.
pub(crate) const fn window_minutes(prayer: Prayer, ramadan: bool) -> Option<u32> {
    match (prayer, ramadan) {
        (Prayer::Sunrise, _) => None,
        (Prayer::Fajr, false) => Some(40),
        (Prayer::Fajr | Prayer::Isha, true) | (Prayer::Dhuhr | Prayer::Asr, _) => Some(30),
        (Prayer::Maghrib, false) => Some(20),
        (Prayer::Maghrib, true) => Some(25),
        (Prayer::Isha, false) => Some(35),
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
        Prayer::Isha => times.midnight().map(Instant::unix_seconds),
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
    let minutes = window_minutes(prayer, ramadan)?;
    let ends = capped_end(at, minutes, waqt_end(prayer, times));
    (at <= now && now < ends).then_some(Active {
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

    if let Some(prayer) = started_today {
        active_in(
            prayer,
            times.get(prayer).unix_seconds(),
            now,
            times,
            ramadan,
        )
    } else {
        let (yesterday_times, yesterday_ramadan) = yesterday()?;
        let at = yesterday_times.get(Prayer::Isha).unix_seconds();
        active_in(Prayer::Isha, at, now, &yesterday_times, yesterday_ramadan)
    }
}

/// Every prayer's own adhan instant and the instant its active window closes, for
/// `times`/`ramadan`. `ends` is `None` only for [`Prayer::Sunrise`], which has no window.
pub(crate) fn windows_for(times: &PrayerTimes, ramadan: bool) -> [(Prayer, i64, Option<i64>); 6] {
    Prayer::ALL.map(|prayer| {
        let at = times.get(prayer).unix_seconds();
        let ends = window_minutes(prayer, ramadan)
            .map(|minutes| capped_end(at, minutes, waqt_end(prayer, times)));
        (prayer, at, ends)
    })
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

    #[test]
    fn capped_end_is_inert_when_the_boundary_is_later() {
        assert_eq!(capped_end(0, 40, Some(78 * 60)), 40 * 60);
    }

    #[test]
    fn capped_end_shortens_the_window_when_the_boundary_is_earlier() {
        assert_eq!(capped_end(0, 40, Some(35 * 60)), 35 * 60);
    }

    #[test]
    fn capped_end_is_the_natural_length_without_a_boundary() {
        assert_eq!(capped_end(0, 35, None), 35 * 60);
    }

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
    fn fajrs_window_is_capped_at_sunrise_when_shorter_than_its_own_length() {
        let times = compute(
            Date::new(2026, 6, 21).expect("valid"),
            Coordinates::new(65.0121, 25.4651).expect("valid"),
            Parameters::default(),
        )
        .expect("computable");
        let fajr = times.get(Prayer::Fajr).unix_seconds();
        let sunrise = times.sunrise().unix_seconds();
        assert_eq!(sunrise - fajr, 16 * 60, "sanity: a 16-minute gap");

        let active = active_at(sunrise - 1, &times, false, || None).expect("active");
        assert_eq!(active.prayer, Prayer::Fajr);
        assert_eq!(active.ends, sunrise);
        assert!(active_at(sunrise, &times, false, || None).is_none());
    }

    #[test]
    fn nothing_is_active_before_yesterdays_isha_has_begun() {
        let today = riyadh();
        let yesterday = compute(
            Date::new(2026, 10, 28).expect("valid"),
            Coordinates::new(24.7136, 46.6753).expect("valid"),
            Parameters::default(),
        )
        .expect("computable");
        let yesterday_isha = yesterday.get(Prayer::Isha).unix_seconds();
        assert!(
            active_at(yesterday_isha - 60, &today, false, || Some((
                yesterday, false
            )))
            .is_none(),
            "a prayer cannot be active before its own adhan"
        );
    }

    #[test]
    fn todays_ramadan_flag_shortens_todays_own_window() {
        let times = riyadh();
        let at = times.get(Prayer::Isha).unix_seconds();
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
    fn windows_for_matches_active_at_for_every_today_prayer() {
        let times = riyadh();
        for (prayer, at, ends) in windows_for(&times, false) {
            if prayer == Prayer::Sunrise {
                assert_eq!(ends, None);
                continue;
            }
            let active =
                active_in(prayer, at, at + 1, &times, false).expect("active just after at");
            assert_eq!(Some(active.ends), ends);
        }
    }

    #[test]
    fn windows_for_ramadan_shortens_the_same_entries_as_window_minutes() {
        let times = riyadh();
        let baseline = windows_for(&times, false);
        let ramadan = windows_for(&times, true);
        let isha = Prayer::ALL
            .iter()
            .position(|&p| p == Prayer::Isha)
            .expect("isha");
        assert!(
            ramadan[isha].2 < baseline[isha].2,
            "Isha's window is shorter in Ramadan"
        );
    }

    #[test]
    fn yesterdays_ramadan_status_is_used_for_yesterdays_isha_not_todays() {
        let today = riyadh();
        let yesterday = compute(
            Date::new(2026, 10, 28).expect("valid"),
            Coordinates::new(24.7136, 46.6753).expect("valid"),
            Parameters::default(),
        )
        .expect("computable");
        let yesterday_isha = yesterday.get(Prayer::Isha).unix_seconds();
        let active = active_at(yesterday_isha + 32 * 60, &today, false, || {
            Some((yesterday, true))
        });
        assert!(active.is_none(), "must use yesterday's own Ramadan status");
    }
}
