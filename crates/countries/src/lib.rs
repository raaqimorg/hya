//! Which prayer-time method a country customarily uses.
//!
//! Kept apart from `hya-engine` so the calculation stays free of lookup tables that go stale.
//! The table is deliberately short: a country is listed only when one of the core's methods is
//! its own national authority's. Everywhere else the custom varies by community or mosque (the
//! UK and North America are split between several), or the authority's method is not one the
//! core implements, so [`entry_for`] returns `None` and the caller must choose.

#![no_std]

#[cfg(kani)]
mod verification;

use core::fmt;
use core::str::FromStr;

use hya_engine::Method;

/// An ISO 3166-1 alpha-2 country code, such as `SA`.
///
/// Only constructible through [`CountryCode::new`] (or [`FromStr`]), which accepts exactly two
/// ASCII letters in either case and stores them uppercase. It checks the shape, not that the
/// code is assigned: an unassigned code is simply not in the table.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CountryCode([u8; 2]);

/// The text is not two ASCII letters.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InvalidCountryCode;

impl fmt::Display for InvalidCountryCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("a country code is two letters (ISO 3166-1 alpha-2), e.g. SA")
    }
}

impl core::error::Error for InvalidCountryCode {}

impl CountryCode {
    /// Parses a two-letter code, ignoring case.
    ///
    /// # Errors
    ///
    /// [`InvalidCountryCode`] unless `code` is exactly two ASCII letters.
    pub const fn new(code: &str) -> Result<Self, InvalidCountryCode> {
        Self::from_bytes(code.as_bytes())
    }

    /// As [`CountryCode::new`], over raw bytes (anything not ASCII is rejected).
    const fn from_bytes(code: &[u8]) -> Result<Self, InvalidCountryCode> {
        match code {
            [first, second] if first.is_ascii_alphabetic() && second.is_ascii_alphabetic() => {
                Ok(Self([
                    first.to_ascii_uppercase(),
                    second.to_ascii_uppercase(),
                ]))
            }
            _ => Err(InvalidCountryCode),
        }
    }

    /// The two uppercase ASCII letters.
    #[must_use]
    pub const fn letters(self) -> [u8; 2] {
        self.0
    }
}

impl FromStr for CountryCode {
    type Err = InvalidCountryCode;

    fn from_str(code: &str) -> Result<Self, Self::Err> {
        Self::new(code)
    }
}

impl fmt::Display for CountryCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let [first, second] = self.0;
        write!(f, "{}{}", char::from(first), char::from(second))
    }
}

/// A country whose national authority's method is in the core.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Entry {
    /// The country.
    pub country: CountryCode,
    /// Its method.
    pub method: Method,
    /// The authority that issues the country's official times.
    pub authority: &'static str,
}

const fn entry(letters: [u8; 2], method: Method, authority: &'static str) -> Entry {
    Entry {
        country: CountryCode(letters),
        method,
        authority,
    }
}

/// Every listed country, sorted by code.
///
/// Malaysia's entry is the federal JAKIM method; Kelantan's timetables use
/// [`Method::MalaysiaKelantan`], which a caller must choose by state.
pub const ENTRIES: [Entry; 11] = [
    entry(
        *b"AE",
        Method::Dubai,
        "General Authority of Islamic Affairs and Endowments",
    ),
    entry(
        *b"EG",
        Method::Egyptian,
        "Egyptian General Authority of Survey",
    ),
    entry(
        *b"IR",
        Method::Tehran,
        "Institute of Geophysics, University of Tehran",
    ),
    entry(
        *b"JO",
        Method::Jordan,
        "Ministry of Awqaf, Islamic Affairs and Holy Places",
    ),
    entry(
        *b"KW",
        Method::Kuwait,
        "Ministry of Awqaf and Islamic Affairs",
    ),
    entry(
        *b"MY",
        Method::Malaysia,
        "JAKIM (Department of Islamic Development Malaysia)",
    ),
    entry(
        *b"PK",
        Method::Karachi,
        "University of Islamic Sciences, Karachi",
    ),
    entry(
        *b"QA",
        Method::Qatar,
        "Ministry of Endowments and Islamic Affairs",
    ),
    entry(*b"SA", Method::UmmAlQura, "Umm al-Qura University, Makkah"),
    entry(
        *b"SG",
        Method::Singapore,
        "Majlis Ugama Islam Singapura (MUIS)",
    ),
    entry(*b"TR", Method::Turkey, "Diyanet İşleri Başkanlığı"),
];

/// The listed entry for `country`, if any.
#[must_use]
pub fn entry_for(country: CountryCode) -> Option<Entry> {
    ENTRIES
        .binary_search_by_key(&country, |entry| entry.country)
        .ok()
        .and_then(|index| ENTRIES.get(index))
        .copied()
}
