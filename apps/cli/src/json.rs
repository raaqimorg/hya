//! The `--json` snapshot: today's six prayer windows, tonight's midnight and last third,
//! yesterday's Isha window (for the midnight carry-over), tomorrow's Fajr, and the
//! already-resolved active/next prayer — all in Unix seconds, so a caller never has to redo
//! any of `hya`'s own math.

use hya_engine::{Prayer, PrayerTimes};
use serde::Serialize;

use crate::window;
use crate::{Next, prayer_name};

#[derive(Debug, Serialize)]
pub(crate) struct Window {
    pub(crate) at: i64,
    pub(crate) ends: Option<i64>,
}

#[derive(Debug, Serialize)]
pub(crate) struct Today {
    pub(crate) fajr: Window,
    pub(crate) sunrise: Window,
    pub(crate) dhuhr: Window,
    pub(crate) asr: Window,
    pub(crate) maghrib: Window,
    pub(crate) isha: Window,
}

#[derive(Debug, Serialize)]
pub(crate) struct ActiveJson {
    pub(crate) prayer: &'static str,
    pub(crate) at: i64,
    pub(crate) ends: i64,
}

#[derive(Debug, Serialize)]
pub(crate) struct NextJson {
    pub(crate) prayer: &'static str,
    pub(crate) at: i64,
    pub(crate) tomorrow: bool,
}

#[derive(Debug, Serialize)]
pub(crate) struct Snapshot {
    pub(crate) generated_at: i64,
    pub(crate) active: Option<ActiveJson>,
    pub(crate) next: Option<NextJson>,
    pub(crate) today: Today,
    pub(crate) midnight: Option<i64>,
    pub(crate) last_third: Option<i64>,
    pub(crate) yesterday_isha: Option<Window>,
    pub(crate) tomorrow_fajr: Option<i64>,
}

/// Builds the `--json` snapshot from the pieces `run()` already computes: today's resolved
/// `active`/`next` prayer, today's own `times`/`ramadan_today`, tonight's midnight and last
/// third (from `night_marks_on`), yesterday's schedule (for Isha's midnight carry-over), and
/// tomorrow's Fajr instant.
#[expect(
    clippy::too_many_arguments,
    reason = "every value `write_json` already has in scope"
)]
pub(crate) fn build(
    now: i64,
    times: &PrayerTimes,
    ramadan_today: bool,
    active: Option<window::Active>,
    next: Option<Next>,
    yesterday: Option<(PrayerTimes, bool)>,
    tomorrow_fajr: Option<i64>,
    midnight: Option<i64>,
    last_third: Option<i64>,
) -> Snapshot {
    let to_window = |(_, at, ends): (Prayer, i64, Option<i64>)| Window { at, ends };
    let [fajr, sunrise, dhuhr, asr, maghrib, isha] = window::windows_for(times, ramadan_today);
    let today = Today {
        fajr: to_window(fajr),
        sunrise: to_window(sunrise),
        dhuhr: to_window(dhuhr),
        asr: to_window(asr),
        maghrib: to_window(maghrib),
        isha: to_window(isha),
    };
    let yesterday_isha = yesterday.map(|(yesterday_times, yesterday_ramadan)| {
        let [.., yesterdays_isha] = window::windows_for(&yesterday_times, yesterday_ramadan);
        to_window(yesterdays_isha)
    });
    let active = active.map(|active| ActiveJson {
        prayer: prayer_name(active.prayer),
        at: active.at,
        ends: active.ends,
    });
    let next = next.map(|next| NextJson {
        prayer: prayer_name(next.prayer),
        at: next.at,
        tomorrow: next.tomorrow,
    });
    Snapshot {
        generated_at: now,
        active,
        next,
        today,
        midnight,
        last_third,
        yesterday_isha,
        tomorrow_fajr,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hya_engine::{Coordinates, Date, Parameters, compute};

    fn riyadh() -> PrayerTimes {
        compute(
            Date::new(2026, 10, 29).expect("valid"),
            Coordinates::new(24.7136, 46.6753).expect("valid"),
            Parameters::default(),
        )
        .expect("computable")
    }

    #[test]
    fn today_sunrise_has_no_ends_every_other_prayer_does() {
        let snapshot = build(
            riyadh().get(Prayer::Isha).unix_seconds(),
            &riyadh(),
            false,
            None,
            None,
            None,
            None,
            None,
            None,
        );
        assert_eq!(snapshot.today.sunrise.ends, None);
        assert!(snapshot.today.fajr.ends.is_some());
        assert!(snapshot.today.isha.ends.is_some());
    }

    #[test]
    fn serializes_to_the_documented_shape() {
        let snapshot = build(
            1_000_000_000,
            &riyadh(),
            false,
            None,
            None,
            None,
            Some(1_000_050_000),
            Some(1_000_010_000),
            Some(1_000_020_000),
        );
        let value: serde_json::Value = serde_json::to_value(&snapshot).expect("serializable");
        assert_eq!(value["tomorrow_fajr"], 1_000_050_000_i64);
        assert_eq!(value["midnight"], 1_000_010_000_i64);
        assert_eq!(value["last_third"], 1_000_020_000_i64);
        assert!(value["today"]["fajr"]["ends"].is_number());
        assert_eq!(value["today"]["sunrise"]["ends"], serde_json::Value::Null);
    }

    #[test]
    fn midnight_and_last_third_are_null_when_absent() {
        let snapshot = build(
            1_000_000_000,
            &riyadh(),
            false,
            None,
            None,
            None,
            None,
            None,
            None,
        );
        let value: serde_json::Value = serde_json::to_value(&snapshot).expect("serializable");
        assert_eq!(value["midnight"], serde_json::Value::Null);
        assert_eq!(value["last_third"], serde_json::Value::Null);
    }
}
