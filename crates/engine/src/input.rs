//! Branded, bounded input types.
//!
//! Each type wraps a primitive behind a private field, so the only way to obtain one is a
//! checked constructor. Code that receives a [`Latitude`] never has to re-check its range.

use core::fmt;

/// Why an input value was rejected.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum InvalidInput {
    /// The value was NaN or infinite.
    NotFinite,
    /// Latitude outside `-90..=90` degrees.
    LatitudeOutOfRange,
    /// Longitude outside `-180..=180` degrees.
    LongitudeOutOfRange,
    /// Elevation outside `-500..=9000` metres.
    ElevationOutOfRange,
    /// Twilight angle outside `0.5..=30` degrees.
    TwilightAngleOutOfRange,
    /// Isha interval outside `1..=240` minutes.
    IshaIntervalOutOfRange,
    /// Minute adjustment outside `-120..=120`.
    AdjustmentOutOfRange,
    /// Date outside the supported years (see [`crate::Date::MIN_YEAR`] and [`crate::Date::MAX_YEAR`]).
    DateOutOfRange,
    /// The month/day combination does not exist (for example 30 February).
    NonexistentDate,
    /// An announced Ramadan must span 29 or 30 days.
    RamadanLength,
    /// An announced Ramadan must start within three days of a calculated one.
    RamadanTooFarFromCalendar,
}

impl fmt::Display for InvalidInput {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::NotFinite => "value must be a finite number",
            Self::LatitudeOutOfRange => "latitude must be within -90..=90 degrees",
            Self::LongitudeOutOfRange => "longitude must be within -180..=180 degrees",
            Self::ElevationOutOfRange => "elevation must be within -500..=9000 metres",
            Self::TwilightAngleOutOfRange => "twilight angle must be within 0.5..=30 degrees",
            Self::IshaIntervalOutOfRange => "isha interval must be within 1..=240 minutes",
            Self::AdjustmentOutOfRange => "adjustment must be within -120..=120 minutes",
            Self::DateOutOfRange => "year must be within 1900..=2100",
            Self::NonexistentDate => "no such calendar date",
            Self::RamadanLength => "an announced Ramadan must span 29 or 30 days",
            Self::RamadanTooFarFromCalendar => {
                "an announced Ramadan must start within 3 days of the Umm al-Qura calendar"
            }
        })
    }
}

impl core::error::Error for InvalidInput {}

macro_rules! bounded_f64 {
    ($(#[$doc:meta])* $name:ident, $min:literal ..= $max:literal, $error:ident) => {
        $(#[$doc])*
        #[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
        pub struct $name(f64);

        impl $name {
            /// Smallest accepted value.
            pub const MIN: f64 = $min;
            /// Largest accepted value.
            pub const MAX: f64 = $max;

            /// Validates `value`.
            ///
            /// # Errors
            ///
            #[doc = concat!("[`InvalidInput::NotFinite`] for NaN or infinity, [`InvalidInput::", stringify!($error), "`] when out of range.")]
            pub fn new(value: f64) -> Result<Self, InvalidInput> {
                if !value.is_finite() {
                    return Err(InvalidInput::NotFinite);
                }
                if value < Self::MIN || value > Self::MAX {
                    return Err(InvalidInput::$error);
                }
                Ok(Self(value))
            }

            /// The underlying number.
            #[must_use]
            pub const fn get(self) -> f64 {
                self.0
            }
        }

        impl TryFrom<f64> for $name {
            type Error = InvalidInput;

            fn try_from(value: f64) -> Result<Self, Self::Error> {
                Self::new(value)
            }
        }

        impl From<$name> for f64 {
            fn from(value: $name) -> Self {
                value.0
            }
        }
    };
}

bounded_f64!(
    /// Geographic latitude in degrees, `-90..=90` (north positive).
    Latitude, -90.0 ..= 90.0, LatitudeOutOfRange
);
bounded_f64!(
    /// Geographic longitude in degrees, `-180..=180` (east positive).
    Longitude, -180.0 ..= 180.0, LongitudeOutOfRange
);
bounded_f64!(
    /// Height above sea level in metres, `-500..=9000`. Raises the horizon dip at sunrise/sunset.
    Elevation, -500.0 ..= 9000.0, ElevationOutOfRange
);
bounded_f64!(
    /// Depression angle of the sun below the horizon in degrees, `0.5..=30`.
    TwilightAngle, 0.5 ..= 30.0, TwilightAngleOutOfRange
);

impl TwilightAngle {
    /// Builds an angle the crate itself guarantees is in range (built-in method tables).
    /// Every call site is covered by a test that re-validates it through [`Self::new`].
    pub(crate) const fn trusted(value: f64) -> Self {
        Self(value)
    }
}

impl Default for Elevation {
    /// Sea level.
    fn default() -> Self {
        Self(0.0)
    }
}

macro_rules! bounded_int {
    ($(#[$doc:meta])* $name:ident($int:ty), $min:literal ..= $max:literal, $error:ident) => {
        $(#[$doc])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
        pub struct $name($int);

        impl $name {
            /// Smallest accepted value.
            pub const MIN: $int = $min;
            /// Largest accepted value.
            pub const MAX: $int = $max;

            /// Validates `value`.
            ///
            /// # Errors
            ///
            #[doc = concat!("[`InvalidInput::", stringify!($error), "`] when out of range.")]
            pub const fn new(value: $int) -> Result<Self, InvalidInput> {
                if value < Self::MIN || value > Self::MAX {
                    return Err(InvalidInput::$error);
                }
                Ok(Self(value))
            }

            /// Builds a value the crate itself guarantees is in range (built-in method tables).
            pub(crate) const fn trusted(value: $int) -> Self {
                Self(value)
            }

            /// The underlying number.
            #[must_use]
            pub const fn get(self) -> $int {
                self.0
            }
        }

        impl TryFrom<$int> for $name {
            type Error = InvalidInput;

            fn try_from(value: $int) -> Result<Self, Self::Error> {
                Self::new(value)
            }
        }
    };
}

bounded_int!(
    /// Minutes between Maghrib and Isha for interval-based methods, `1..=240`.
    IshaInterval(u16), 1 ..= 240, IshaIntervalOutOfRange
);
bounded_int!(
    /// A signed correction in minutes applied to one prayer, `-120..=120`.
    MinuteAdjustment(i16), -120 ..= 120, AdjustmentOutOfRange
);

/// A validated point on Earth.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Coordinates {
    /// North-positive latitude.
    pub latitude: Latitude,
    /// East-positive longitude.
    pub longitude: Longitude,
    /// Height above sea level.
    pub elevation: Elevation,
}

impl Default for Latitude {
    fn default() -> Self {
        Self(0.0)
    }
}

impl Default for Longitude {
    fn default() -> Self {
        Self(0.0)
    }
}

impl Coordinates {
    /// Validates a latitude/longitude pair at sea level.
    ///
    /// # Errors
    ///
    /// Propagates [`Latitude::new`] and [`Longitude::new`] errors.
    pub fn new(latitude: f64, longitude: f64) -> Result<Self, InvalidInput> {
        Ok(Self {
            latitude: Latitude::new(latitude)?,
            longitude: Longitude::new(longitude)?,
            elevation: Elevation::default(),
        })
    }

    /// Returns a copy at the given elevation.
    #[must_use]
    pub const fn with_elevation(self, elevation: Elevation) -> Self {
        Self { elevation, ..self }
    }
}
