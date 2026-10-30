//! Calculation methods and the parameters they expand to.
//!
//! A method is only a named preset. Everything the math needs lives in [`Parameters`], which
//! callers may build or tweak freely.

use core::fmt;
use core::str::FromStr;

use crate::input::{IshaInterval, MinuteAdjustment, TwilightAngle};
use crate::ramadan::RamadanDates;

/// The Ministry of Awqaf's margins for Amman: sunrise 7 min earlier, Maghrib 6 min later.
const JORDAN_ADJUSTMENTS: Adjustments = Adjustments {
    sunrise: MinuteAdjustment::trusted(-7),
    maghrib: MinuteAdjustment::trusted(6),
    ..Adjustments::NONE
};

/// Diyanet's margins, as adhan-js applies them.
const TURKEY_ADJUSTMENTS: Adjustments = Adjustments {
    sunrise: MinuteAdjustment::trusted(-7),
    dhuhr: MinuteAdjustment::trusted(5),
    asr: MinuteAdjustment::trusted(4),
    maghrib: MinuteAdjustment::trusted(7),
    ..Adjustments::NONE
};

/// Moonsighting Committee's published offsets: Dhuhr +5 min, Maghrib +3 min.
const MOONSIGHTING_COMMITTEE_ADJUSTMENTS: Adjustments = Adjustments {
    dhuhr: MinuteAdjustment::trusted(5),
    maghrib: MinuteAdjustment::trusted(3),
    ..Adjustments::NONE
};

/// With [`Rounding::Up`], a minute back rounds sunrise down: the cautious end of Fajr.
const SUNRISE_ROUNDED_DOWN: Adjustments = Adjustments {
    sunrise: MinuteAdjustment::trusted(-1),
    ..Adjustments::NONE
};

/// The official calculator's convention: times rounded up, sunrise rounded down.
const fn umm_al_qura(base: Parameters) -> Parameters {
    Parameters {
        fajr: TwilightAngle::trusted(18.5),
        isha: IshaRule::Interval {
            regular: IshaInterval::trusted(90),
            ramadan: IshaInterval::trusted(120),
        },
        rounding: Rounding::Up,
        adjustments: SUNRISE_ROUNDED_DOWN,
        ..base
    }
}

/// JAKIM's convention, with the Isha angle that differs by state.
const fn malaysia(base: Parameters, isha: TwilightAngle) -> Parameters {
    Parameters {
        isha: IshaRule::Angle(isha),
        dhuhr: DhuhrRule::AfterTheLimb,
        rounding: Rounding::Up,
        adjustments: SUNRISE_ROUNDED_DOWN,
        ..base
    }
}

/// A named calculation authority. Defaults to [`Method::UmmAlQura`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Method {
    /// Umm al-Qura University, Makkah (Saudi Arabia): Fajr 18.5°, Isha 90 min (120 in Ramadan),
    /// times rounded up and sunrise rounded down. Matches the official calculator at
    /// ummulqura.org.sa (`tests/official.rs`).
    #[default]
    UmmAlQura,
    /// Muslim World League: Fajr 18°, Isha 17°.
    MuslimWorldLeague,
    /// Egyptian General Authority of Survey: Fajr 19.5°, Isha 17.5°.
    Egyptian,
    /// University of Islamic Sciences, Karachi: Fajr 18°, Isha 18°.
    Karachi,
    /// Islamic Society of North America: Fajr 15°, Isha 15°.
    NorthAmerica,
    /// UAE: Fajr 18.2°, Isha 18.2°.
    Dubai,
    /// Kuwait: Fajr 18°, Isha 17.5°.
    Kuwait,
    /// Qatar: Fajr 18°, Isha 90 min.
    Qatar,
    /// Majlis Ugama Islam Singapura: Fajr 20°, Isha 18°, rounded up.
    Singapore,
    /// Diyanet İşleri Başkanlığı (Türkiye): Fajr 18°, Isha 17°.
    Turkey,
    /// Institute of Geophysics, University of Tehran: Fajr 17.7°, Isha 14°, Maghrib 4.5°.
    Tehran,
    /// Moonsighting Committee Worldwide (common in the UK and North America): Fajr and Isha 18°,
    /// never longer than its season-adjusted twilight; a seventh of the night above 55°.
    MoonsightingCommittee,
    /// JAKIM (Malaysia): Fajr 18° (since the 2019 Muzakarah), Isha 18°, Dhuhr once the sun's
    /// disc has crossed the meridian, times rounded up and sunrise rounded down. Matches the
    /// official e-solat timetables (`tests/official.rs`).
    Malaysia,
    /// As [`Method::Malaysia`] with Isha at 17°, as in Kelantan's official timetables.
    MalaysiaKelantan,
    /// Ministry of Awqaf (Jordan): Fajr 18°, Isha 18°, sunrise 7 minutes earlier and Maghrib 6
    /// later. Matches the ministry's Amman timetable; its lower-lying regions get sunrise and
    /// Maghrib a few minutes less cautious than these (`tests/official.rs`).
    Jordan,
}

impl Method {
    /// Every built-in method, in a stable order.
    pub const ALL: [Self; 15] = [
        Self::UmmAlQura,
        Self::MuslimWorldLeague,
        Self::Egyptian,
        Self::Karachi,
        Self::NorthAmerica,
        Self::Dubai,
        Self::Kuwait,
        Self::Qatar,
        Self::Singapore,
        Self::Turkey,
        Self::Tehran,
        Self::MoonsightingCommittee,
        Self::Malaysia,
        Self::MalaysiaKelantan,
        Self::Jordan,
    ];

    /// Stable machine identifier, e.g. `"umm_al_qura"`.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::UmmAlQura => "umm_al_qura",
            Self::MuslimWorldLeague => "muslim_world_league",
            Self::Egyptian => "egyptian",
            Self::Karachi => "karachi",
            Self::NorthAmerica => "north_america",
            Self::Dubai => "dubai",
            Self::Kuwait => "kuwait",
            Self::Qatar => "qatar",
            Self::Singapore => "singapore",
            Self::Turkey => "turkey",
            Self::Tehran => "tehran",
            Self::MoonsightingCommittee => "moonsighting_committee",
            Self::Malaysia => "malaysia",
            Self::MalaysiaKelantan => "malaysia_kelantan",
            Self::Jordan => "jordan",
        }
    }

    /// The full parameter set this method stands for.
    ///
    /// Minute adjustments match the published tables these authorities issue (the same values
    /// adhan-js uses).
    #[must_use]
    pub const fn parameters(self) -> Parameters {
        let angle = TwilightAngle::trusted;
        let base = Parameters {
            fajr: angle(18.0),
            isha: IshaRule::Angle(angle(17.0)),
            dhuhr: DhuhrRule::Transit,
            maghrib: MaghribRule::Sunset,
            asr: AsrShadow::Standard,
            high_latitude: HighLatitudeRule::Recommended,
            rounding: Rounding::Nearest,
            adjustments: Adjustments::NONE,
            ramadan: RamadanDates::Calculated,
        };
        let dhuhr_plus_one = Adjustments {
            dhuhr: MinuteAdjustment::trusted(1),
            ..Adjustments::NONE
        };
        match self {
            Self::UmmAlQura => umm_al_qura(base),
            Self::MuslimWorldLeague => Parameters {
                adjustments: dhuhr_plus_one,
                ..base
            },
            Self::Egyptian => Parameters {
                fajr: angle(19.5),
                isha: IshaRule::Angle(angle(17.5)),
                adjustments: dhuhr_plus_one,
                ..base
            },
            Self::Karachi => Parameters {
                isha: IshaRule::Angle(angle(18.0)),
                adjustments: dhuhr_plus_one,
                ..base
            },
            Self::NorthAmerica => Parameters {
                fajr: angle(15.0),
                isha: IshaRule::Angle(angle(15.0)),
                adjustments: dhuhr_plus_one,
                ..base
            },
            Self::Dubai => Parameters {
                fajr: angle(18.2),
                isha: IshaRule::Angle(angle(18.2)),
                adjustments: Adjustments {
                    sunrise: MinuteAdjustment::trusted(-3),
                    dhuhr: MinuteAdjustment::trusted(3),
                    asr: MinuteAdjustment::trusted(3),
                    maghrib: MinuteAdjustment::trusted(3),
                    ..Adjustments::NONE
                },
                ..base
            },
            Self::Kuwait => Parameters {
                isha: IshaRule::Angle(angle(17.5)),
                ..base
            },
            Self::Qatar => Parameters {
                isha: IshaRule::Interval {
                    regular: IshaInterval::trusted(90),
                    ramadan: IshaInterval::trusted(90),
                },
                ..base
            },
            Self::Singapore => Parameters {
                fajr: angle(20.0),
                isha: IshaRule::Angle(angle(18.0)),
                rounding: Rounding::Up,
                adjustments: dhuhr_plus_one,
                ..base
            },
            Self::Turkey => Parameters {
                adjustments: TURKEY_ADJUSTMENTS,
                ..base
            },
            Self::Tehran => Parameters {
                fajr: angle(17.7),
                isha: IshaRule::Angle(angle(14.0)),
                maghrib: MaghribRule::Angle(angle(4.5)),
                ..base
            },
            Self::MoonsightingCommittee => Parameters {
                isha: IshaRule::Angle(angle(18.0)),
                high_latitude: HighLatitudeRule::SeasonAdjusted(Shafaq::General),
                adjustments: MOONSIGHTING_COMMITTEE_ADJUSTMENTS,
                ..base
            },
            Self::Malaysia => malaysia(base, angle(18.0)),
            Self::MalaysiaKelantan => malaysia(base, angle(17.0)),
            Self::Jordan => Parameters {
                isha: IshaRule::Angle(angle(18.0)),
                adjustments: JORDAN_ADJUSTMENTS,
                ..base
            },
        }
    }
}

impl fmt::Display for Method {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.id())
    }
}

/// [`Method::from_str`] was given an unknown identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct UnknownMethod;

impl fmt::Display for UnknownMethod {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("unknown calculation method")
    }
}

impl core::error::Error for UnknownMethod {}

impl FromStr for Method {
    type Err = UnknownMethod;

    fn from_str(id: &str) -> Result<Self, Self::Err> {
        Self::ALL
            .into_iter()
            .find(|method| method.id() == id)
            .ok_or(UnknownMethod)
    }
}

/// How Isha is determined.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum IshaRule {
    /// When the sun reaches this depression angle after sunset.
    Angle(TwilightAngle),
    /// A fixed interval after Maghrib, with a separate interval during Ramadan.
    Interval {
        /// Outside Ramadan.
        regular: IshaInterval,
        /// On evenings that begin a day of Ramadan.
        ramadan: IshaInterval,
    },
}

/// When Dhuhr begins.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum DhuhrRule {
    /// At solar transit (the sun's centre on the meridian).
    #[default]
    Transit,
    /// Once the sun's whole disc has crossed the meridian, by the falak convention of a 16′
    /// semidiameter: 64 seconds after transit. Used in Malaysia.
    AfterTheLimb,
}

/// How Maghrib is determined.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MaghribRule {
    /// At sunset.
    Sunset,
    /// When the sun reaches this depression angle (Jaʿfari).
    Angle(TwilightAngle),
}

/// The shadow-length convention for Asr.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum AsrShadow {
    /// Shadow equals object length plus noon shadow (Shafiʿi, Maliki, Hanbali).
    #[default]
    Standard,
    /// Shadow equals twice object length plus noon shadow (Hanafi).
    Hanafi,
}

impl AsrShadow {
    pub(crate) const fn factor(self) -> f64 {
        match self {
            Self::Standard => 1.0,
            Self::Hanafi => 2.0,
        }
    }
}

/// Fallback for Fajr and Isha where twilight never ends (summer above ~48°).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum HighLatitudeRule {
    /// [`HighLatitudeRule::SeventhOfTheNight`] above 48° latitude,
    /// [`HighLatitudeRule::MiddleOfTheNight`] elsewhere.
    #[default]
    Recommended,
    /// Fajr no earlier, and Isha no later, than the middle of the night.
    MiddleOfTheNight,
    /// Fajr no earlier than the last seventh, Isha no later than the first seventh of the night.
    SeventhOfTheNight,
    /// Bound each by `angle / 60` of the night.
    TwilightAngle,
    /// No fallback: returns [`crate::ComputeError::Undefined`] when twilight never ends.
    None,
    /// Moonsighting Committee Worldwide: twilight is never longer than a season- and
    /// latitude-adjusted number of minutes (for Isha, of the chosen [`Shafaq`]), and above 55°
    /// it is a seventh of the night. Unlike the other rules this also shortens twilight at
    /// mid latitudes in summer.
    SeasonAdjusted(Shafaq),
}

/// Which glow in the western sky ends Isha under [`HighLatitudeRule::SeasonAdjusted`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Shafaq {
    /// The committee's blend of red and white, its default.
    #[default]
    General,
    /// The red glow (*shafaq ahmar*): Shafiʿi, Maliki and Hanbali. The earliest Isha.
    Red,
    /// The white glow (*shafaq abyad*): Hanafi. The latest Isha.
    White,
}

/// How fractional minutes are rounded.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Rounding {
    /// To the nearest minute.
    #[default]
    Nearest,
    /// Always up to the next whole minute.
    Up,
}

/// Per-prayer minute corrections, applied after the astronomical calculation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Adjustments {
    /// Fajr correction.
    pub fajr: MinuteAdjustment,
    /// Sunrise correction.
    pub sunrise: MinuteAdjustment,
    /// Dhuhr correction.
    pub dhuhr: MinuteAdjustment,
    /// Asr correction.
    pub asr: MinuteAdjustment,
    /// Maghrib correction.
    pub maghrib: MinuteAdjustment,
    /// Isha correction.
    pub isha: MinuteAdjustment,
}

impl Adjustments {
    /// No corrections.
    pub const NONE: Self = Self {
        fajr: MinuteAdjustment::trusted(0),
        sunrise: MinuteAdjustment::trusted(0),
        dhuhr: MinuteAdjustment::trusted(0),
        asr: MinuteAdjustment::trusted(0),
        maghrib: MinuteAdjustment::trusted(0),
        isha: MinuteAdjustment::trusted(0),
    };
}

/// Everything the calculation depends on besides date and place.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Parameters {
    /// Sun depression at Fajr.
    pub fajr: TwilightAngle,
    /// Isha rule.
    pub isha: IshaRule,
    /// Dhuhr rule.
    pub dhuhr: DhuhrRule,
    /// Maghrib rule.
    pub maghrib: MaghribRule,
    /// Asr shadow convention.
    pub asr: AsrShadow,
    /// High-latitude fallback.
    pub high_latitude: HighLatitudeRule,
    /// Minute rounding.
    pub rounding: Rounding,
    /// Per-prayer corrections.
    pub adjustments: Adjustments,
    /// Which days count as Ramadan, for methods with a Ramadan Isha interval.
    pub ramadan: RamadanDates,
}

impl Default for Parameters {
    /// [`Method::UmmAlQura`].
    fn default() -> Self {
        Method::default().parameters()
    }
}

impl From<Method> for Parameters {
    fn from(method: Method) -> Self {
        method.parameters()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_umm_al_qura() {
        assert_eq!(Method::default(), Method::UmmAlQura, "default method");
        assert_eq!(
            Parameters::default(),
            Method::UmmAlQura.parameters(),
            "default params"
        );
    }

    #[test]
    fn ids_round_trip() {
        for method in Method::ALL {
            assert_eq!(method.id().parse(), Ok(method), "{method}");
        }
        assert_eq!("nope".parse::<Method>(), Err(UnknownMethod), "unknown id");
    }

    /// Every `trusted` constant in the method table must pass its own validator.
    #[test]
    fn built_in_parameters_are_valid() {
        for method in Method::ALL {
            let p = method.parameters();
            assert!(TwilightAngle::new(p.fajr.get()).is_ok(), "{method} fajr");
            match p.isha {
                IshaRule::Angle(a) => assert!(TwilightAngle::new(a.get()).is_ok(), "{method} isha"),
                IshaRule::Interval { regular, ramadan } => {
                    assert!(
                        IshaInterval::new(regular.get()).is_ok(),
                        "{method} isha regular"
                    );
                    assert!(
                        IshaInterval::new(ramadan.get()).is_ok(),
                        "{method} isha ramadan"
                    );
                }
            }
            if let MaghribRule::Angle(a) = p.maghrib {
                assert!(TwilightAngle::new(a.get()).is_ok(), "{method} maghrib");
            }
            let a = p.adjustments;
            for m in [a.fajr, a.sunrise, a.dhuhr, a.asr, a.maghrib, a.isha] {
                assert!(
                    MinuteAdjustment::new(m.get()).is_ok(),
                    "{method} adjustment"
                );
            }
        }
    }
}
