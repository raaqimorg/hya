//! Kani proof harnesses: bounded model checking over *every* possible input, not samples.
//!
//! Run with `cargo kani -p hya-engine`. Each harness is a theorem; Kani either proves it for
//! all values of the symbolic inputs or produces a counterexample.

use crate::date::Instant;
use crate::date::{Date, civil_from_days, days_from_civil};
use crate::input::{InvalidInput, IshaInterval, Latitude, Longitude, MinuteAdjustment};
use crate::method::Method;
use crate::ramadan::{self, AnnouncedRamadan, RamadanDates};
use crate::seasonal;

/// `Latitude::new` accepts exactly the finite values in `-90..=90`.
#[kani::proof]
fn latitude_accepts_exactly_its_range() {
    let value: f64 = kani::any();
    let accepted = Latitude::new(value).is_ok();
    assert_eq!(
        accepted,
        value.is_finite() && (-90.0..=90.0).contains(&value)
    );
    if let Ok(latitude) = Latitude::new(value) {
        assert!(
            latitude.get().to_bits() == value.to_bits(),
            "stores the value unchanged"
        );
    }
}

/// `Longitude::new` accepts exactly the finite values in `-180..=180`.
#[kani::proof]
fn longitude_accepts_exactly_its_range() {
    let value: f64 = kani::any();
    assert_eq!(
        Longitude::new(value).is_ok(),
        value.is_finite() && (-180.0..=180.0).contains(&value)
    );
}

/// Integer bounded types accept exactly their documented ranges.
#[kani::proof]
fn bounded_integers_accept_exactly_their_ranges() {
    let minutes: u16 = kani::any();
    assert_eq!(
        IshaInterval::new(minutes).is_ok(),
        (1..=240).contains(&minutes)
    );
    let adjustment: i16 = kani::any();
    assert_eq!(
        MinuteAdjustment::new(adjustment).is_ok(),
        (-120..=120).contains(&adjustment)
    );
}

/// For every year in range and every month/day byte, date construction never panics, and a
/// constructed date survives the round trip through its day count.
#[kani::proof]
#[kani::unwind(2)]
fn date_round_trips_through_day_count() {
    let year: i16 = kani::any();
    let month: u8 = kani::any();
    let day: u8 = kani::any();
    kani::assume((Date::MIN_YEAR..=Date::MAX_YEAR).contains(&year));
    if let Ok(date) = Date::new(year, month, day) {
        let days = date.days_since_unix_epoch();
        assert_eq!(civil_from_days(days), Some((year, month, day)));
        assert_eq!(Date::from_days_since_unix_epoch(days), Ok(date));
    } else {
        assert!(month == 0 || month > 12 || day == 0 || day > 28);
    }
}

/// `days_from_civil` never overflows for any input at all (it returns `None` instead).
#[kani::proof]
fn days_from_civil_is_total() {
    let _ = days_from_civil(kani::any(), kani::any(), kani::any());
}

/// `civil_from_days` never overflows for any `i32` (it returns `None` instead).
#[kani::proof]
fn civil_from_days_is_total() {
    let _ = civil_from_days(kani::any());
}

/// Out-of-range years are always rejected with `DateOutOfRange`.
#[kani::proof]
fn out_of_range_years_are_rejected() {
    let year: i16 = kani::any();
    kani::assume(!(Date::MIN_YEAR..=Date::MAX_YEAR).contains(&year));
    assert_eq!(
        Date::new(year, kani::any(), kani::any()),
        Err(InvalidInput::DateOutOfRange)
    );
}

/// The Ramadan lookup is total over every possible day index.
#[kani::proof]
#[kani::unwind(10)]
fn ramadan_lookup_is_total() {
    let _ = ramadan::contains_day(kani::any());
}

/// `AnnouncedRamadan::new` never panics, accepts only 29- or 30-day months, and the lookup it
/// builds is total.
#[kani::proof]
#[kani::unwind(10)]
fn announced_ramadan_accepts_only_whole_months() {
    let (Ok(first), Ok(last)) = (
        Date::from_days_since_unix_epoch(kani::any()),
        Date::from_days_since_unix_epoch(kani::any()),
    ) else {
        return;
    };
    if let Ok(announced) = AnnouncedRamadan::new(first, last) {
        let length = last.days_since_unix_epoch() - first.days_since_unix_epoch() + 1;
        assert!(length == 29 || length == 30);
        let _ = RamadanDates::Announced(announced).contains_day(kani::any());
    }
}

/// Instant conversions never overflow for instants `compute` can produce.
#[kani::proof]
fn instant_conversions_are_exact_in_range() {
    let minutes: i64 = kani::any();
    // 1899-12-24 .. 2101-01-08 in minutes, the widest span `compute` can emit.
    kani::assume((-36_828_000..=68_920_000).contains(&minutes));
    let instant = Instant::from_unix_minutes(minutes);
    assert_eq!(instant.unix_seconds(), minutes * 60);
    assert_eq!(instant.unix_millis(), minutes * 60_000);
}

/// Every method identifier parses back to its method.
///
/// The bound covers both loops: 15 methods, and byte-wise comparison of the longest id
/// (`moonsighting_committee`, 22 bytes), plus one for the exit check.
#[kani::proof]
#[kani::unwind(24)]
fn method_ids_round_trip() {
    let index: usize = kani::any();
    kani::assume(index < Method::ALL.len());
    let method = Method::ALL[index];
    assert_eq!(method.id().parse::<Method>(), Ok(method));
}

/// The Moonsighting Committee day count is defined and stays on its 365/366-day cycle for every
/// valid date in either hemisphere.
#[kani::proof]
fn days_since_solstice_is_always_in_cycle() {
    let days: i32 = kani::any();
    kani::assume((-25_567..=47_846).contains(&days));
    let date = Date::from_days_since_unix_epoch(days);
    assert!(date.is_ok());
    if let Ok(date) = date {
        let north: bool = kani::any();
        let latitude = if north { 51.5 } else { -33.9 };
        let offset = seasonal::days_since_solstice(date, latitude);
        assert!(offset.is_some_and(|d| (0..366).contains(&d)));
    }
}
