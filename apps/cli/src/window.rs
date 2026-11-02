//! The active prayer window: how long after its own adhan a prayer is still shown as current.

use hya_engine::Prayer;

/// The window length, in minutes, for `prayer`: the adhan→iqama gap plus a buffer for praying
/// it, chosen from the baseline or Ramadan table. `None` for [`Prayer::Sunrise`], which is
/// never active.
pub(crate) const fn window_minutes(prayer: Prayer, ramadan: bool) -> Option<u32> {
    match (prayer, ramadan) {
        (Prayer::Sunrise, _) => None,
        (Prayer::Fajr, false) => Some(40),
        (Prayer::Fajr, true) => Some(30),
        (Prayer::Dhuhr, _) => Some(30),
        (Prayer::Asr, _) => Some(30),
        (Prayer::Maghrib, false) => Some(20),
        (Prayer::Maghrib, true) => Some(25),
        (Prayer::Isha, false) => Some(35),
        (Prayer::Isha, true) => Some(30),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn baseline_minutes_match_the_table() {
        assert_eq!(window_minutes(Prayer::Fajr, false), Some(40));
        assert_eq!(window_minutes(Prayer::Dhuhr, false), Some(30));
        assert_eq!(window_minutes(Prayer::Asr, false), Some(30));
        assert_eq!(window_minutes(Prayer::Maghrib, false), Some(20));
        assert_eq!(window_minutes(Prayer::Isha, false), Some(35));
    }

    #[test]
    fn ramadan_minutes_match_the_table() {
        assert_eq!(window_minutes(Prayer::Fajr, true), Some(30));
        assert_eq!(window_minutes(Prayer::Dhuhr, true), Some(30));
        assert_eq!(window_minutes(Prayer::Asr, true), Some(30));
        assert_eq!(window_minutes(Prayer::Maghrib, true), Some(25));
        assert_eq!(window_minutes(Prayer::Isha, true), Some(30));
    }

    #[test]
    fn sunrise_is_never_active() {
        assert_eq!(window_minutes(Prayer::Sunrise, false), None);
        assert_eq!(window_minutes(Prayer::Sunrise, true), None);
    }
}
