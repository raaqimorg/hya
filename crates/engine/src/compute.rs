//! The prayer-time calculation.

use core::fmt;

use crate::date::Date;
use crate::date::Instant;
use crate::input::{Coordinates, MinuteAdjustment};
use crate::method::{DhuhrRule, HighLatitudeRule, IshaRule, MaghribRule, Parameters, Rounding};
use crate::seasonal;
use crate::solar::{Side, Sky, wrap_hours};

/// Sun depression at sunrise/sunset: refraction (34′) plus the solar semi-diameter (16′).
const HORIZON_DEPRESSION: f64 = 0.833;
/// Above this latitude [`HighLatitudeRule::Recommended`] uses the seventh-of-the-night rule.
const HIGH_LATITUDE: f64 = 48.0;
/// The falak convention for the sun's trailing limb crossing the meridian: a 16′ semidiameter
/// at 15°/h, i.e. 64 seconds. Malaysia's official timetables match this, not the exact
/// declination-dependent delay (1.07–1.16 min), on 98% of days (`tests/official.rs`).
const LIMB_DELAY_HOURS: f64 = 64.0 / 3600.0;

/// Above this latitude [`HighLatitudeRule::SeasonAdjusted`] uses a seventh of the night.
const SEASONAL_SEVENTH_LATITUDE: f64 = 55.0;
const MINUTES_PER_DAY: i64 = 1440;
/// Results more than a week away from the requested day are rejected as nonsensical.
const MAX_OFFSET_MINUTES: f64 = 7.0 * 1440.0;

/// One of the daily times.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Prayer {
    /// Dawn prayer.
    Fajr,
    /// Sunrise (end of Fajr).
    Sunrise,
    /// Noon prayer.
    Dhuhr,
    /// Afternoon prayer.
    Asr,
    /// Sunset prayer.
    Maghrib,
    /// Night prayer.
    Isha,
}

impl Prayer {
    /// The six times in daily order.
    pub const ALL: [Self; 6] = [
        Self::Fajr,
        Self::Sunrise,
        Self::Dhuhr,
        Self::Asr,
        Self::Maghrib,
        Self::Isha,
    ];

    /// Stable machine identifier, e.g. `"fajr"`.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Fajr => "fajr",
            Self::Sunrise => "sunrise",
            Self::Dhuhr => "dhuhr",
            Self::Asr => "asr",
            Self::Maghrib => "maghrib",
            Self::Isha => "isha",
        }
    }
}

impl fmt::Display for Prayer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.id())
    }
}

/// Why no schedule could be produced for a date and place.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum ComputeError {
    /// The sun does not rise or does not set on this day (polar day or night).
    PolarDayOrNight,
    /// This time does not occur, and no high-latitude rule was allowed to substitute it.
    Undefined(Prayer),
    /// The times came out of order: large minute adjustments, or rounding near the poles.
    OutOfOrder,
    /// A result fell outside the representable range.
    Unrepresentable,
}

impl fmt::Display for ComputeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PolarDayOrNight => f.write_str("the sun does not rise or set on this day"),
            Self::Undefined(prayer) => write!(f, "{prayer} does not occur on this day"),
            Self::OutOfOrder => f.write_str("computed times are not in daily order"),
            Self::Unrepresentable => f.write_str("computed time is out of range"),
        }
    }
}

impl core::error::Error for ComputeError {}

/// How a schedule deviated from the plain astronomical definition.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "independent yes/no facts about one schedule, not states of one machine"
)]
pub struct Notes {
    /// Fajr was set by the high-latitude rule.
    pub fajr_high_latitude: bool,
    /// Isha was set by the high-latitude rule.
    pub isha_high_latitude: bool,
    /// Isha used the method's Ramadan interval.
    pub ramadan_isha: bool,
    /// The method's Maghrib angle was not used, because the sun does not reach it, or it falls
    /// at or after Isha (short summer nights), so Maghrib is sunset.
    pub maghrib_at_sunset: bool,
}

/// A day's schedule.
///
/// Invariant (enforced by [`compute`]): `fajr < sunrise < dhuhr < asr < maghrib < isha` and
/// `maghrib < midnight < last_third`. Fields are private so the invariant cannot be broken.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PrayerTimes {
    fajr: Instant,
    sunrise: Instant,
    dhuhr: Instant,
    asr: Instant,
    maghrib: Instant,
    isha: Instant,
    midnight: Instant,
    last_third: Instant,
    notes: Notes,
}

impl PrayerTimes {
    /// Dawn.
    #[must_use]
    pub const fn fajr(&self) -> Instant {
        self.fajr
    }

    /// Sunrise.
    #[must_use]
    pub const fn sunrise(&self) -> Instant {
        self.sunrise
    }

    /// Noon prayer.
    #[must_use]
    pub const fn dhuhr(&self) -> Instant {
        self.dhuhr
    }

    /// Afternoon prayer.
    #[must_use]
    pub const fn asr(&self) -> Instant {
        self.asr
    }

    /// Sunset prayer.
    #[must_use]
    pub const fn maghrib(&self) -> Instant {
        self.maghrib
    }

    /// Night prayer.
    #[must_use]
    pub const fn isha(&self) -> Instant {
        self.isha
    }

    /// Halfway between sunset and the next sunrise.
    #[must_use]
    pub const fn midnight(&self) -> Instant {
        self.midnight
    }

    /// Start of the last third of the night.
    #[must_use]
    pub const fn last_third(&self) -> Instant {
        self.last_third
    }

    /// How this schedule deviated from the plain astronomical definition.
    #[must_use]
    pub const fn notes(&self) -> Notes {
        self.notes
    }

    /// The time for one prayer.
    #[must_use]
    pub const fn get(&self, prayer: Prayer) -> Instant {
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

/// Initial guesses (local solar hours) refined by [`refine`].
#[derive(Debug, Clone, Copy)]
struct Estimate {
    fajr: Option<f64>,
    sunrise: Option<f64>,
    dhuhr: f64,
    asr: Option<f64>,
    sunset: Option<f64>,
    maghrib: Option<f64>,
    isha: Option<f64>,
}

impl Estimate {
    const INITIAL: Self = Self {
        fajr: Some(5.0_f64),
        sunrise: Some(6.0_f64),
        dhuhr: 12.0,
        asr: Some(13.0_f64),
        sunset: Some(18.0_f64),
        maghrib: Some(18.0_f64),
        isha: Some(18.0_f64),
    };
}

fn refine(sky: Sky, parameters: Parameters, horizon: f64, guess: Estimate) -> Estimate {
    let fallback = Estimate::INITIAL;
    let at = |value: Option<f64>, default: Option<f64>| value.or(default).unwrap_or(12.0_f64);
    Estimate {
        fajr: sky.time_at_depression(
            parameters.fajr.get(),
            at(guess.fajr, fallback.fajr),
            Side::Morning,
        ),
        sunrise: sky.time_at_depression(
            horizon,
            at(guess.sunrise, fallback.sunrise),
            Side::Morning,
        ),
        dhuhr: sky.noon(guess.dhuhr),
        asr: sky.asr(parameters.asr.factor(), at(guess.asr, fallback.asr)),
        sunset: sky.time_at_depression(horizon, at(guess.sunset, fallback.sunset), Side::Evening),
        maghrib: match parameters.maghrib {
            MaghribRule::Sunset => None,
            MaghribRule::Angle(angle) => sky.time_at_depression(
                angle.get(),
                at(guess.maghrib, fallback.maghrib),
                Side::Evening,
            ),
        },
        isha: match parameters.isha {
            IshaRule::Interval { .. } => None,
            IshaRule::Angle(angle) => {
                sky.time_at_depression(angle.get(), at(guess.isha, fallback.isha), Side::Evening)
            }
        },
    }
}

/// Fraction of the night that bounds Fajr/Isha under `rule`, given that prayer's angle.
fn night_fraction(rule: HighLatitudeRule, latitude: f64, angle: f64) -> Option<f64> {
    match rule {
        HighLatitudeRule::Recommended if libm::fabs(latitude) > HIGH_LATITUDE => Some(1.0 / 7.0),
        HighLatitudeRule::Recommended | HighLatitudeRule::MiddleOfTheNight => Some(0.5),
        HighLatitudeRule::SeventhOfTheNight => Some(1.0 / 7.0),
        HighLatitudeRule::TwilightAngle => Some(angle / 60.0),
        HighLatitudeRule::None | HighLatitudeRule::SeasonAdjusted(_) => None,
    }
}

/// How long a twilight (Fajr → sunrise, or sunset → Isha) lasts, in hours, and whether the
/// high-latitude rule set it. `astronomical` is its length at `angle`, `None` if the sun never
/// gets that deep. `None` means the prayer does not occur.
fn twilight_length(
    rule: HighLatitudeRule,
    side: Side,
    astronomical: Option<f64>,
    angle: f64,
    (date, latitude, night): (Date, f64, f64),
) -> Result<Option<(f64, bool)>, ComputeError> {
    let (base, replaced, limit) = match rule {
        HighLatitudeRule::SeasonAdjusted(shafaq) => {
            let limit = seasonal::twilight_hours(side, shafaq, latitude, date)
                .ok_or(ComputeError::Unrepresentable)?;
            if libm::fabs(latitude) >= SEASONAL_SEVENTH_LATITUDE {
                (Some(night / 7.0_f64), true, Some(limit))
            } else {
                (astronomical, false, Some(limit))
            }
        }
        other @ (HighLatitudeRule::Recommended
        | HighLatitudeRule::MiddleOfTheNight
        | HighLatitudeRule::SeventhOfTheNight
        | HighLatitudeRule::TwilightAngle
        | HighLatitudeRule::None) => (
            astronomical,
            false,
            night_fraction(other, latitude, angle).map(|fraction| night * fraction),
        ),
    };
    Ok(match (base, limit) {
        (Some(length), Some(limit)) if length > limit => Some((limit, true)),
        (Some(length), _) => Some((length, replaced)),
        (None, Some(limit)) => Some((limit, true)),
        (None, None) => None,
    })
}

/// Converts local solar hours on `date` to a rounded UTC instant, then applies a whole-minute
/// adjustment (after rounding, so an adjustment of k shifts the result by exactly k minutes).
fn to_instant(
    date: Date,
    local_hours: f64,
    longitude: f64,
    rounding: Rounding,
    adjustment: MinuteAdjustment,
) -> Result<Instant, ComputeError> {
    let offset = (local_hours - longitude / 15.0_f64) * 60.0_f64;
    let rounded = match rounding {
        Rounding::Nearest => libm::round(offset),
        Rounding::Up => libm::ceil(offset),
    };
    if !rounded.is_finite() || libm::fabs(rounded) > MAX_OFFSET_MINUTES {
        return Err(ComputeError::Unrepresentable);
    }
    #[expect(
        clippy::as_conversions,
        clippy::cast_possible_truncation,
        reason = "finite, integral after rounding, and bounded to ±10_080 just above"
    )]
    let offset_minutes = rounded as i64;
    i64::from(date.days_since_unix_epoch())
        .checked_mul(MINUTES_PER_DAY)
        .and_then(|day_start| day_start.checked_add(offset_minutes))
        .and_then(|instant| instant.checked_add(i64::from(adjustment.get())))
        .map(Instant::from_unix_minutes)
        .ok_or(ComputeError::Unrepresentable)
}

/// Midnight and the last third fall strictly after Maghrib. Midnight is measured from sunset,
/// so a large Maghrib adjustment on a short night can overtake it. Kept separate from the
/// prayer ordering so mutation testing can name it: the `<` → `<=` mutants are equivalent,
/// since a tie needs a zero-length night, which polar-day detection rejects first.
fn night_marks_are_ordered(times: &PrayerTimes) -> bool {
    times.maghrib < times.midnight && times.midnight < times.last_third
}

/// Maghrib, and whether the method's angle had to give way to sunset. As adhan-js does, an
/// angle-based Maghrib applies only where it falls before Isha: on a short night the
/// high-latitude rule can put Isha within minutes of sunset, before the angle.
fn maghrib_or_sunset(rule: MaghribRule, angle: Option<f64>, sunset: f64, isha: f64) -> (f64, bool) {
    match (rule, angle) {
        (_, Some(angle)) if angle < isha => (angle, false),
        (MaghribRule::Angle(_), _) => (sunset, true),
        (MaghribRule::Sunset, _) => (sunset, false),
    }
}

/// Computes the prayer times for `date` at `coordinates`.
///
/// `date` is the local *solar* day at the longitude: the day whose solar noon is the result's
/// Dhuhr. That is the civil date almost everywhere. Where a time zone runs far ahead of the
/// sun (UTC+13 and UTC+14 in the Pacific), the civil date is a day later, so a caller with a
/// civil date and a zone should take the neighbouring day whose Dhuhr falls on that date in
/// the zone, as the CLI and the JS package's `timeZone` option do.
///
/// Pure: the same inputs always give the same output, on every platform.
///
/// # Errors
///
/// - [`ComputeError::PolarDayOrNight`] when the sun does not rise or set.
/// - [`ComputeError::Undefined`] when a time does not occur and the high-latitude rule is
///   [`HighLatitudeRule::None`], or when Asr does not occur (a noon sun at the horizon).
/// - [`ComputeError::OutOfOrder`] if adjustments push times out of daily order.
/// - [`ComputeError::Unrepresentable`] if a result is absurdly far from `date`.
pub fn compute(
    date: Date,
    coordinates: Coordinates,
    parameters: Parameters,
) -> Result<PrayerTimes, ComputeError> {
    let latitude = coordinates.latitude.get();
    let longitude = coordinates.longitude.get();
    let horizon =
        HORIZON_DEPRESSION + 0.0347_f64 * libm::sqrt(coordinates.elevation.get().max(0.0));
    let sky = Sky::new(date.days_since_unix_epoch(), latitude, longitude);

    let first = refine(sky, parameters, horizon, Estimate::INITIAL);
    let raw = refine(sky, parameters, horizon, first);

    let (Some(sunrise), Some(sunset)) = (raw.sunrise, raw.sunset) else {
        return Err(ComputeError::PolarDayOrNight);
    };
    let dhuhr = match parameters.dhuhr {
        DhuhrRule::Transit => raw.dhuhr,
        DhuhrRule::AfterTheLimb => raw.dhuhr + LIMB_DELAY_HOURS,
    };
    let asr = raw.asr.ok_or(ComputeError::Undefined(Prayer::Asr))?;
    // An angle-based Maghrib, when the method has one and the sun reaches it.
    let angle_maghrib = match parameters.maghrib {
        MaghribRule::Sunset => None,
        MaghribRule::Angle(_) => raw.maghrib,
    };

    let night = wrap_hours(sunrise - sunset);
    let mut notes = Notes::default();

    let context = (date, latitude, night);

    // Fajr: bounded by the high-latitude rule.
    let (morning, fajr_rule) = twilight_length(
        parameters.high_latitude,
        Side::Morning,
        raw.fajr.map(|fajr| wrap_hours(sunrise - fajr)),
        parameters.fajr.get(),
        context,
    )?
    .ok_or(ComputeError::Undefined(Prayer::Fajr))?;
    notes.fajr_high_latitude = fajr_rule;
    let fajr = sunrise - morning;

    // Isha: an interval after Maghrib, or an angle bounded by the high-latitude rule.
    let isha = match parameters.isha {
        IshaRule::Interval { regular, ramadan } => {
            // By calendar date, as the Umm al-Qura timetable and the Saudi Ministry of Islamic
            // Affairs apply it: the evening of a day in Ramadan gets the Ramadan interval. The
            // eve of 1 Ramadan does not (the sighting is announced after Maghrib), the eve of Eid
            // does (it is still a Ramadan day).
            notes.ramadan_isha = parameters.ramadan.contains(date);
            let interval = if notes.ramadan_isha { ramadan } else { regular };
            angle_maghrib.unwrap_or(sunset) + f64::from(interval.get()) / 60.0_f64
        }
        IshaRule::Angle(angle) => {
            let (evening, isha_rule) = twilight_length(
                parameters.high_latitude,
                Side::Evening,
                raw.isha.map(|isha| wrap_hours(isha - sunset)),
                angle.get(),
                context,
            )?
            .ok_or(ComputeError::Undefined(Prayer::Isha))?;
            notes.isha_high_latitude = isha_rule;
            sunset + evening
        }
    };
    let (maghrib, maghrib_at_sunset) =
        maghrib_or_sunset(parameters.maghrib, angle_maghrib, sunset, isha);
    notes.maghrib_at_sunset = maghrib_at_sunset;

    let adjust = parameters.adjustments;
    let none = MinuteAdjustment::default();
    let at = |hours: f64, adjustment| {
        to_instant(date, hours, longitude, parameters.rounding, adjustment)
    };
    let times = PrayerTimes {
        fajr: at(fajr, adjust.fajr)?,
        sunrise: at(sunrise, adjust.sunrise)?,
        dhuhr: at(dhuhr, adjust.dhuhr)?,
        asr: at(asr, adjust.asr)?,
        maghrib: at(maghrib, adjust.maghrib)?,
        isha: at(isha, adjust.isha)?,
        midnight: at(sunset + night / 2.0_f64, none)?,
        last_third: at(sunset + night * 2.0_f64 / 3.0_f64, none)?,
        notes,
    };

    let ordered = times.fajr < times.sunrise
        && times.sunrise < times.dhuhr
        && times.dhuhr < times.asr
        && times.asr < times.maghrib
        && times.maghrib < times.isha
        && night_marks_are_ordered(&times);
    if ordered {
        Ok(times)
    } else {
        Err(ComputeError::OutOfOrder)
    }
}
