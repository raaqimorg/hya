//! Golden tests: agreement with adhan-js, an independent implementation using different
//! (higher-precision) astronomical algorithms, across 22 cities × 24 dates × their methods.
//! Regenerate with `bun scripts/generate-adhan-js.ts`.

use hya_engine::{Coordinates, Date, HighLatitudeRule, Method, Prayer, Shafaq, compute};
use serde::Deserialize;

#[derive(Deserialize)]
struct Fixture {
    cases: Vec<Case>,
}

#[derive(Deserialize)]
struct Case {
    city: String,
    latitude: f64,
    longitude: f64,
    method: String,
    #[serde(default)]
    shafaq: Option<String>,
    date: (i16, u8, u8),
    unix_minutes: Times,
}

#[derive(Deserialize)]
struct Times {
    fajr: i64,
    sunrise: i64,
    dhuhr: i64,
    asr: i64,
    maghrib: i64,
    isha: i64,
}

impl Times {
    const fn get(&self, prayer: Prayer) -> i64 {
        match prayer {
            Prayer::Fajr => self.fajr,
            Prayer::Sunrise => self.sunrise,
            Prayer::Dhuhr => self.dhuhr,
            Prayer::Asr => self.asr,
            Prayer::Maghrib => self.maghrib,
            Prayer::Isha => self.isha,
        }
    }
}

/// Both implementations round to the minute, so ±1 minute is the tightest honest tolerance.
const TOLERANCE_MINUTES: u64 = 1;
/// See `asr_reference.rs`: adhan-js, not this crate, is the one off there.
const ADHAN_HIGH_LATITUDE_ASR_TOLERANCE: u64 = 2;

#[test]
fn agrees_with_adhan_js_within_one_minute() {
    let fixture: Fixture = serde_json::from_str(include_str!("../../../fixtures/adhan-js.json"))
        .expect("valid fixture");
    assert_eq!(fixture.cases.len(), 528, "fixture size");

    let mut failures = Vec::new();
    let mut exact = 0_usize;
    for case in &fixture.cases {
        let coordinates = Coordinates::new(case.latitude, case.longitude).expect("valid coords");
        let (year, month, day) = case.date;
        let date = Date::new(year, month, day).expect("valid date");
        let method: Method = case.method.parse().expect("known method");
        let mut parameters = method.parameters();
        parameters.high_latitude = match (method, case.shafaq.as_deref()) {
            // The committee's own rule; adhan-js applies it regardless of its generic setting.
            (Method::MoonsightingCommittee, None | Some("general")) => {
                HighLatitudeRule::SeasonAdjusted(Shafaq::General)
            }
            (Method::MoonsightingCommittee, Some("red")) => {
                HighLatitudeRule::SeasonAdjusted(Shafaq::Red)
            }
            (Method::MoonsightingCommittee, Some("white")) => {
                HighLatitudeRule::SeasonAdjusted(Shafaq::White)
            }
            (_, None) => HighLatitudeRule::MiddleOfTheNight,
            (_, Some(other)) => panic!("unexpected shafaq {other}"),
        };
        let times = compute(date, coordinates, parameters).expect("computable");

        for prayer in Prayer::ALL {
            let ours = times.get(prayer).unix_minutes();
            let theirs = case.unix_minutes.get(prayer);
            let diff = ours.abs_diff(theirs);
            if diff == 0 {
                exact = exact.saturating_add(1);
            }
            // adhan-js's Asr drifts up to ~2 min above 50° in winter; `asr_reference.rs` holds
            // ours to the exact definition there instead.
            let tolerance = if prayer == Prayer::Asr && case.latitude.abs() > 50.0_f64 {
                ADHAN_HIGH_LATITUDE_ASR_TOLERANCE
            } else {
                TOLERANCE_MINUTES
            };
            if diff > tolerance {
                failures.push(format!(
                    "{} {date} {prayer}: ours {ours}, adhan {theirs}",
                    case.city
                ));
            }
        }
    }
    assert!(
        failures.is_empty(),
        "{} mismatches:\n{}",
        failures.len(),
        failures.join("\n")
    );
    // Guard against a tolerance that silently hides a systematic one-minute bias.
    assert!(
        exact * 10 >= fixture.cases.len() * 6 * 7,
        "only {exact} exact matches"
    );
}
