//! Presets against the authorities' own published timetables: JAKIM's e-solat (Malaysia), the
//! Ministry of Awqaf (Jordan), the official Umm al-Qura calculator (Saudi Arabia) and MUIS
//! (Singapore).
//!
//! The fixture is fetched once by `bun scripts/fetch-official.ts` and committed; nothing here
//! touches the network. Two checks:
//!
//! - Wilayah Persekutuan publishes its reference points, so every time is compared there, for
//!   the whole year.
//! - Other zones' reference points are not published, but Isha − Maghrib barely depends on
//!   where in Malaysia you stand (it cancels longitude, and 1–7°N changes it by seconds). That
//!   interval pins the Isha angle in every zone, and is how Kelantan's 17° shows up.

#![expect(
    clippy::expect_used,
    reason = "test-only helpers; a failed precondition should abort the test loudly"
)]

extern crate alloc;

use alloc::collections::BTreeMap;
use hya_engine::{Coordinates, Date, Method, Prayer, PrayerTimes, compute};
use serde::Deserialize;

#[derive(Deserialize)]
struct Fixture {
    zones: BTreeMap<String, BTreeMap<String, [String; 6]>>,
}

/// Row order in the fixture.
const COLUMNS: [Prayer; 6] = [
    Prayer::Fajr,
    Prayer::Sunrise,
    Prayer::Dhuhr,
    Prayer::Asr,
    Prayer::Maghrib,
    Prayer::Isha,
];

/// Highland zones are computed for their elevation, which moves sunset and so the interval.
const HIGHLANDS: [&str; 4] = ["PHG05", "PHG06", "PRK07", "SBH06"];

fn fixture() -> Fixture {
    serde_json::from_str(include_str!("../../../fixtures/official/jakim-2026.json"))
        .expect("valid fixture")
}

fn official_minutes(hhmm: &str) -> i64 {
    let (hours, minutes) = hhmm.split_once(':').expect("hh:mm");
    let hours: i64 = hours.parse().expect("hours");
    let minutes: i64 = minutes.parse().expect("minutes");
    hours
        .checked_mul(60)
        .and_then(|h| h.checked_add(minutes))
        .expect("small")
}

fn date(key: &str) -> Date {
    let mut parts = key.split('-');
    let mut next = || parts.next().expect("y-m-d");
    let year = next().parse().expect("year");
    let month = next().parse().expect("month");
    let day = next().parse().expect("day");
    Date::new(year, month, day).expect("valid date")
}

/// Minutes after local midnight in Malaysia (UTC+8).
fn local_minutes(times: &PrayerTimes, prayer: Prayer, day: Date) -> i64 {
    i64::from(day.days_since_unix_epoch())
        .checked_mul(1440)
        .and_then(|midnight_utc| times.get(prayer).unix_minutes().checked_sub(midnight_utc))
        .and_then(|utc| utc.checked_add(480))
        .expect("small")
}

fn degrees(d: f64, m: f64, s: f64) -> f64 {
    d + m / 60.0 + s / 3600.0
}

#[test]
fn federal_territories_match_at_their_published_reference_points() {
    // Almanak Falak Syarie Malaysia 2021 (JUPEM), via the Federal Territories mufti's office.
    // Sunrise has its own reference point in each zone.
    let zones = [
        (
            "WLY01",
            (degrees(3.0, 44.0, 0.0), degrees(101.0, 23.0, 0.0)),
            (degrees(3.0, 11.0, 24.0), degrees(101.0, 45.0, 30.0)),
        ),
        (
            "WLY02",
            (degrees(5.0, 14.0, 53.0), degrees(115.0, 8.0, 23.0)),
            (degrees(5.0, 17.0, 23.0), degrees(115.0, 20.0, 34.0)),
        ),
    ];
    let fixture = fixture();
    for (zone, prayers_at, sunrise_at) in zones {
        let days = fixture.zones.get(zone).expect("zone in fixture");
        assert_eq!(days.len(), 365, "{zone} full year");
        let prayers_at = Coordinates::new(prayers_at.0, prayers_at.1).expect("valid");
        let sunrise_at = Coordinates::new(sunrise_at.0, sunrise_at.1).expect("valid");
        let mut exact = [0_usize; 6];
        for (key, row) in days {
            let day = date(key);
            let main = compute(day, prayers_at, Method::Malaysia.parameters()).expect("ok");
            let sunrise = compute(day, sunrise_at, Method::Malaysia.parameters()).expect("ok");
            for ((prayer, official), hits) in COLUMNS.iter().zip(row).zip(&mut exact) {
                let times = if *prayer == Prayer::Sunrise {
                    &sunrise
                } else {
                    &main
                };
                let error = local_minutes(times, *prayer, day) - official_minutes(official);
                assert!(
                    error.abs() <= 2,
                    "{zone} {key} {prayer}: off by {error} min"
                );
                if error == 0 {
                    *hits += 1;
                }
            }
        }
        for (prayer, hits) in COLUMNS.iter().zip(exact) {
            // Measured: 72–98% of days to the minute (Fajr in WLY01 is the lowest). The rest
            // differ by the last rounding step; JAKIM's own solar model is not published.
            assert!(
                hits * 100 >= days.len() * 70,
                "{zone} {prayer}: {hits}/365 exact"
            );
        }
    }
}

fn isha_after_maghrib(method: Method, day: Date) -> i64 {
    // Mid-peninsula; the interval moves by seconds across Malaysia's latitudes.
    let anywhere = Coordinates::new(4.0, 102.0).expect("valid");
    let times = compute(day, anywhere, method.parameters()).expect("ok");
    local_minutes(&times, Prayer::Isha, day)
        .checked_sub(local_minutes(&times, Prayer::Maghrib, day))
        .expect("small")
}

#[test]
fn every_lowland_zone_has_the_isha_angle_of_its_preset() {
    for (zone, days) in fixture().zones {
        if HIGHLANDS.contains(&zone.as_str()) {
            continue;
        }
        let kelantan = zone.starts_with("KTN");
        let mut close = 0_usize;
        let sampled = days.len();
        for (key, row) in days {
            let day = date(&key);
            let [.., maghrib, isha] = &row;
            let official = official_minutes(isha) - official_minutes(maghrib);
            let error = |method| isha_after_maghrib(method, day) - official;
            let (own, other) = if kelantan {
                (Method::MalaysiaKelantan, Method::Malaysia)
            } else {
                (Method::Malaysia, Method::MalaysiaKelantan)
            };
            // Both ends are rounded up, so ±1 is the floor; latitude adds the odd second minute.
            assert!(
                error(own).abs() <= 2,
                "{zone} {key}: {own} off by {}",
                error(own)
            );
            if error(own).abs() <= 1 {
                close += 1;
            }
            // The other angle is a degree away, about four minutes: never a rounding slip.
            assert!(error(other).abs() >= 3, "{zone} {key}: {other} fits too");
        }
        assert!(
            close * 100 >= sampled * 90,
            "{zone}: {close}/{sampled} within a minute"
        );
    }
}

#[derive(Deserialize)]
struct Jordan {
    cities: BTreeMap<String, BTreeMap<String, [String; 6]>>,
}

/// Minutes after local midnight in Jordan (UTC+3).
fn jordan_minutes(times: &PrayerTimes, prayer: Prayer, day: Date) -> i64 {
    local_minutes(times, prayer, day)
        .checked_sub(300)
        .expect("small")
}

/// The ministry does not publish its reference points, so every check is an interval from
/// Dhuhr: that cancels longitude, leaving only latitude (city centres are close enough) and the
/// parameters under test.
#[test]
fn jordan_matches_the_ministrys_timetables_from_dhuhr() {
    let fixture: Jordan = serde_json::from_str(include_str!(
        "../../../fixtures/official/awqaf-jordan-2026.json"
    ))
    .expect("valid fixture");
    let centres = [
        ("amman", 31.9539_f64, 35.9106_f64),
        ("irbid", 32.5556_f64, 35.85_f64),
        ("aqaba", 29.5267_f64, 35.0078_f64),
    ];
    for (city, latitude, longitude) in centres {
        let days = fixture.cities.get(city).expect("city in fixture");
        assert!(days.len() >= 30, "{city}: {} days", days.len());
        let at = Coordinates::new(latitude, longitude).expect("valid");
        let deeper_fajr = hya_engine::Parameters {
            fajr: hya_engine::TwilightAngle::new(19.0).expect("valid"),
            ..Method::Jordan.parameters()
        };
        let shallower_isha = hya_engine::Parameters {
            isha: hya_engine::IshaRule::Angle(hya_engine::TwilightAngle::new(17.0).expect("valid")),
            ..Method::Jordan.parameters()
        };
        for (key, row) in days {
            let day = date(key);
            let ours = compute(day, at, Method::Jordan.parameters()).expect("ok");
            let deeper = compute(day, at, deeper_fajr).expect("ok");
            let shallower = compute(day, at, shallower_isha).expect("ok");
            let official = |prayer: Prayer| {
                let index = COLUMNS.iter().position(|p| *p == prayer).expect("column");
                official_minutes(row.get(index).expect("six times"))
            };
            let from_dhuhr = |times: &PrayerTimes, prayer| {
                jordan_minutes(times, prayer, day)
                    .checked_sub(jordan_minutes(times, Prayer::Dhuhr, day))
                    .expect("small")
            };
            let error = |times: &PrayerTimes, prayer| {
                from_dhuhr(times, prayer)
                    .checked_sub(
                        official(prayer)
                            .checked_sub(official(Prayer::Dhuhr))
                            .expect("small"),
                    )
                    .expect("small")
            };
            // Both ends are rounded, so ±1 is the floor; reference points add up to one more.
            for prayer in [Prayer::Fajr, Prayer::Asr, Prayer::Isha] {
                let e = error(&ours, prayer);
                assert!(e.abs() <= 2, "{city} {key} {prayer}: off by {e}");
            }
            // A degree either way is about five minutes: never a rounding slip.
            assert!(
                error(&shallower, Prayer::Isha) <= -3,
                "{city} {key}: Isha 17° fits too"
            );
            assert!(
                error(&deeper, Prayer::Fajr) <= -3,
                "{city} {key}: Fajr 19° fits too"
            );
            // Sunrise and Maghrib carry the ministry's elevation corrections, which the preset
            // takes from Amman. Elsewhere it is more cautious, never less: sunrise (the end of
            // Fajr) no later and Maghrib no earlier, by at most 5 minutes.
            let sunrise = error(&ours, Prayer::Sunrise);
            assert!(
                (-6..=1).contains(&sunrise),
                "{city} {key} sunrise: {sunrise}"
            );
            let maghrib = error(&ours, Prayer::Maghrib);
            assert!(
                (-1..=5).contains(&maghrib),
                "{city} {key} maghrib: {maghrib}"
            );
        }
    }
}

/// Minutes after local midnight at `offset` minutes east of UTC.
fn minutes_at(times: &PrayerTimes, prayer: Prayer, day: Date, offset: i64) -> i64 {
    local_minutes(times, prayer, day)
        .checked_sub(480)
        .and_then(|utc| utc.checked_add(offset))
        .expect("small")
}

/// Ours minus official, in minutes, for one row.
fn errors(times: &PrayerTimes, row: &[String; 6], day: Date, offset: i64) -> [i64; 6] {
    let mut out = [0_i64; 6];
    for ((slot, prayer), official) in out.iter_mut().zip(COLUMNS).zip(row) {
        *slot = minutes_at(times, prayer, day, offset)
            .checked_sub(official_minutes(official))
            .expect("small");
    }
    out
}

#[derive(Deserialize)]
struct UmmAlQuraCity {
    latitude: f64,
    longitude: f64,
    days: BTreeMap<String, [String; 6]>,
}

#[derive(Deserialize)]
struct UmmAlQura {
    cities: BTreeMap<String, UmmAlQuraCity>,
}

/// Saudi Arabia, UTC+3.
const AST: i64 = 180;

fn umm_al_qura() -> UmmAlQura {
    serde_json::from_str(include_str!(
        "../../../fixtures/official/umm-al-qura-2026.json"
    ))
    .expect("valid fixture")
}

/// Exact-minute matches and misses beyond a minute over a year, per prayer.
fn tally(
    city: &UmmAlQuraCity,
    parameters: hya_engine::Parameters,
) -> ([usize; 6], [usize; 6], i64) {
    let at = Coordinates::new(city.latitude, city.longitude).expect("valid");
    let mut exact = [0_usize; 6];
    let mut beyond_one = [0_usize; 6];
    let mut worst = 0_i64;
    for (key, row) in &city.days {
        let day = date(key);
        let times = compute(day, at, parameters).expect("ok");
        for ((e, hit), miss) in errors(&times, row, day, AST)
            .into_iter()
            .zip(&mut exact)
            .zip(&mut beyond_one)
        {
            if e == 0 {
                *hit = hit.checked_add(1).expect("small");
            }
            if e.abs() > 1 {
                *miss = miss.checked_add(1).expect("small");
            }
            worst = worst.max(e.abs());
        }
    }
    (exact, beyond_one, worst)
}

/// The authority's calculator works from exact coordinates, so this is a direct comparison: no
/// reference point, no interval tricks.
#[test]
fn umm_al_qura_matches_the_official_calculator() {
    for (name, city) in umm_al_qura().cities {
        assert_eq!(city.days.len(), 365, "{name}");
        let (exact, beyond_one, worst) = tally(&city, Method::UmmAlQura.parameters());
        assert!(worst <= 2, "{name}: off by {worst}");
        let total_beyond: usize = beyond_one.iter().sum();
        assert!(total_beyond <= 3, "{name}: {total_beyond} times beyond ±1");
        // Dhuhr to Isha follow the sun's position closely; Fajr's angle is where the two
        // calculations differ most (still within a minute).
        for (prayer, hits) in COLUMNS.into_iter().zip(exact).skip(2) {
            assert!(hits >= 320, "{name} {prayer}: {hits}/365 exact");
        }
        let all: usize = exact.iter().sum();
        assert!(all >= 1860, "{name}: {all}/2190 exact");
    }
}

/// Rounding up (sunrise down) is the authority's own convention: to the nearest minute, as
/// adhan-js rounds, only about half the times match.
#[test]
fn umm_al_qura_rounds_up_like_the_authority() {
    let nearest = hya_engine::Parameters {
        rounding: hya_engine::Rounding::Nearest,
        adjustments: hya_engine::Adjustments::NONE,
        ..Method::UmmAlQura.parameters()
    };
    for (name, city) in umm_al_qura().cities {
        let (exact, _, _) = tally(&city, nearest);
        let all: usize = exact.iter().sum();
        assert!(
            all <= 1200,
            "{name}: nearest-minute rounding fits too ({all}/2190)"
        );
    }
}

/// The official Isha is 120 minutes after Maghrib on exactly the calendar's Ramadan days.
#[test]
fn umm_al_qura_ramadan_isha_follows_the_calendar() {
    for (name, city) in umm_al_qura().cities {
        let mut ramadan_days = 0_u32;
        for (key, row) in &city.days {
            let [.., maghrib, isha] = row;
            let gap = official_minutes(isha)
                .checked_sub(official_minutes(maghrib))
                .expect("small");
            let ramadan = hya_engine::is_ramadan(date(key));
            assert_eq!(gap, if ramadan { 120 } else { 90 }, "{name} {key}");
            if ramadan {
                ramadan_days = ramadan_days.checked_add(1).expect("small");
            }
        }
        assert_eq!(ramadan_days, 30, "{name}: Ramadan 1447 has 30 days");
    }
}

#[derive(Deserialize)]
struct Muis {
    days: BTreeMap<String, [String; 6]>,
}

/// MUIS issues one timetable for the island and does not publish the point it computes for.
/// This is the point that fits best (on a 0.01° grid around the city): central Singapore.
const MUIS_POINT: (f64, f64) = (1.28, 103.82);

/// Singapore, UTC+8.
const SGT: i64 = 480;

#[test]
fn singapore_matches_muis() {
    let fixture: Muis =
        serde_json::from_str(include_str!("../../../fixtures/official/muis-2026.json"))
            .expect("valid fixture");
    assert_eq!(fixture.days.len(), 365);
    let at = Coordinates::new(MUIS_POINT.0, MUIS_POINT.1).expect("valid");
    let angle = |degrees| hya_engine::TwilightAngle::new(degrees).expect("valid");
    let shallower_fajr = hya_engine::Parameters {
        fajr: angle(19.0_f64),
        ..Method::Singapore.parameters()
    };
    let shallower_isha = hya_engine::Parameters {
        isha: hya_engine::IshaRule::Angle(angle(17.0_f64)),
        ..Method::Singapore.parameters()
    };
    let mut exact = 0_usize;
    for (key, row) in &fixture.days {
        let day = date(key);
        let ours = errors(
            &compute(day, at, Method::Singapore.parameters()).expect("ok"),
            row,
            day,
            SGT,
        );
        for (prayer, e) in COLUMNS.into_iter().zip(ours) {
            assert!(e.abs() <= 1, "{key} {prayer}: off by {e}");
            if e == 0 {
                exact = exact.checked_add(1).expect("small");
            }
        }
        // A degree either way is about four minutes this close to the equator.
        let [fajr, ..] = errors(
            &compute(day, at, shallower_fajr).expect("ok"),
            row,
            day,
            SGT,
        );
        assert!(fajr >= 3, "{key}: Fajr 19° fits too");
        let [.., isha] = errors(
            &compute(day, at, shallower_isha).expect("ok"),
            row,
            day,
            SGT,
        );
        assert!(isha <= -3, "{key}: Isha 17° fits too");
    }
    assert!(exact >= 1550, "{exact}/2190 exact");
}
