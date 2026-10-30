//! Asr against first principles, not against adhan-js.
//!
//! adhan-js takes the noon-shadow declination at 0h UT instead of at transit, which drifts up to
//! ~2 minutes above 50° in winter, so the golden test cannot hold Asr to ±1 minute there. This
//! fixture solves the literal definition with an independent solar model (NOAA, not the core's
//! USNO formula) by bisection. Regenerate with `bun scripts/generate-asr-reference.ts`.

use hya_engine::{Adjustments, AsrShadow, Coordinates, Date, Method, Parameters, compute};
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
    factor: u8,
    date: (i16, u8, u8),
    unix_minutes: f64,
}

#[test]
fn asr_is_within_rounding_of_the_exact_definition() {
    let fixture: Fixture =
        serde_json::from_str(include_str!("../../../fixtures/asr-reference.json"))
            .expect("valid fixture");
    assert_eq!(fixture.cases.len(), 384, "fixture size");

    let mut worst = 0.0_f64;
    for case in &fixture.cases {
        let (year, month, day) = case.date;
        let date = Date::new(year, month, day).expect("valid date");
        let coordinates = Coordinates::new(case.latitude, case.longitude).expect("valid coords");
        let parameters = Parameters {
            asr: if case.factor == 2 {
                AsrShadow::Hanafi
            } else {
                AsrShadow::Standard
            },
            adjustments: Adjustments::NONE,
            ..Method::MuslimWorldLeague.parameters()
        };
        let times = compute(date, coordinates, parameters).expect("computable");
        let ours =
            f64::from(i32::try_from(times.asr().unix_minutes() - 29_000_000).expect("small"));
        let error = (ours - (case.unix_minutes - 29_000_000.0_f64)).abs();
        assert!(
            error < 1.0_f64,
            "{} {date} ×{}: off by {error:.2} min",
            case.city,
            case.factor
        );
        worst = worst.max(error);
    }
    // Rounding to the minute alone costs up to 0.5; the model and solver add only hundredths.
    assert!(worst < 0.6_f64, "worst error {worst:.3} min");
}
