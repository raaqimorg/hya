//! WebAssembly bindings for `hya-engine`.
//!
//! The boundary is deliberately narrow: plain numbers and short identifier strings in, a flat
//! `Float64Array` out. Validation and branding happen on both sides; the TypeScript wrapper in
//! `packages/js` turns the array back into a typed object.

use hya_engine::{
    AnnouncedRamadan, AsrShadow, Coordinates, Date, Elevation, HighLatitudeRule, Instant, Method,
    Parameters, Prayer, RamadanDates, Shafaq,
};
use wasm_bindgen::prelude::wasm_bindgen;

/// Layout of the array returned by [`compute`]: six prayer times, then midnight and last third
/// (Unix milliseconds; `NaN` when the next day has no Fajr), then four 0/1 flags.
pub const LAYOUT: [&str; 12] = [
    "fajr",
    "sunrise",
    "dhuhr",
    "asr",
    "maghrib",
    "isha",
    "midnight",
    "lastThird",
    "fajrHighLatitude",
    "ishaHighLatitude",
    "ramadanIsha",
    "maghribAtSunset",
];

fn high_latitude_rule(id: &str) -> Option<HighLatitudeRule> {
    match id {
        "recommended" => Some(HighLatitudeRule::Recommended),
        "middle_of_the_night" => Some(HighLatitudeRule::MiddleOfTheNight),
        "seventh_of_the_night" => Some(HighLatitudeRule::SeventhOfTheNight),
        "twilight_angle" => Some(HighLatitudeRule::TwilightAngle),
        "none" => Some(HighLatitudeRule::None),
        "season_adjusted" => Some(HighLatitudeRule::SeasonAdjusted(Shafaq::General)),
        _ => None,
    }
}

fn shafaq(id: &str) -> Option<Shafaq> {
    match id {
        "general" => Some(Shafaq::General),
        "red" => Some(Shafaq::Red),
        "white" => Some(Shafaq::White),
        _ => None,
    }
}

/// `rule` and `shafaq` may be empty for "the method's own". A shafaq only means something to the
/// season-adjusted rule, so pairing it with any other is rejected rather than ignored.
fn resolve_high_latitude(
    method: Method,
    rule: &str,
    shafaq_id: &str,
) -> Result<HighLatitudeRule, String> {
    let rule = if rule.is_empty() {
        method.parameters().high_latitude
    } else {
        high_latitude_rule(rule)
            .ok_or_else(|| format!("invalid_input: high latitude rule {rule}"))?
    };
    if shafaq_id.is_empty() {
        return Ok(rule);
    }
    let chosen = shafaq(shafaq_id).ok_or_else(|| format!("invalid_input: shafaq {shafaq_id}"))?;
    match rule {
        HighLatitudeRule::SeasonAdjusted(_) => Ok(HighLatitudeRule::SeasonAdjusted(chosen)),
        HighLatitudeRule::Recommended
        | HighLatitudeRule::MiddleOfTheNight
        | HighLatitudeRule::SeventhOfTheNight
        | HighLatitudeRule::TwilightAngle
        | HighLatitudeRule::None => {
            Err("invalid_input: shafaq applies only to the season_adjusted rule".to_owned())
        }
    }
}

fn asr_shadow(id: &str) -> Option<AsrShadow> {
    match id {
        "standard" => Some(AsrShadow::Standard),
        "hanafi" => Some(AsrShadow::Hanafi),
        _ => None,
    }
}

/// Exact conversion: instants from the core fit in `i32` minutes (years 1900..=2100).
fn millis(instant: Instant) -> Option<f64> {
    i32::try_from(instant.unix_minutes())
        .ok()
        .map(|minutes| f64::from(minutes) * 60_000.0_f64)
}

fn flag(value: bool) -> f64 {
    if value { 1.0 } else { 0.0 }
}

/// Computes one day's schedule. See [`LAYOUT`] for the shape of the result. Pass an empty
/// `high_latitude` or `shafaq` for the method's own.
///
/// # Errors
///
/// A message prefixed with a stable code: `invalid_input:`, `unknown_method:`, `compute:`.
#[wasm_bindgen]
#[expect(
    clippy::too_many_arguments,
    reason = "flat numeric ABI keeps the wasm boundary trivial"
)]
pub fn compute(
    year: i16,
    month: u8,
    day: u8,
    latitude: f64,
    longitude: f64,
    elevation: f64,
    method: &str,
    asr: &str,
    high_latitude: &str,
    shafaq: &str,
    ramadan: &[i16],
) -> Result<Vec<f64>, String> {
    let invalid = |e: hya_engine::InvalidInput| format!("invalid_input: {e}");
    let date = Date::new(year, month, day).map_err(invalid)?;
    let coordinates = Coordinates::new(latitude, longitude)
        .map_err(invalid)?
        .with_elevation(Elevation::new(elevation).map_err(invalid)?);
    let method: Method = method
        .parse()
        .map_err(|_| format!("unknown_method: {method}"))?;
    let parameters = Parameters {
        asr: asr_shadow(asr).ok_or_else(|| format!("invalid_input: asr {asr}"))?,
        high_latitude: resolve_high_latitude(method, high_latitude, shafaq)?,
        ramadan: ramadan_dates(ramadan)?,
        ..method.parameters()
    };
    let times =
        hya_engine::compute(date, coordinates, parameters).map_err(|e| format!("compute: {e}"))?;
    let overflow = || "compute: time out of range".to_owned();
    let mut out = Vec::with_capacity(LAYOUT.len());
    for prayer in Prayer::ALL {
        out.push(millis(times.get(prayer)).ok_or_else(overflow)?);
    }
    for mark in [times.midnight(), times.last_third()] {
        out.push(match mark {
            Some(instant) => millis(instant).ok_or_else(overflow)?,
            None => f64::NAN,
        });
    }
    let notes = times.notes();
    out.extend([
        flag(notes.fajr_high_latitude),
        flag(notes.isha_high_latitude),
        flag(notes.ramadan_isha),
        flag(notes.maghrib_at_sunset),
    ]);
    Ok(out)
}

/// `[]` for the calculated calendar, or `[y, m, d, y, m, d]`: the announced first and last day.
fn ramadan_dates(raw: &[i16]) -> Result<RamadanDates, String> {
    let invalid = |what: &str| format!("invalid_input: ramadan {what}");
    let date = |y: i16, m: i16, d: i16| {
        let month = u8::try_from(m).map_err(|_| invalid("month"))?;
        let day = u8::try_from(d).map_err(|_| invalid("day"))?;
        Date::new(y, month, day).map_err(|e| format!("invalid_input: ramadan {e}"))
    };
    match *raw {
        [] => Ok(RamadanDates::Calculated),
        [y1, m1, d1, y2, m2, d2] => AnnouncedRamadan::new(date(y1, m1, d1)?, date(y2, m2, d2)?)
            .map(RamadanDates::Announced)
            .map_err(|e| format!("invalid_input: {e}")),
        _ => Err(invalid("must be empty or six numbers")),
    }
}

/// Whether the date is in Ramadan per the Umm al-Qura calendar.
///
/// # Errors
///
/// `invalid_input:` for an invalid or out-of-range date.
#[wasm_bindgen(js_name = isRamadan)]
pub fn is_ramadan(year: i16, month: u8, day: u8) -> Result<bool, String> {
    Date::new(year, month, day)
        .map(hya_engine::is_ramadan)
        .map_err(|e| format!("invalid_input: {e}"))
}

/// Identifiers of every built-in method.
#[wasm_bindgen]
#[must_use]
pub fn methods() -> Vec<String> {
    Method::ALL.iter().map(|m| m.id().to_owned()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn returns_full_layout() {
        let out = compute(
            2026,
            10,
            29,
            24.7136,
            46.6753,
            0.0,
            "umm_al_qura",
            "standard",
            "recommended",
            "",
            &[],
        )
        .expect("ok");
        assert_eq!(out.len(), LAYOUT.len(), "layout");
        assert!(out.windows(2).take(5).all(|w| w[0] < w[1]), "ordered");
    }

    #[test]
    fn ramadan_dates_parse_or_fail_typed() {
        assert_eq!(ramadan_dates(&[]), Ok(RamadanDates::Calculated), "empty");
        assert!(
            matches!(
                ramadan_dates(&[2026, 2, 19, 2026, 3, 20]),
                Ok(RamadanDates::Announced(_))
            ),
            "late sighting"
        );
        for bad in [
            &[2026, 2, 19][..],
            &[2026, 2, 19, 2026, 3, 30],
            &[2026, -2, 19, 2026, 3, 20],
            &[2027, 2, 19, 2027, 3, 20],
        ] {
            assert!(
                ramadan_dates(bad).is_err_and(|e| e.starts_with("invalid_input: ")),
                "{bad:?}"
            );
        }
    }

    #[test]
    fn high_latitude_defaults_to_the_methods_own() {
        let mc = Method::MoonsightingCommittee;
        let own = HighLatitudeRule::SeasonAdjusted(Shafaq::General);
        assert_eq!(resolve_high_latitude(mc, "", ""), Ok(own), "own rule");
        assert_eq!(
            resolve_high_latitude(Method::UmmAlQura, "", ""),
            Ok(HighLatitudeRule::Recommended),
            "others recommend"
        );
        assert_eq!(
            resolve_high_latitude(mc, "", "white"),
            Ok(HighLatitudeRule::SeasonAdjusted(Shafaq::White)),
            "shafaq"
        );
        assert_eq!(
            resolve_high_latitude(Method::UmmAlQura, "season_adjusted", "red"),
            Ok(HighLatitudeRule::SeasonAdjusted(Shafaq::Red)),
            "any method may use it"
        );
        assert_eq!(
            resolve_high_latitude(mc, "middle_of_the_night", ""),
            Ok(HighLatitudeRule::MiddleOfTheNight),
            "explicit override"
        );
        for (rule, shafaq) in [("", "pink"), ("recommended", "red"), ("bogus", "")] {
            assert!(
                resolve_high_latitude(mc, rule, shafaq)
                    .is_err_and(|e| e.starts_with("invalid_input:")),
                "{rule:?} {shafaq:?}"
            );
        }
    }

    #[test]
    fn errors_carry_stable_codes() {
        let err = |r: Result<Vec<f64>, String>| r.expect_err("should fail");
        assert!(
            err(compute(
                2026,
                2,
                30,
                0.0,
                0.0,
                0.0,
                "umm_al_qura",
                "standard",
                "recommended",
                "",
                &[]
            ))
            .starts_with("invalid_input:"),
            "date"
        );
        assert!(
            err(compute(
                2026,
                1,
                1,
                0.0,
                0.0,
                0.0,
                "nope",
                "standard",
                "recommended",
                "",
                &[]
            ))
            .starts_with("unknown_method:"),
            "method"
        );
        assert!(
            err(compute(
                2026,
                12,
                21,
                78.2,
                15.6,
                0.0,
                "umm_al_qura",
                "standard",
                "recommended",
                "",
                &[]
            ))
            .starts_with("compute:"),
            "polar"
        );
    }
}
