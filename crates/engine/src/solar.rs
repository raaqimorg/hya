//! Low-precision solar ephemeris (U.S. Naval Observatory approximation), accurate to about a
//! minute of time for 1900..=2100, which is the granularity prayer times are published at.
//!
//! Times here are fractional hours of *local mean solar time* (longitude-based, not civil).

use core::f64::consts::PI;

const DEGREES: f64 = PI / 180.0;

fn sin_deg(degrees: f64) -> f64 {
    libm::sin(degrees * DEGREES)
}

fn cos_deg(degrees: f64) -> f64 {
    libm::cos(degrees * DEGREES)
}

fn tan_deg(degrees: f64) -> f64 {
    libm::tan(degrees * DEGREES)
}

fn asin_deg(x: f64) -> f64 {
    libm::asin(x) / DEGREES
}

fn acos_deg(x: f64) -> f64 {
    libm::acos(x) / DEGREES
}

fn atan2_deg(y: f64, x: f64) -> f64 {
    libm::atan2(y, x) / DEGREES
}

fn acot_deg(x: f64) -> f64 {
    libm::atan(1.0 / x) / DEGREES
}

/// `value` reduced into `0..=modulus` (floor-based, so never negative; rounding can land a
/// tiny negative input exactly on `modulus`, which is the same angle or time of day).
pub(crate) fn wrap(value: f64, modulus: f64) -> f64 {
    value - modulus * libm::floor(value / modulus)
}

/// `hours` reduced into `0..24`.
pub(crate) fn wrap_hours(hours: f64) -> f64 {
    wrap(hours, 24.0)
}

/// Julian Day at 00:00 UTC of the given day (days since 1970-01-01).
pub(crate) fn julian_day(days_since_unix_epoch: i32) -> f64 {
    2_440_587.5 + f64::from(days_since_unix_epoch)
}

#[derive(Debug, Clone, Copy)]
struct SunPosition {
    declination: f64,
    equation_of_time: f64,
}

impl SunPosition {
    /// Solar transit, in local mean solar hours.
    fn transit(self) -> f64 {
        wrap_hours(12.0 - self.equation_of_time)
    }
}

fn sun_position(julian_day: f64) -> SunPosition {
    let days = julian_day - 2_451_545.0_f64;
    let mean_anomaly = wrap(357.529 + 0.985_600_28 * days, 360.0);
    let mean_longitude = wrap(280.459 + 0.985_647_36 * days, 360.0);
    let ecliptic_longitude = wrap(
        mean_longitude + 1.915 * sin_deg(mean_anomaly) + 0.020 * sin_deg(2.0 * mean_anomaly),
        360.0,
    );
    let obliquity = 23.439_f64 - 0.000_000_36_f64 * days;
    let right_ascension = atan2_deg(
        cos_deg(obliquity) * sin_deg(ecliptic_longitude),
        cos_deg(ecliptic_longitude),
    ) / 15.0_f64;
    SunPosition {
        declination: asin_deg(sin_deg(obliquity) * sin_deg(ecliptic_longitude)),
        equation_of_time: mean_longitude / 15.0 - wrap_hours(right_ascension),
    }
}

/// Which side of solar noon an event falls on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Side {
    Morning,
    Evening,
}

/// The sun as seen from one latitude on one day.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Sky {
    /// Julian Day of local midnight (UTC midnight shifted by longitude).
    julian_day: f64,
    latitude: f64,
}

impl Sky {
    pub(crate) fn new(days_since_unix_epoch: i32, latitude: f64, longitude: f64) -> Self {
        Self {
            julian_day: julian_day(days_since_unix_epoch) - longitude / (15.0 * 24.0),
            latitude,
        }
    }

    fn sun_at(self, hours: f64) -> SunPosition {
        sun_position(self.julian_day + hours / 24.0)
    }

    /// Solar transit, evaluated near `hours`.
    pub(crate) fn noon(self, hours: f64) -> f64 {
        self.sun_at(hours).transit()
    }

    /// When the sun's centre is `depression` degrees below the horizon, near `hours`.
    /// `None` if the sun never reaches that altitude on this day.
    pub(crate) fn time_at_depression(self, depression: f64, hours: f64, side: Side) -> Option<f64> {
        self.time_at_altitude(-depression, hours, side)
    }

    fn time_at_altitude(self, altitude: f64, hours: f64, side: Side) -> Option<f64> {
        self.time_at_altitude_for(self.sun_at(hours), altitude, side)
    }

    /// [`Sky::time_at_altitude`] for a sun position already evaluated, so a caller that needs
    /// it too does not evaluate it twice.
    fn time_at_altitude_for(self, sun: SunPosition, altitude: f64, side: Side) -> Option<f64> {
        let declination = sun.declination;
        let cos_hour_angle = (sin_deg(altitude) - sin_deg(declination) * sin_deg(self.latitude))
            / (cos_deg(declination) * cos_deg(self.latitude));
        if !cos_hour_angle.is_finite() || !(-1.0_f64..=1.0_f64).contains(&cos_hour_angle) {
            return None;
        }
        let offset = acos_deg(cos_hour_angle) / 15.0_f64;
        let noon = sun.transit();
        Some(match side {
            Side::Morning => noon - offset,
            Side::Evening => noon + offset,
        })
    }

    /// Asr: when an object's shadow is `factor` times its length plus its noon shadow.
    pub(crate) fn asr(self, factor: f64, hours: f64) -> Option<f64> {
        // The noon shadow is the shadow at transit, so it uses the declination at transit.
        let sun = self.sun_at(hours);
        let declination = self.sun_at(sun.transit()).declination;
        let altitude = acot_deg(factor + tan_deg(libm::fabs(self.latitude - declination)));
        self.time_at_altitude_for(sun, altitude, Side::Evening)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wrap_stays_in_range() {
        for value in [
            -725.5_f64, -360.0_f64, -0.1_f64, 0.0_f64, 359.9_f64, 360.0_f64, 1e6_f64,
        ] {
            let w = wrap(value, 360.0);
            assert!((0.0_f64..=360.0_f64).contains(&w), "{value} -> {w}");
        }
    }

    /// Pins the transcription of the USNO formula, including the slow obliquity drift (worth
    /// ~0.013° by 2100, far below a minute, so no end-to-end test can see it). Reference values
    /// come from an independent Python evaluation of the same published formula.
    #[test]
    fn formula_matches_independent_evaluation_at_century_ends() {
        for (julian_day, declination, equation_of_time) in [
            (
                2_488_045.0_f64,
                -22.655_709_023_593_364_f64,
                0.142_980_770_107_701_72_f64,
            ),
            (
                2_415_045.0_f64,
                -19.003_308_084_623_46_f64,
                -0.208_124_871_797_284_78_f64,
            ),
        ] {
            let sun = sun_position(julian_day);
            assert!(
                (sun.declination - declination).abs() < 1e-9_f64,
                "declination at {julian_day}"
            );
            assert!(
                (sun.equation_of_time - equation_of_time).abs() < 1e-9_f64,
                "eot at {julian_day}"
            );
        }
    }

    #[test]
    fn equinox_declination_is_near_zero() {
        // 2026-03-20 ≈ 20_532 days since epoch.
        let sun = sun_position(julian_day(20_532) + 0.5);
        assert!(
            sun.declination.abs() < 0.6_f64,
            "declination {}",
            sun.declination
        );
    }

    #[test]
    fn solstice_declination_is_near_tilt() {
        // 2026-06-21 = 20_625 days since epoch.
        let sun = sun_position(julian_day(20_625) + 0.5);
        assert!(
            (sun.declination - 23.44).abs() < 0.05_f64,
            "declination {}",
            sun.declination
        );
    }

    #[test]
    fn polar_night_has_no_sunrise() {
        // Longyearbyen (78°N) on 2026-12-21 (20_808).
        let sky = Sky::new(20_808, 78.2, 15.6);
        assert_eq!(
            sky.time_at_depression(0.833, 6.0, Side::Morning),
            None,
            "polar night"
        );
    }
}
