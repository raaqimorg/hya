//! Sequential ("walk the calendar") tests: compute every day for years at a time and check how
//! results evolve from one day to the next. Catches discontinuities a single-day test cannot.

#![expect(
    clippy::expect_used,
    reason = "test-only helpers; a failed precondition should abort the test loudly"
)]

use hya_engine::{
    Coordinates, Date, HighLatitudeRule, Method, Parameters, Prayer, compute, is_ramadan,
};

fn walk(from: Date, days: u32) -> impl Iterator<Item = Date> {
    core::iter::successors(Some(from), |date| date.next().ok())
        .take(usize::try_from(days).expect("fits"))
}

/// Day-to-day change of each time, in minutes, once the 1440-minute day is removed.
fn daily_drift(a: i64, b: i64) -> u64 {
    b.saturating_sub(a).saturating_sub(1440).unsigned_abs()
}

const CITIES: [(&str, f64, f64, Method); 6] = [
    ("makkah", 21.4225, 39.8262, Method::UmmAlQura),
    ("cairo", 30.0444, 31.2357, Method::Egyptian),
    ("jakarta", -6.2088, 106.8456, Method::Singapore),
    ("toronto", 43.6532, -79.3832, Method::NorthAmerica),
    ("london", 51.5074, -0.1278, Method::MuslimWorldLeague),
    ("oslo", 59.9139, 10.7522, Method::MuslimWorldLeague),
];

#[test]
fn times_drift_smoothly_day_to_day() {
    let start = Date::new(2024, 1, 1).expect("valid");
    for (city, lat, lng, method) in CITIES {
        let coords = Coordinates::new(lat, lng).expect("valid");
        let params = Parameters {
            high_latitude: HighLatitudeRule::SeventhOfTheNight,
            ..method.parameters()
        };
        let mut previous: Option<(Date, hya_engine::PrayerTimes)> = None;
        for date in walk(start, 3 * 366) {
            let times =
                compute(date, coords, params).unwrap_or_else(|e| panic!("{city} {date}: {e}"));
            if let Some((_, before)) = previous {
                for prayer in Prayer::ALL {
                    let drift = daily_drift(
                        before.get(prayer).unix_minutes(),
                        times.get(prayer).unix_minutes(),
                    );
                    // The Ramadan interval switch is the one legitimate jump (±30 min).
                    let ramadan_switch = prayer == Prayer::Isha
                        && before.notes().ramadan_isha != times.notes().ramadan_isha;
                    let limit = if ramadan_switch { 31 } else { 9 };
                    assert!(drift <= limit, "{city} {date} {prayer}: drift {drift} min");
                }
            }
            previous = Some((date, times));
        }
    }
}

#[test]
fn ramadan_isha_tracks_the_umm_al_qura_calendar_exactly() {
    let makkah = Coordinates::new(21.4225, 39.8262).expect("valid");
    let params = Method::UmmAlQura.parameters();
    let mut switches = 0_u32;
    let mut previous = None;
    for date in walk(Date::new(2020, 1, 1).expect("valid"), 10 * 366) {
        let times = compute(date, makkah, params).expect("computable");
        // By calendar date: a day in Ramadan gets the Ramadan interval that evening.
        let in_ramadan = is_ramadan(date);
        assert_eq!(times.notes().ramadan_isha, in_ramadan, "{date}");
        let gap = times
            .isha()
            .unix_minutes()
            .saturating_sub(times.maghrib().unix_minutes());
        assert_eq!(gap, if in_ramadan { 120 } else { 90 }, "{date}");
        if previous.is_some_and(|flag| flag != in_ramadan) {
            switches = switches.saturating_add(1);
        }
        previous = Some(in_ramadan);
    }
    // Ten years contain ten Ramadans: an on and an off switch each.
    assert!((19..=21).contains(&switches), "{switches} switches");
}

#[test]
fn high_latitude_rule_engages_in_summer_but_not_mid_latitude_winter() {
    // At ~60°N even winter twilight is about a seventh of the night, so the clamp can engage
    // year-round there (adhan-js agrees). At 51.5°N winter twilight is clearly shorter.
    let params = Parameters {
        high_latitude: HighLatitudeRule::SeventhOfTheNight,
        ..Method::MuslimWorldLeague.parameters()
    };
    let oslo = Coordinates::new(59.9139, 10.7522).expect("valid");
    let london = Coordinates::new(51.5074, -0.1278).expect("valid");
    for date in walk(Date::new(2026, 1, 1).expect("valid"), 365) {
        if (5..=7).contains(&date.month()) {
            let notes = compute(date, oslo, params).expect("computable").notes();
            assert!(
                notes.fajr_high_latitude && notes.isha_high_latitude,
                "{date} oslo summer"
            );
        }
        if date.month() == 12 || date.month() == 1 {
            let notes = compute(date, london, params).expect("computable").notes();
            assert!(
                !notes.fajr_high_latitude && !notes.isha_high_latitude,
                "{date} london winter"
            );
        }
    }
}
