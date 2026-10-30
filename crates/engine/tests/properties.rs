//! Property-based and metamorphic tests: invariants that must hold for *every* input, checked
//! on thousands of random inputs per run (`PROPTEST_CASES=100000` for a deep run).

#![expect(
    clippy::expect_used,
    clippy::unreachable,
    reason = "test-only helpers; a failed precondition should abort the test loudly"
)]

use hya_engine::{
    Adjustments, AnnouncedRamadan, AsrShadow, ComputeError, Coordinates, Date, Elevation,
    HighLatitudeRule, InvalidInput, IshaRule, Latitude, Longitude, Method, MinuteAdjustment,
    Parameters, Prayer, PrayerTimes, RamadanDates, Shafaq, TwilightAngle, compute, is_ramadan,
};
use proptest::prelude::*;

fn cases() -> u32 {
    std::env::var("PROPTEST_CASES")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(2_000)
}

// ---------- generators ----------

fn any_date() -> impl Strategy<Value = Date> {
    // 1900-01-01 ..= 2100-12-31 as day indexes.
    (-25_567_i32..=47_846)
        .prop_map(|days| Date::from_days_since_unix_epoch(days).expect("in range"))
}

fn any_method() -> impl Strategy<Value = Method> {
    proptest::sample::select(Method::ALL.to_vec())
}

fn any_rule() -> impl Strategy<Value = HighLatitudeRule> {
    proptest::sample::select(vec![
        HighLatitudeRule::Recommended,
        HighLatitudeRule::MiddleOfTheNight,
        HighLatitudeRule::SeventhOfTheNight,
        HighLatitudeRule::TwilightAngle,
        HighLatitudeRule::None,
        HighLatitudeRule::SeasonAdjusted(Shafaq::General),
        HighLatitudeRule::SeasonAdjusted(Shafaq::Red),
        HighLatitudeRule::SeasonAdjusted(Shafaq::White),
    ])
}

/// Generated directly, not filtered: filters hit proptest's reject cap on million-case runs.
fn fallback_rule() -> impl Strategy<Value = HighLatitudeRule> {
    proptest::sample::select(vec![
        HighLatitudeRule::Recommended,
        HighLatitudeRule::MiddleOfTheNight,
        HighLatitudeRule::SeventhOfTheNight,
        HighLatitudeRule::TwilightAngle,
        HighLatitudeRule::SeasonAdjusted(Shafaq::General),
        HighLatitudeRule::SeasonAdjusted(Shafaq::Red),
        HighLatitudeRule::SeasonAdjusted(Shafaq::White),
    ])
}

fn any_shafaq() -> impl Strategy<Value = Shafaq> {
    proptest::sample::select(vec![Shafaq::General, Shafaq::Red, Shafaq::White])
}

fn any_asr() -> impl Strategy<Value = AsrShadow> {
    proptest::sample::select(vec![AsrShadow::Standard, AsrShadow::Hanafi])
}

fn coordinates(max_latitude: f64) -> impl Strategy<Value = Coordinates> {
    (
        -max_latitude..=max_latitude,
        -180.0_f64..=180.0,
        0.0_f64..=3000.0,
    )
        .prop_map(|(lat, lng, elev)| {
            Coordinates::new(lat, lng)
                .expect("in range")
                .with_elevation(Elevation::new(elev).expect("in range"))
        })
}

fn parameters() -> impl Strategy<Value = Parameters> {
    (any_method(), any_asr(), any_rule()).prop_map(|(method, asr, rule)| Parameters {
        asr,
        high_latitude: rule,
        ..method.parameters()
    })
}

fn day(index: i32) -> Option<Date> {
    Date::from_days_since_unix_epoch(index).ok()
}

/// The first and last day of the first calculated Ramadan starting on or after `from`.
fn calculated_ramadan_from(from: Date) -> Option<(i32, i32)> {
    let start = from.days_since_unix_epoch();
    let first = (start..start.checked_add(400)?).find(|&d| {
        day(d).is_some_and(is_ramadan) && !d.checked_sub(1).and_then(day).is_some_and(is_ramadan)
    })?;
    let end = (first..first.checked_add(31)?).find(|&d| !day(d).is_some_and(is_ramadan))?;
    Some((first, end.checked_sub(1)?))
}

/// An announced Ramadan within the allowed distance of a calculated one.
fn announced(from: Date, shift: i32, length: i32) -> Option<RamadanDates> {
    let (first, _) = calculated_ramadan_from(from)?;
    let first = first.checked_add(shift)?;
    let dates = AnnouncedRamadan::new(
        day(first)?,
        day(first.checked_add(length.checked_sub(1)?)?)?,
    )
    .ok()?;
    Some(RamadanDates::Announced(dates))
}

fn minutes(times: &PrayerTimes, prayer: Prayer) -> i64 {
    times.get(prayer).unix_minutes()
}

/// Minutes since the start of the requested UTC day.
fn day_offset(date: Date, times: &PrayerTimes, prayer: Prayer) -> i64 {
    minutes(times, prayer)
        .saturating_sub(i64::from(date.days_since_unix_epoch()).saturating_mul(1440))
}

/// `num/den` of the way from `from` to `to`, rounded to the nearest minute, a half up:
/// ⌊(2·night·num + den) / (2·den)⌋, independent of how the engine computes it.
fn fraction_of_night(from: i64, to: i64, num: i64, den: i64) -> Option<i64> {
    let night = to.checked_sub(from)?;
    let twice = night.checked_mul(num)?.checked_mul(2)?.checked_add(den)?;
    from.checked_add(twice.checked_div(den.checked_mul(2)?)?)
}

proptest! {
    // 2000 cases by default; `PROPTEST_CASES=1000000` (`bun run deep:properties`) overrides it.
    #![proptest_config(ProptestConfig { cases: cases(), ..ProptestConfig::default() })]

    // ---------- branded types accept exactly their ranges ----------

    #[test]
    fn latitude_validation_matches_range(value in proptest::num::f64::ANY) {
        let valid = value.is_finite() && (-90.0_f64..=90.0_f64).contains(&value);
        prop_assert_eq!(Latitude::new(value).is_ok(), valid);
        if !value.is_finite() {
            prop_assert_eq!(Latitude::new(value), Err(InvalidInput::NotFinite));
        }
    }

    #[test]
    fn longitude_validation_matches_range(value in proptest::num::f64::ANY) {
        prop_assert_eq!(
            Longitude::new(value).is_ok(),
            value.is_finite() && (-180.0_f64..=180.0_f64).contains(&value)
        );
    }

    #[test]
    fn twilight_angle_validation_matches_range(value in proptest::num::f64::ANY) {
        prop_assert_eq!(
            TwilightAngle::new(value).is_ok(),
            value.is_finite() && (0.5_f64..=30.0_f64).contains(&value)
        );
    }

    #[test]
    fn dates_round_trip(date in any_date()) {
        let rebuilt = Date::new(date.year(), date.month(), date.day());
        prop_assert_eq!(rebuilt, Ok(date));
        prop_assert_eq!(Date::from_days_since_unix_epoch(date.days_since_unix_epoch()), Ok(date));
    }

    #[test]
    fn next_date_is_one_day_later(date in any_date()) {
        if let Ok(next) = date.next() {
            prop_assert_eq!(
                next.days_since_unix_epoch(),
                date.days_since_unix_epoch().saturating_add(1)
            );
            prop_assert!(next > date);
        }
    }

    // ---------- totality and the output invariant ----------

    /// Anywhere on Earth, any date, any parameters: never panics, and any `Ok` is ordered.
    #[test]
    fn total_and_ordered_everywhere(date in any_date(), coords in coordinates(90.0), params in parameters()) {
        match compute(date, coords, params) {
            Ok(t) => {
                let order = [Prayer::Fajr, Prayer::Sunrise, Prayer::Dhuhr, Prayer::Asr, Prayer::Maghrib, Prayer::Isha];
                for pair in order.windows(2) {
                    prop_assert!(t.get(pair[0]) < t.get(pair[1]), "{} !< {}", pair[0], pair[1]);
                }
                // The night marks exist exactly when the next day has a Fajr, and split the
                // night from Maghrib to it at a half and two thirds.
                let next = date
                    .days_since_unix_epoch()
                    .checked_add(1)
                    .and_then(|days| Date::from_days_since_unix_epoch(days).ok())
                    .and_then(|next| compute(next, coords, params).ok());
                prop_assert_eq!(t.midnight().is_some(), next.is_some());
                prop_assert_eq!(t.last_third().is_some(), next.is_some());
                if let (Some(midnight), Some(last_third), Some(next)) = (t.midnight(), t.last_third(), next) {
                    prop_assert!(t.maghrib() < midnight && midnight < last_third && last_third < next.fajr());
                    let (from, to) = (t.maghrib().unix_minutes(), next.fajr().unix_minutes());
                    prop_assert_eq!(Some(midnight.unix_minutes()), fraction_of_night(from, to, 1, 2));
                    prop_assert_eq!(Some(last_third.unix_minutes()), fraction_of_night(from, to, 2, 3));
                }
                // Every time lies within the requested day ± one day.
                for prayer in Prayer::ALL {
                    prop_assert!((-1440..=2880).contains(&day_offset(date, &t, prayer)), "{prayer}");
                }
            }
            Err(ComputeError::PolarDayOrNight | ComputeError::Undefined(_) | ComputeError::OutOfOrder) => {
                // Twilight at angle a ends only if |lat| > 90 - 23.44 - a; the deepest built-in
                // angle is 20°, so without a fallback rule failures start at ~46.5°.
                let floor = if params.high_latitude == HighLatitudeRule::None { 46.5_f64 } else { 48.0_f64 };
                prop_assert!(coords.latitude.get().abs() > floor, "error at {:?}", coords.latitude);
            }
            Err(other) => prop_assert!(false, "unexpected error {other}"),
        }
    }

    /// Where people actually live (|lat| ≤ 55°), every method with a fallback always succeeds.
    #[test]
    fn always_succeeds_below_55_degrees(date in any_date(), coords in coordinates(55.0), method in any_method(), asr in any_asr(), rule in fallback_rule()) {
        let params = Parameters { asr, high_latitude: rule, ..method.parameters() };
        prop_assert!(compute(date, coords, params).is_ok());
    }

    /// Without a fallback rule nothing is ever marked as high-latitude-adjusted.
    #[test]
    fn no_rule_means_no_substitution(date in any_date(), coords in coordinates(90.0), method in any_method()) {
        let params = Parameters { high_latitude: HighLatitudeRule::None, ..method.parameters() };
        if let Ok(t) = compute(date, coords, params) {
            prop_assert!(!t.notes().fajr_high_latitude);
            prop_assert!(!t.notes().isha_high_latitude);
        }
    }

    /// Pure function: identical inputs give identical outputs.
    #[test]
    fn deterministic(date in any_date(), coords in coordinates(90.0), params in parameters()) {
        prop_assert_eq!(compute(date, coords, params), compute(date, coords, params));
    }

    // ---------- metamorphic relations ----------

    /// Adding k minutes to an adjustment moves exactly that prayer by exactly k minutes.
    #[test]
    fn adjustments_shift_exactly(date in any_date(), coords in coordinates(45.0), k in -10_i16..=10) {
        let base = Method::MuslimWorldLeague.parameters();
        let shifted = Parameters {
            adjustments: Adjustments { asr: MinuteAdjustment::new(k).expect("in range"), ..base.adjustments },
            ..base
        };
        let (a, b) = (compute(date, coords, base).expect("ok"), compute(date, coords, shifted).expect("ok"));
        prop_assert_eq!(minutes(&b, Prayer::Asr).saturating_sub(minutes(&a, Prayer::Asr)), i64::from(k));
        for prayer in [Prayer::Fajr, Prayer::Sunrise, Prayer::Dhuhr, Prayer::Maghrib, Prayer::Isha] {
            prop_assert_eq!(a.get(prayer), b.get(prayer), "{} moved", prayer);
        }
    }

    /// Hanafi Asr (twice the shadow) is always later than standard Asr.
    #[test]
    fn hanafi_asr_is_later(date in any_date(), coords in coordinates(55.0), method in any_method()) {
        let standard = compute(date, coords, Parameters { asr: AsrShadow::Standard, ..method.parameters() }).expect("ok");
        let hanafi = compute(date, coords, Parameters { asr: AsrShadow::Hanafi, ..method.parameters() }).expect("ok");
        prop_assert!(hanafi.asr() > standard.asr());
    }

    /// A deeper Fajr angle never gives a later Fajr.
    #[test]
    fn deeper_fajr_angle_is_never_later(date in any_date(), coords in coordinates(45.0), a in 12.0_f64..=19.0_f64, extra in 0.0_f64..=3.0_f64) {
        let with = |angle: f64| Parameters {
            fajr: TwilightAngle::new(angle).expect("in range"),
            high_latitude: HighLatitudeRule::None,
            ..Method::MuslimWorldLeague.parameters()
        };
        let shallow = compute(date, coords, with(a));
        let deep = compute(date, coords, with(a + extra));
        if let (Ok(shallow), Ok(deep)) = (shallow, deep) {
            prop_assert!(deep.fajr() <= shallow.fajr());
        }
    }

    /// Moving 15° east moves solar noon one hour earlier in UTC (within rounding + day drift).
    #[test]
    fn longitude_shifts_noon_by_four_minutes_per_degree(date in any_date(), lat in -50.0_f64..=50.0_f64, lng in -150.0_f64..=150.0_f64) {
        let params = Parameters { adjustments: Adjustments::NONE, ..Method::MuslimWorldLeague.parameters() };
        let west = compute(date, Coordinates::new(lat, lng).expect("ok"), params).expect("ok");
        let east = compute(date, Coordinates::new(lat, lng + 15.0).expect("ok"), params).expect("ok");
        let shift = minutes(&west, Prayer::Dhuhr).saturating_sub(minutes(&east, Prayer::Dhuhr));
        prop_assert!((59..=61).contains(&shift), "shift {shift}");
    }

    /// Dhuhr is within the equation-of-time bound (±16.5 min) of local mean noon.
    #[test]
    fn dhuhr_is_near_mean_noon(date in any_date(), coords in coordinates(60.0)) {
        let params = Parameters { adjustments: Adjustments::NONE, ..Method::MuslimWorldLeague.parameters() };
        if let Ok(t) = compute(date, coords, params) {
            let mean_noon = 720.0_f64 - coords.longitude.get() * 4.0_f64;
            let dhuhr = day_offset(date, &t, Prayer::Dhuhr);
            let dhuhr = f64::from(i32::try_from(dhuhr).expect("small"));
            prop_assert!((dhuhr - mean_noon).abs() <= 17.5_f64, "dhuhr {dhuhr}, mean noon {mean_noon}");
        }
    }

    /// The seasonal rule only ever shortens twilight: Fajr never earlier, Isha never later,
    /// than the plain angle wherever that angle is reached (below 55°, where no seventh-of-the-
    /// night override applies).
    #[test]
    fn season_adjusted_never_lengthens_twilight(date in any_date(), coords in coordinates(54.9), shafaq in any_shafaq()) {
        let plain = Parameters { high_latitude: HighLatitudeRule::None, ..Method::MoonsightingCommittee.parameters() };
        let seasonal = Parameters { high_latitude: HighLatitudeRule::SeasonAdjusted(shafaq), ..plain };
        let adjusted = compute(date, coords, seasonal).expect("total below 55°");
        if let Ok(plain) = compute(date, coords, plain) {
            prop_assert!(adjusted.fajr() >= plain.fajr());
            prop_assert!(adjusted.isha() <= plain.isha());
        }
    }

    /// White twilight outlasts the committee's general one, so its Isha is never earlier.
    #[test]
    fn white_shafaq_isha_is_never_earlier_than_general(date in any_date(), coords in coordinates(90.0)) {
        let with = |shafaq| Parameters {
            high_latitude: HighLatitudeRule::SeasonAdjusted(shafaq),
            ..Method::MoonsightingCommittee.parameters()
        };
        if let (Ok(general), Ok(white)) = (compute(date, coords, with(Shafaq::General)), compute(date, coords, with(Shafaq::White))) {
            prop_assert!(white.isha() >= general.isha());
            prop_assert_eq!(white.fajr(), general.fajr(), "shafaq only affects Isha");
        }
    }

    /// Interval-based Isha is exactly the interval after Maghrib.
    #[test]
    fn interval_isha_is_exact(date in any_date(), coords in coordinates(55.0)) {
        let params = Method::UmmAlQura.parameters();
        let t = compute(date, coords, params).expect("ok");
        let IshaRule::Interval { regular, ramadan } = params.isha else { unreachable!("interval method") };
        let expected = if t.notes().ramadan_isha { ramadan } else { regular };
        prop_assert_eq!(
            minutes(&t, Prayer::Isha).saturating_sub(minutes(&t, Prayer::Maghrib)),
            i64::from(expected.get())
        );
    }

    /// Announcing exactly the calculated dates is the same as announcing nothing.
    #[test]
    fn announcing_the_calendar_changes_nothing(from in any_date(), offset in 0_i32..=40_i32, coords in coordinates(55.0), method in any_method()) {
        let Some((first, last)) = calculated_ramadan_from(from) else { return Ok(()) };
        let Some(date) = first.checked_sub(5).and_then(|d| d.checked_add(offset)).and_then(day) else { return Ok(()) };
        let same = AnnouncedRamadan::new(day(first).expect("in range"), day(last).expect("in range"))
            .expect("the calendar is a valid announcement");
        let params = method.parameters();
        let announced = Parameters { ramadan: RamadanDates::Announced(same), ..params };
        prop_assert_eq!(compute(date, coords, announced), compute(date, coords, params));
    }

    /// An announcement only ever moves Isha, and only on the days it disputes.
    #[test]
    fn announcements_only_move_isha(from in any_date(), shift in -3_i32..=3_i32, length in 29_i32..=30_i32, offset in 0_i32..=40_i32, coords in coordinates(55.0)) {
        let Some(dates) = announced(from, shift, length) else { return Ok(()) };
        let Some((first, _)) = calculated_ramadan_from(from) else { return Ok(()) };
        let Some(date) = first.checked_sub(5).and_then(|d| d.checked_add(offset)).and_then(day) else { return Ok(()) };
        let params = Method::UmmAlQura.parameters();
        let calculated = compute(date, coords, params).expect("ok");
        let moved = compute(date, coords, Parameters { ramadan: dates, ..params }).expect("ok");
        prop_assert_eq!(moved.notes().ramadan_isha, dates.contains(date));
        for prayer in Prayer::ALL {
            if prayer != Prayer::Isha {
                prop_assert_eq!(moved.get(prayer), calculated.get(prayer), "{}", prayer);
            }
        }
        if dates.contains(date) == is_ramadan(date) {
            prop_assert_eq!(moved, calculated);
        }
    }
}
