//! Coverage-guided fuzzing of the whole public surface: arbitrary bytes become arbitrary
//! (often invalid) inputs. The only acceptable outcomes are `Ok` with ordered times or a typed
//! `Err`; any panic, overflow (overflow-checks are on) or ordering violation is a finding.

#![no_main]

use hya_engine::{
    Adjustments, AnnouncedRamadan, AsrShadow, Coordinates, Date, DhuhrRule, Elevation,
    HighLatitudeRule, IshaInterval, IshaRule, MaghribRule, Method, MinuteAdjustment, Parameters,
    Prayer, RamadanDates, Rounding, Shafaq, TwilightAngle, compute,
};
use libfuzzer_sys::fuzz_target;

fn f64_from(bytes: &[u8]) -> f64 {
    let mut buf = [0_u8; 8];
    for (slot, byte) in buf.iter_mut().zip(bytes) {
        *slot = *byte;
    }
    f64::from_le_bytes(buf)
}

fuzz_target!(|data: &[u8]| {
    if data.len() < 43 {
        return;
    }
    let year = i16::from_le_bytes([data[0], data[1]]);
    let Ok(date) = Date::new(year, data[2], data[3]) else {
        return;
    };
    let Ok(coords) = Coordinates::new(f64_from(&data[4..12]), f64_from(&data[12..20])) else {
        return;
    };
    let coords = match Elevation::new(f64_from(&data[20..28])) {
        Ok(elevation) => coords.with_elevation(elevation),
        Err(_) => coords,
    };
    let method = Method::ALL[usize::from(data[28]) % Method::ALL.len()];
    let rules = [
        HighLatitudeRule::Recommended,
        HighLatitudeRule::MiddleOfTheNight,
        HighLatitudeRule::SeventhOfTheNight,
        HighLatitudeRule::TwilightAngle,
        HighLatitudeRule::None,
        HighLatitudeRule::SeasonAdjusted(Shafaq::General),
        HighLatitudeRule::SeasonAdjusted(Shafaq::Red),
        HighLatitudeRule::SeasonAdjusted(Shafaq::White),
    ];
    let mut params = Parameters {
        high_latitude: rules[usize::from(data[29]) % rules.len()],
        ..method.parameters()
    };
    if let Ok(angle) = TwilightAngle::new(f64_from(&data[30..38])) {
        params.fajr = angle;
    }
    // An announced Ramadan starting up to ±32k days from `date`, 0..=63 days long: mostly
    // rejected, which exercises the constructor; the accepted ones exercise the lookup.
    let start = date
        .days_since_unix_epoch()
        .saturating_add(i32::from(i16::from_le_bytes([data[38], data[39]])));
    let end = start.saturating_add(i32::from(data[40] % 64));
    if let (Ok(first), Ok(last)) = (
        Date::from_days_since_unix_epoch(start),
        Date::from_days_since_unix_epoch(end),
    ) && let Ok(announced) = AnnouncedRamadan::new(first, last)
    {
        params.ramadan = RamadanDates::Announced(announced);
    }
    // Every other parameter, from an optional tail, so short inputs keep their meaning.
    if let Some(tail) = data.get(43..70) {
        let flags = tail[0];
        if flags & 1 != 0 {
            params.rounding = Rounding::Up;
        }
        if flags & 2 != 0 {
            params.asr = AsrShadow::Hanafi;
        }
        if flags & 4 != 0 {
            params.dhuhr = DhuhrRule::AfterTheLimb;
        }
        if flags & 8 != 0 {
            let regular = IshaInterval::new(u16::from_le_bytes([tail[9], tail[10]]));
            let ramadan = IshaInterval::new(u16::from_le_bytes([tail[11], tail[12]]));
            if let (Ok(regular), Ok(ramadan)) = (regular, ramadan) {
                params.isha = IshaRule::Interval { regular, ramadan };
            }
        } else if let Ok(angle) = TwilightAngle::new(f64_from(&tail[1..9])) {
            params.isha = IshaRule::Angle(angle);
        }
        if flags & 16 != 0
            && let Ok(angle) = TwilightAngle::new(f64_from(&tail[13..21]))
        {
            params.maghrib = MaghribRule::Angle(angle);
        }
        if flags & 32 != 0 {
            let minutes = |byte: u8| MinuteAdjustment::new(i16::from(i8::from_le_bytes([byte])));
            if let (Ok(fajr), Ok(sunrise), Ok(dhuhr), Ok(asr), Ok(maghrib), Ok(isha)) = (
                minutes(tail[21]),
                minutes(tail[22]),
                minutes(tail[23]),
                minutes(tail[24]),
                minutes(tail[25]),
                minutes(tail[26]),
            ) {
                params.adjustments = Adjustments {
                    fajr,
                    sunrise,
                    dhuhr,
                    asr,
                    maghrib,
                    isha,
                };
            }
        }
    }
    if let Ok(times) = compute(date, coords, params) {
        let mut previous = None;
        for prayer in Prayer::ALL {
            let at = times.get(prayer);
            assert!(previous < Some(at), "out of order at {prayer:?}");
            previous = Some(at);
        }
        assert_eq!(
            times.midnight().is_some(),
            times.last_third().is_some(),
            "one night mark without the other"
        );
        if let (Some(midnight), Some(last_third)) = (times.midnight(), times.last_third()) {
            assert!(times.maghrib() < midnight, "midnight before maghrib");
            assert!(midnight < last_third, "last third before midnight");
        }
    }
});
