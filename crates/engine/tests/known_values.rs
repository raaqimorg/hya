//! Example-based tests: hand-checked outcomes, exact messages and ids, the method tables, the
//! `Recommended` latitude switch, night fractions, the ordering check, and every error path.

#![expect(
    clippy::expect_used,
    reason = "test-only helpers; a failed precondition should abort the test loudly"
)]

use hya_engine::{
    Adjustments, AnnouncedRamadan, ComputeError, Coordinates, Date, DhuhrRule, Elevation,
    HighLatitudeRule, InvalidInput, IshaRule, Method, MinuteAdjustment, Parameters, Prayer,
    PrayerTimes, RamadanDates, Rounding, Shafaq, compute,
};

fn date(y: i16, m: u8, d: u8) -> Date {
    Date::new(y, m, d).expect("valid date")
}

fn at(lat: f64, lng: f64) -> Coordinates {
    Coordinates::new(lat, lng).expect("valid coordinates")
}

fn minutes(times: &PrayerTimes, prayer: Prayer) -> i64 {
    times.get(prayer).unix_minutes()
}

fn hhmm_utc(minutes: i64) -> String {
    let of_day = minutes.rem_euclid(1440);
    format!("{:02}:{:02}", of_day / 60, of_day % 60)
}

// ---------- hand-checked days and error paths ----------

#[test]
fn riyadh_today_umm_al_qura() {
    // Riyadh is UTC+3; local 04:40 / 05:58 / 11:38 / 14:51 / 17:16 / 18:46, exactly as the
    // official calculator at ummulqura.org.sa gives them. (adhan-js, which rounds to the
    // nearest minute, gives 04:39 / 05:58 / 11:37 / 14:51 / 17:15 / 18:45.)
    let times = compute(
        Date::new(2026, 10, 29).expect("valid"),
        Coordinates::new(24.7136, 46.6753).expect("valid"),
        Parameters::default(),
    )
    .expect("computable");
    let utc: Vec<String> = Prayer::ALL
        .iter()
        .map(|&p| hhmm_utc(times.get(p).unix_minutes()))
        .collect();
    assert_eq!(
        utc,
        ["01:40", "02:58", "08:38", "11:51", "14:16", "15:46"],
        "schedule"
    );
}

#[test]
fn ramadan_evening_in_makkah_uses_120_minutes() {
    // 2027-02-20 is 13 Ramadan 1448, so its evening is still Ramadan.
    let times = compute(
        Date::new(2027, 2, 20).expect("valid"),
        Coordinates::new(21.4225, 39.8262).expect("valid"),
        Method::UmmAlQura.parameters(),
    )
    .expect("computable");
    assert!(times.notes().ramadan_isha, "ramadan flag");
    assert_eq!(
        times.isha().unix_minutes() - times.maghrib().unix_minutes(),
        120,
        "gap"
    );
}

#[test]
fn eve_of_first_ramadan_already_uses_120_and_eve_of_eid_does_not() {
    let makkah = Coordinates::new(21.4225, 39.8262).expect("valid");
    let gap = |y, m, d| {
        let t = compute(
            Date::new(y, m, d).expect("valid"),
            makkah,
            Parameters::default(),
        )
        .expect("ok");
        t.isha().unix_minutes() - t.maghrib().unix_minutes()
    };
    // Ramadan 1447: 2026-02-18 ..= 2026-03-19; Eid al-Fitr 2026-03-20.
    assert_eq!(
        gap(2026, 2, 17),
        90,
        "eve of 1 Ramadan: not yet announced at Isha"
    );
    assert_eq!(gap(2026, 2, 18), 120, "1 Ramadan");
    assert_eq!(gap(2026, 3, 19), 120, "last day of Ramadan (eve of Eid)");
    assert_eq!(gap(2026, 3, 20), 90, "Eid al-Fitr");
}

#[test]
fn announced_dates_override_the_calendar() {
    let makkah = Coordinates::new(21.4225, 39.8262).expect("valid");
    let date = |m, d| Date::new(2026, m, d).expect("valid");
    // Suppose the crescent is sighted a day after the calculation: 1447 runs 02-19 ..= 03-20.
    let late = RamadanDates::Announced(
        AnnouncedRamadan::new(date(2, 19), date(3, 20)).expect("valid announcement"),
    );
    let gap = |m, d| {
        let params = Parameters {
            ramadan: late,
            ..Parameters::default()
        };
        let t = compute(date(m, d), makkah, params).expect("ok");
        t.isha().unix_minutes() - t.maghrib().unix_minutes()
    };
    assert_eq!(gap(2, 18), 90, "calculated 1 Ramadan, not yet begun");
    assert_eq!(gap(2, 19), 120, "announced 1 Ramadan");
    assert_eq!(gap(3, 20), 120, "announced last day");
    assert_eq!(gap(3, 21), 90, "Eid al-Fitr");
}

#[test]
fn polar_night_is_an_error_not_a_panic() {
    let longyearbyen = Coordinates::new(78.2232, 15.6267).expect("valid");
    let result = compute(
        Date::new(2026, 12, 21).expect("valid"),
        longyearbyen,
        Parameters::default(),
    );
    assert_eq!(result, Err(ComputeError::PolarDayOrNight), "polar night");
}

#[test]
fn midsummer_london_without_rule_has_no_isha() {
    let params = Parameters {
        high_latitude: HighLatitudeRule::None,
        ..Method::MuslimWorldLeague.parameters()
    };
    let result = compute(
        Date::new(2026, 6, 21).expect("valid"),
        Coordinates::new(51.5074, -0.1278).expect("valid"),
        params,
    );
    assert_eq!(
        result,
        Err(ComputeError::Undefined(Prayer::Fajr)),
        "no twilight end"
    );
}

#[test]
fn midsummer_london_with_rule_is_flagged() {
    let times = compute(
        Date::new(2026, 6, 21).expect("valid"),
        Coordinates::new(51.5074, -0.1278).expect("valid"),
        Method::MuslimWorldLeague.parameters(),
    )
    .expect("computable");
    assert!(
        times.notes().fajr_high_latitude && times.notes().isha_high_latitude,
        "flags"
    );
}

#[test]
fn elevation_brings_sunrise_earlier_and_sunset_later() {
    let date = Date::new(2026, 10, 29).expect("valid");
    let ground = Coordinates::new(21.4225, 39.8262).expect("valid");
    let tower = ground.with_elevation(Elevation::new(600.0).expect("valid"));
    let low = compute(date, ground, Parameters::default()).expect("ok");
    let high = compute(date, tower, Parameters::default()).expect("ok");
    assert!(
        high.sunrise() < low.sunrise(),
        "sunrise earlier from height"
    );
    assert!(high.maghrib() > low.maghrib(), "sunset later from height");
}

#[test]
fn every_method_works_in_every_fixture_city_today() {
    let date = Date::new(2026, 10, 29).expect("valid");
    for (lat, lng) in [
        (24.71_f64, 46.67_f64),
        (30.04_f64, 31.24_f64),
        (-6.21_f64, 106.85_f64),
        (43.65_f64, -79.38_f64),
        (-33.92_f64, 18.42_f64),
    ] {
        for method in Method::ALL {
            let coords = Coordinates::new(lat, lng).expect("valid");
            assert!(
                compute(date, coords, method.parameters()).is_ok(),
                "{method} at {lat},{lng}"
            );
        }
    }
}

#[test]
fn tehran_falls_back_to_sunset_when_its_maghrib_angle_passes_isha() {
    // Oslo, 1 June: the seventh-of-the-night Isha comes before the sun is 4.5° down. adhan-js
    // gives Maghrib at sunset, 20:24, and Isha 21:13 (UTC). In January the angle stands.
    let oslo = Coordinates::new(59.9139, 10.7522).expect("valid");
    let summer = compute(
        Date::new(2026, 6, 1).expect("valid"),
        oslo,
        Method::Tehran.parameters(),
    )
    .expect("computable");
    assert_eq!(
        hhmm_utc(summer.maghrib().unix_minutes()),
        "20:24",
        "maghrib"
    );
    assert_eq!(hhmm_utc(summer.isha().unix_minutes()), "21:13", "isha");
    assert!(summer.notes().maghrib_at_sunset, "noted");
    let winter = compute(
        Date::new(2026, 1, 15).expect("valid"),
        oslo,
        Method::Tehran.parameters(),
    )
    .expect("computable");
    assert_eq!(
        hhmm_utc(winter.maghrib().unix_minutes()),
        "15:27",
        "maghrib"
    );
    assert!(!winter.notes().maghrib_at_sunset, "angle used");
}

// ---------- user-visible strings ----------

#[test]
fn prayer_ids_and_display() {
    let ids: Vec<&str> = Prayer::ALL.iter().map(|p| p.id()).collect();
    assert_eq!(
        ids,
        ["fajr", "sunrise", "dhuhr", "asr", "maghrib", "isha"],
        "ids"
    );
    assert_eq!(Prayer::Isha.to_string(), "isha", "display");
}

#[test]
fn error_messages() {
    assert_eq!(
        ComputeError::PolarDayOrNight.to_string(),
        "the sun does not rise or set on this day",
        "polar"
    );
    assert_eq!(
        ComputeError::Undefined(Prayer::Fajr).to_string(),
        "fajr does not occur on this day",
        "undefined"
    );
    assert_eq!(
        ComputeError::OutOfOrder.to_string(),
        "computed times are not in daily order",
        "order"
    );
    assert_eq!(
        ComputeError::Unrepresentable.to_string(),
        "computed time is out of range",
        "range"
    );
    assert_eq!(
        InvalidInput::LatitudeOutOfRange.to_string(),
        "latitude must be within -90..=90 degrees",
        "lat"
    );
    assert_eq!(
        InvalidInput::DateOutOfRange.to_string(),
        "year must be within 1900..=2100",
        "year"
    );
    let unknown = "nope".parse::<Method>().expect_err("unknown");
    assert_eq!(
        unknown.to_string(),
        "unknown calculation method",
        "unknown method"
    );
}

#[test]
fn date_and_method_display() {
    assert_eq!(date(2026, 3, 5).to_string(), "2026-03-05", "zero padded");
    assert_eq!(Method::NorthAmerica.to_string(), "north_america", "method");
}

// ---------- instants ----------

#[test]
fn instant_unit_conversions() {
    let times = compute(
        date(2026, 10, 29),
        at(24.7136, 46.6753),
        Parameters::default(),
    )
    .expect("ok");
    let fajr = times.fajr();
    assert_eq!(fajr.unix_seconds(), fajr.unix_minutes() * 60, "seconds");
    assert_eq!(fajr.unix_millis(), fajr.unix_minutes() * 60_000, "millis");
}

// ---------- method tables (values from adhan-js 4.4.4) ----------

fn adjustments(sunrise: i16, dhuhr: i16, asr: i16, maghrib: i16) -> Adjustments {
    let m = |v| MinuteAdjustment::new(v).expect("in range");
    Adjustments {
        sunrise: m(sunrise),
        dhuhr: m(dhuhr),
        asr: m(asr),
        maghrib: m(maghrib),
        ..Adjustments::NONE
    }
}

#[test]
fn method_adjustments_and_rounding() {
    for method in Method::ALL {
        let params = method.parameters();
        let expected = match method {
            Method::MuslimWorldLeague
            | Method::Egyptian
            | Method::Karachi
            | Method::NorthAmerica
            | Method::Singapore => adjustments(0, 1, 0, 0),
            Method::Dubai => adjustments(-3, 3, 3, 3),
            Method::Turkey => adjustments(-7, 5, 4, 7),
            Method::MoonsightingCommittee => adjustments(0, 5, 0, 3),
            Method::UmmAlQura | Method::Malaysia | Method::MalaysiaKelantan => {
                adjustments(-1, 0, 0, 0)
            }
            Method::Jordan => adjustments(-7, 0, 0, 6),
            Method::Kuwait | Method::Qatar | Method::Tehran => Adjustments::NONE,
        };
        assert_eq!(params.adjustments, expected, "{method} adjustments");
        let malaysian = matches!(method, Method::Malaysia | Method::MalaysiaKelantan);
        let rounding = if matches!(method, Method::Singapore | Method::UmmAlQura) || malaysian {
            Rounding::Up
        } else {
            Rounding::Nearest
        };
        assert_eq!(params.rounding, rounding, "{method} rounding");
        let dhuhr = if malaysian {
            DhuhrRule::AfterTheLimb
        } else {
            DhuhrRule::Transit
        };
        assert_eq!(params.dhuhr, dhuhr, "{method} dhuhr rule");
        assert_eq!(Parameters::from(method), params, "{method} From");
    }
}

// ---------- high-latitude rules ----------

fn fajr_with(rule: HighLatitudeRule, coords: Coordinates, day: Date) -> i64 {
    let params = Parameters {
        high_latitude: rule,
        ..Method::MuslimWorldLeague.parameters()
    };
    minutes(
        &compute(day, coords, params).expect("computable"),
        Prayer::Fajr,
    )
}

#[test]
fn recommended_switches_to_seventh_strictly_above_48_degrees() {
    let june = date(2026, 6, 21);
    let december = date(2026, 12, 21);
    let cases = [
        (at(48.0, 2.0), june, HighLatitudeRule::MiddleOfTheNight),
        (at(47.9, 2.0), june, HighLatitudeRule::MiddleOfTheNight),
        (at(50.0, 2.0), june, HighLatitudeRule::SeventhOfTheNight),
        // Southern summer: the switch uses |latitude|.
        (
            at(-50.0, -70.0),
            december,
            HighLatitudeRule::SeventhOfTheNight,
        ),
    ];
    for (coords, day, equivalent) in cases {
        let lat = coords.latitude.get();
        let recommended = fajr_with(HighLatitudeRule::Recommended, coords, day);
        assert_eq!(
            recommended,
            fajr_with(equivalent, coords, day),
            "{lat}: like {equivalent:?}"
        );
    }
    // At 48° itself the two rules really differ, so the boundary above is meaningful.
    let edge = at(48.0, 2.0);
    assert_ne!(
        fajr_with(HighLatitudeRule::MiddleOfTheNight, edge, june),
        fajr_with(HighLatitudeRule::SeventhOfTheNight, edge, june),
        "rules differ at the boundary"
    );
}

#[test]
fn night_fractions_and_night_marks() {
    let oslo = at(59.9139, 10.7522);
    let june = date(2026, 6, 21);
    for (rule, fraction) in [
        (HighLatitudeRule::MiddleOfTheNight, 0.5_f64),
        (HighLatitudeRule::SeventhOfTheNight, 1.0_f64 / 7.0_f64),
        (HighLatitudeRule::TwilightAngle, 18.0_f64 / 60.0_f64),
    ] {
        let params = Parameters {
            high_latitude: rule,
            ..Method::MuslimWorldLeague.parameters()
        };
        let t = compute(june, oslo, params).expect("computable");
        let night = minutes(&t, Prayer::Sunrise) + 1440 - minutes(&t, Prayer::Maghrib);
        let gap = minutes(&t, Prayer::Sunrise) - minutes(&t, Prayer::Fajr);
        let expected = f64::from(i32::try_from(night).expect("small")) * fraction;
        assert!(
            (f64::from(i32::try_from(gap).expect("small")) - expected).abs() <= 2.0_f64,
            "{rule:?}: {gap} vs {expected}"
        );
    }
}

fn next_day(day: Date) -> Date {
    day.days_since_unix_epoch()
        .checked_add(1)
        .and_then(|days| Date::from_days_since_unix_epoch(days).ok())
        .expect("in range")
}

#[test]
fn the_night_runs_from_maghrib_to_the_next_fajr() {
    // Midnight is half of it and the last third begins at two thirds, each rounded to the
    // nearest minute, a half minute up, as adhan-js's SunnahTimes does.
    let riyadh = at(24.7136, 46.6753);
    for day in [date(2026, 1, 1), date(2026, 6, 21), date(2026, 10, 29)] {
        let t = compute(day, riyadh, Parameters::default()).expect("ok");
        let next = compute(next_day(day), riyadh, Parameters::default()).expect("ok");
        let maghrib = minutes(&t, Prayer::Maghrib);
        let night = minutes(&next, Prayer::Fajr) - maghrib;
        let midnight = t.midnight().expect("a night").unix_minutes();
        let last_third = t.last_third().expect("a night").unix_minutes();
        assert_eq!(midnight, maghrib + (night + 1) / 2, "{day} midnight");
        assert_eq!(
            last_third,
            maghrib + (4 * night + 3) / 6,
            "{day} last third"
        );
    }

    // Hand-checked, Riyadh on 29 October 2026 (UTC+3): Maghrib 17:16, the next Fajr 04:40,
    // a night of 684 minutes. Midnight is ⌊685 / 2⌋ = 342 minutes in, the last third
    // ⌊2739 / 6⌋ = 456.
    let t = compute(date(2026, 10, 29), riyadh, Parameters::default()).expect("ok");
    let local = |minutes: i64| hhmm_utc(minutes + 180);
    assert_eq!(local(minutes(&t, Prayer::Maghrib)), "17:16");
    let next = compute(date(2026, 10, 30), riyadh, Parameters::default()).expect("ok");
    assert_eq!(local(minutes(&next, Prayer::Fajr)), "04:40");
    assert_eq!(
        local(t.midnight().expect("a night").unix_minutes()),
        "22:58"
    );
    assert_eq!(
        local(t.last_third().expect("a night").unix_minutes()),
        "00:52"
    );
}

#[test]
fn without_a_next_fajr_there_are_no_night_marks() {
    // The last representable day: the next day is out of range.
    let last = compute(
        date(2100, 12, 31),
        at(24.7136, 46.6753),
        Parameters::default(),
    )
    .expect("the six times still compute");
    assert_eq!((last.midnight(), last.last_third()), (None, None));

    // With no high-latitude rule, Fajr disappears in summer at 52°N. On the last day it occurs,
    // the next day has none.
    let berlin = at(52.52, 13.405);
    let params = Parameters {
        high_latitude: HighLatitudeRule::None,
        ..Method::MuslimWorldLeague.parameters()
    };
    let mut day = date(2026, 4, 1);
    loop {
        if let Ok(t) = compute(day, berlin, params)
            && compute(next_day(day), berlin, params).is_err()
        {
            assert_eq!((t.midnight(), t.last_third()), (None, None), "{day}");
            break;
        }
        day = next_day(day);
        assert!(day < date(2026, 7, 1), "no edge found");
    }
}

// ---------- the ordering guard ----------

/// Shifts `earlier` later and `later` earlier until they coincide; ties must be rejected.
#[test]
fn coinciding_times_are_rejected() {
    let riyadh = at(24.7136, 46.6753);
    let oslo_winter = (at(59.9139, 10.7522), date(2026, 12, 21));
    let pairs = [
        (Prayer::Fajr, Prayer::Sunrise, riyadh, date(2026, 10, 29)),
        (Prayer::Sunrise, Prayer::Dhuhr, oslo_winter.0, oslo_winter.1),
        (Prayer::Dhuhr, Prayer::Asr, oslo_winter.0, oslo_winter.1),
        (Prayer::Asr, Prayer::Maghrib, riyadh, date(2026, 10, 29)),
        (Prayer::Maghrib, Prayer::Isha, riyadh, date(2026, 10, 29)),
    ];
    for (earlier, later, coords, day) in pairs {
        let base = Method::MuslimWorldLeague.parameters();
        let t = compute(day, coords, base).expect("computable");
        let gap = minutes(&t, later) - minutes(&t, earlier);
        let forward = i16::try_from(gap.min(120)).expect("small");
        let backward = i16::try_from(gap - i64::from(forward)).expect("small");
        assert!(
            backward <= 120,
            "{earlier}->{later}: gap {gap} too wide to close"
        );
        let mut adjust = base.adjustments;
        let set = |a: &mut Adjustments, p: Prayer, delta: i16| {
            let slot = match p {
                Prayer::Fajr => &mut a.fajr,
                Prayer::Sunrise => &mut a.sunrise,
                Prayer::Dhuhr => &mut a.dhuhr,
                Prayer::Asr => &mut a.asr,
                Prayer::Maghrib => &mut a.maghrib,
                Prayer::Isha => &mut a.isha,
            };
            *slot = MinuteAdjustment::new(slot.get() + delta).expect("in range");
        };
        set(&mut adjust, earlier, forward);
        set(&mut adjust, later, -backward);
        let tied = compute(
            day,
            coords,
            Parameters {
                adjustments: adjust,
                ..base
            },
        );
        assert_eq!(tied, Err(ComputeError::OutOfOrder), "{earlier} == {later}");
    }
}

#[test]
fn a_night_squeezed_out_by_adjustments_is_rejected() {
    // Every prayer is still in order, but Maghrib pushed late and Fajr pulled early leave no
    // night between them on a short summer night.
    let trondheim = at(63.4305, 10.3951);
    let day = date(2026, 6, 21);
    let base = Method::MuslimWorldLeague.parameters();
    let t = compute(day, trondheim, base).expect("computable");
    let next = compute(next_day(day), trondheim, base).expect("computable");
    let night = minutes(&next, Prayer::Fajr) - minutes(&t, Prayer::Maghrib);
    assert!(night < 240, "night too long for the test: {night} min");
    let pushed = Parameters {
        adjustments: Adjustments {
            fajr: MinuteAdjustment::new(-120).expect("in range"),
            maghrib: MinuteAdjustment::new(120).expect("in range"),
            isha: MinuteAdjustment::new(120).expect("in range"),
            ..base.adjustments
        },
        ..base
    };
    assert_eq!(
        compute(day, trondheim, pushed),
        Err(ComputeError::OutOfOrder)
    );
}

// ---------- Moonsighting Committee ----------

#[test]
fn moonsighting_committee_preset() {
    let params = Method::MoonsightingCommittee.parameters();
    assert_eq!(params.fajr.get(), 18.0_f64, "fajr angle");
    let IshaRule::Angle(isha) = params.isha else {
        panic!("angle-based isha")
    };
    assert_eq!(isha.get(), 18.0_f64, "isha angle");
    assert_eq!(
        params.high_latitude,
        HighLatitudeRule::SeasonAdjusted(Shafaq::General),
        "rule"
    );
    assert_eq!(Shafaq::default(), Shafaq::General, "default shafaq");
    assert_eq!(
        "moonsighting_committee".parse::<Method>(),
        Ok(Method::MoonsightingCommittee),
        "id"
    );
}

/// Minutes of morning (sunrise − Fajr) and evening (Isha − sunset) twilight, the length of the
/// night, and the high-latitude notes, with no minute adjustments in the way.
fn twilights(shafaq: Shafaq, coords: Coordinates, day: Date) -> (i64, i64, i64, bool, bool) {
    let params = Parameters {
        high_latitude: HighLatitudeRule::SeasonAdjusted(shafaq),
        adjustments: Adjustments::NONE,
        ..Method::MoonsightingCommittee.parameters()
    };
    let t = compute(day, coords, params).expect("computable");
    let (sunrise, sunset) = (minutes(&t, Prayer::Sunrise), minutes(&t, Prayer::Maghrib));
    (
        sunrise.saturating_sub(minutes(&t, Prayer::Fajr)),
        minutes(&t, Prayer::Isha).saturating_sub(sunset),
        sunrise.saturating_add(1440).saturating_sub(sunset),
        t.notes().fajr_high_latitude,
        t.notes().isha_high_latitude,
    )
}

#[test]
fn season_adjusted_shortens_summer_twilight_at_mid_latitudes() {
    // Chicago, solstice: 18° twilight lasts over two hours, the committee's limit far less.
    let (morning, evening, _, fajr_flag, isha_flag) =
        twilights(Shafaq::General, at(41.8781, -87.6298), date(2026, 6, 21));
    // 75 + 48.1 × 41.8781 / 55 and 75 + 6.14 × 41.8781 / 55, at 182 days from the solstice.
    assert!((111..=112).contains(&morning), "morning {morning}");
    assert!((79..=80).contains(&evening), "evening {evening}");
    assert!(fajr_flag && isha_flag, "both set by the rule");

    // Riyadh, midwinter: 18° twilight is already shorter than the limit, so it stands.
    let riyadh = at(24.7136, 46.6753);
    let december = date(2026, 12, 21);
    let (_, _, _, fajr_set, isha_set) = twilights(Shafaq::General, riyadh, december);
    assert!(!fajr_set && !isha_set, "astronomical twilight kept");
    let plain = Parameters {
        high_latitude: HighLatitudeRule::None,
        adjustments: Adjustments::NONE,
        ..Method::MoonsightingCommittee.parameters()
    };
    let seasonal = Parameters {
        high_latitude: HighLatitudeRule::SeasonAdjusted(Shafaq::General),
        ..plain
    };
    assert_eq!(
        compute(december, riyadh, plain),
        compute(december, riyadh, seasonal),
        "identical when the limit is not reached"
    );
}

#[test]
fn season_adjusted_uses_a_seventh_of_the_night_from_55_degrees() {
    for (lat, summer) in [
        (55.0_f64, date(2026, 6, 21)),
        (-55.0_f64, date(2026, 12, 21)),
    ] {
        let (morning, evening, night, fajr_flag, isha_flag) =
            twilights(Shafaq::General, at(lat, 0.0), summer);
        assert!(
            (morning - night / 7).abs() <= 1,
            "{lat}: morning {morning}, night {night}"
        );
        assert!(
            (evening - night / 7).abs() <= 1,
            "{lat}: evening {evening}, night {night}"
        );
        assert!(fajr_flag && isha_flag, "{lat}: flagged");

        // Just below, 18° twilight never ends, so the seasonal limit applies instead: far longer.
        let below = if lat > 0.0_f64 { 54.9_f64 } else { -54.9_f64 };
        let (seasonal, _, short_night, _, _) = twilights(Shafaq::General, at(below, 0.0), summer);
        assert!(
            seasonal > short_night / 7 + 30,
            "{below}: morning {seasonal}, night {short_night}"
        );
    }
}

#[test]
fn shafaq_choices_end_isha_at_different_glows() {
    let london = at(51.5074, -0.1278);
    // Winter: red ends earliest; general and white share the solstice value.
    let december = date(2026, 12, 21);
    let isha = |shafaq| twilights(shafaq, london, december).1;
    // 62 + 17.4 × 51.5074 / 55 and 75 + 25.6 × 51.5074 / 55.
    assert!(
        (78..=79).contains(&isha(Shafaq::Red)),
        "red {}",
        isha(Shafaq::Red)
    );
    assert!(
        (98..=99).contains(&isha(Shafaq::General)),
        "general {}",
        isha(Shafaq::General)
    );
    assert_eq!(
        isha(Shafaq::White),
        isha(Shafaq::General),
        "white at the solstice"
    );
    // Summer: white lasts far longer than general.
    let june = date(2026, 6, 21);
    let summer_isha = |shafaq| twilights(shafaq, london, june).1;
    // Day 182 of the cycle, one before the peak (75 + 6.14 × 51.5074 / 55 for general,
    // 75 + 81.84 × 51.5074 / 55 for white) on a 46-day slope: 80.7 and 150.7 minutes.
    assert!(
        (80..=81).contains(&summer_isha(Shafaq::General)),
        "general {}",
        summer_isha(Shafaq::General)
    );
    assert!(
        (150..=151).contains(&summer_isha(Shafaq::White)),
        "white {}",
        summer_isha(Shafaq::White)
    );
}
