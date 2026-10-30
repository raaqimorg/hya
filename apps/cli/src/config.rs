//! The CLI's saved settings, persisted as a tiny `key = value` file.
//!
//! This is the only stateful component in the project, so it is the one covered by the
//! state-machine test at the end of this file.

use core::fmt;

use hya_engine::{
    AnnouncedRamadan, AsrShadow, Coordinates, Date, InvalidInput, Method, RamadanDates, Shafaq,
};

/// Saved user settings.
#[derive(Debug, Clone, PartialEq, Default)]
pub(crate) struct Config {
    /// Where the user is, once they have told us.
    pub(crate) location: Option<Coordinates>,
    /// Calculation method.
    pub(crate) method: Method,
    /// Asr convention.
    pub(crate) asr: AsrShadow,
    /// Which twilight glow ends Isha, for methods using the season-adjusted rule.
    pub(crate) shafaq: Shafaq,
    /// Announced Ramadan dates.
    pub(crate) ramadan: Announcements,
    /// How times are printed.
    pub(crate) clock: Clock,
}

/// How times are printed: `18:45` or `6:45 PM`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum Clock {
    /// `18:45`.
    TwentyFourHour,
    /// `6:45 PM`.
    #[default]
    TwelveHour,
}

impl Clock {
    /// The `strftime` pattern, padded so the minutes line up in a column.
    pub(crate) const fn pattern(self) -> &'static str {
        match self {
            Self::TwentyFourHour => "%H:%M",
            Self::TwelveHour => "%_I:%M %p",
        }
    }
}

/// Announced Ramadans: at most one per month, sorted by start.
#[derive(Debug, Clone, PartialEq, Default)]
pub(crate) struct Announcements(Vec<AnnouncedRamadan>);

impl Announcements {
    /// Records an announcement, replacing any earlier one for the same month.
    pub(crate) fn announce(&mut self, announced: AnnouncedRamadan) {
        self.0.retain(|r| !r.same_month_as(announced));
        self.0.push(announced);
        self.0.sort_by_key(|r| r.first());
    }

    /// The Ramadan dates to use for `date`: the announcement nearest to it, if any. An
    /// announcement only affects its own month, so any other year falls back to the calendar.
    #[must_use]
    pub(crate) fn for_date(&self, date: Date) -> RamadanDates {
        let day = date.days_since_unix_epoch();
        self.0
            .iter()
            .min_by_key(|r| r.first().days_since_unix_epoch().abs_diff(day))
            .map_or(RamadanDates::Calculated, |r| RamadanDates::Announced(*r))
    }
}

/// Why a config file could not be read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ConfigError {
    /// A line was not `key = value`.
    Malformed {
        /// 1-based line number.
        line: usize,
    },
    /// A key appeared twice.
    DuplicateKey(String),
    /// A value failed validation.
    InvalidValue(String),
    /// Only one of latitude/longitude was present.
    IncompleteLocation,
}

/// Every key this version reads.
const KEYS: [&str; 7] = [
    "latitude",
    "longitude",
    "method",
    "asr",
    "shafaq",
    "clock",
    "ramadan",
];

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Malformed { line } => write!(f, "line {line}: expected `key = value`"),
            Self::DuplicateKey(key) => write!(f, "setting `{key}` appears twice"),
            Self::InvalidValue(key) => write!(f, "invalid value for `{key}`"),
            Self::IncompleteLocation => f.write_str("latitude and longitude must be set together"),
        }
    }
}

impl core::error::Error for ConfigError {}

const fn asr_id(asr: AsrShadow) -> &'static str {
    match asr {
        AsrShadow::Standard => "standard",
        AsrShadow::Hanafi => "hanafi",
    }
}

/// Parses an Asr convention identifier.
///
/// # Errors
///
/// Returns the input back if it is neither `standard` nor `hanafi`.
pub(crate) fn parse_asr(id: &str) -> Result<AsrShadow, String> {
    match id {
        "standard" => Ok(AsrShadow::Standard),
        "hanafi" => Ok(AsrShadow::Hanafi),
        other => Err(other.to_owned()),
    }
}

const fn shafaq_id(shafaq: Shafaq) -> &'static str {
    match shafaq {
        Shafaq::General => "general",
        Shafaq::Red => "red",
        Shafaq::White => "white",
    }
}

const fn clock_id(clock: Clock) -> &'static str {
    match clock {
        Clock::TwentyFourHour => "24",
        Clock::TwelveHour => "12",
    }
}

/// Parses a clock identifier.
///
/// # Errors
///
/// Returns the input back if it is neither `12` nor `24`.
pub(crate) fn parse_clock(id: &str) -> Result<Clock, String> {
    match id {
        "24" => Ok(Clock::TwentyFourHour),
        "12" => Ok(Clock::TwelveHour),
        other => Err(other.to_owned()),
    }
}

/// Parses a shafaq identifier.
///
/// # Errors
///
/// Returns the input back if it is not `general`, `red` or `white`.
pub(crate) fn parse_shafaq(id: &str) -> Result<Shafaq, String> {
    match id {
        "general" => Ok(Shafaq::General),
        "red" => Ok(Shafaq::Red),
        "white" => Ok(Shafaq::White),
        other => Err(other.to_owned()),
    }
}

fn parse_date(text: &str) -> Option<Date> {
    let mut parts = text.split('-');
    let (Some(year), Some(month), Some(day), None) =
        (parts.next(), parts.next(), parts.next(), parts.next())
    else {
        return None;
    };
    Date::new(year.parse().ok()?, month.parse().ok()?, day.parse().ok()?).ok()
}

/// Parses an announced Ramadan written `YYYY-MM-DD..YYYY-MM-DD` (first and last day).
///
/// # Errors
///
/// A human-readable message if the dates are malformed or not a plausible Ramadan.
pub(crate) fn parse_ramadan(text: &str) -> Result<AnnouncedRamadan, String> {
    let (first, last) = text
        .split_once("..")
        .and_then(|(a, b)| Some((parse_date(a.trim())?, parse_date(b.trim())?)))
        .ok_or("expected YYYY-MM-DD..YYYY-MM-DD (first and last day of Ramadan)")?;
    AnnouncedRamadan::new(first, last).map_err(|e| e.to_string())
}

/// Parses a typed-in location: two numbers separated by spaces and/or a comma.
///
/// # Errors
///
/// A human-readable message if the line is not exactly two numbers or they are out of range.
pub(crate) fn parse_location(line: &str) -> Result<Coordinates, String> {
    let mut numbers = line
        .split(|c: char| c.is_whitespace() || c == ',')
        .filter(|s| !s.is_empty())
        .map(str::parse::<f64>);
    match (numbers.next(), numbers.next(), numbers.next()) {
        (Some(Ok(lat)), Some(Ok(lng)), None) => {
            Coordinates::new(lat, lng).map_err(|e| e.to_string())
        }
        _ => Err("expected two numbers: latitude longitude".to_owned()),
    }
}

impl Config {
    /// Serialises to the on-disk format, with only the settings that differ from the default,
    /// so a later change of default reaches everyone who never chose otherwise. Floats use
    /// Rust's shortest round-trip formatting, so `parse(render(c)) == c` exactly.
    #[must_use]
    pub(crate) fn render(&self) -> String {
        let default = Self::default();
        let mut out = self.location.map_or_else(String::new, |location| {
            format!(
                "latitude = {:?}\nlongitude = {:?}\n",
                location.latitude.get(),
                location.longitude.get()
            )
        });
        let mut set = |key: &str, value: &str, is_default: bool| {
            if !is_default {
                out.push_str(key);
                out.push_str(" = ");
                out.push_str(value);
                out.push('\n');
            }
        };
        set("method", self.method.id(), self.method == default.method);
        set("asr", asr_id(self.asr), self.asr == default.asr);
        set(
            "shafaq",
            shafaq_id(self.shafaq),
            self.shafaq == default.shafaq,
        );
        set("clock", clock_id(self.clock), self.clock == default.clock);
        for r in &self.ramadan.0 {
            set("ramadan", &format!("{}..{}", r.first(), r.last()), false);
        }
        out
    }

    /// Parses the on-disk format, with the keys it does not know. Blank lines and `#` comments
    /// are ignored. An unknown key is skipped, so a file written by a newer `hya` still loads;
    /// everything else is strict. `ramadan` is the one key that may repeat, once per month.
    ///
    /// # Errors
    ///
    /// Any [`ConfigError`].
    pub(crate) fn parse(text: &str) -> Result<(Self, Vec<String>), ConfigError> {
        let mut config = Self::default();
        let mut unknown = Vec::new();
        let (mut latitude, mut longitude) = (None, None);
        let mut seen: Vec<&str> = Vec::new();
        // Windows editors may save a byte-order mark, which `trim` keeps.
        let text = text.strip_prefix('\u{feff}').unwrap_or(text);
        for (index, raw) in text.lines().enumerate() {
            let line = raw.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let line_number = index.saturating_add(1);
            let (key, value) = line
                .split_once('=')
                .map(|(k, v)| (k.trim(), v.trim()))
                .ok_or(ConfigError::Malformed { line: line_number })?;
            if !KEYS.contains(&key) {
                unknown.push(key.to_owned());
                continue;
            }
            if key != "ramadan" && seen.contains(&key) {
                return Err(ConfigError::DuplicateKey(key.to_owned()));
            }
            seen.push(key);
            let invalid = || ConfigError::InvalidValue(key.to_owned());
            match key {
                "latitude" => latitude = Some(value.parse::<f64>().map_err(|_| invalid())?),
                "longitude" => longitude = Some(value.parse::<f64>().map_err(|_| invalid())?),
                "method" => config.method = value.parse().map_err(|_| invalid())?,
                "asr" => config.asr = parse_asr(value).map_err(|_| invalid())?,
                "shafaq" => config.shafaq = parse_shafaq(value).map_err(|_| invalid())?,
                "clock" => config.clock = parse_clock(value).map_err(|_| invalid())?,
                "ramadan" => {
                    let announced = parse_ramadan(value).map_err(|_| invalid())?;
                    if config.ramadan.0.iter().any(|r| r.same_month_as(announced)) {
                        return Err(ConfigError::DuplicateKey(format!("ramadan = {value}")));
                    }
                    config.ramadan.announce(announced);
                }
                // Every other key was skipped above.
                _ => {}
            }
        }
        config.location = match (latitude, longitude) {
            (Some(lat), Some(lng)) => Some(Coordinates::new(lat, lng).map_err(|e| {
                let blame_longitude = matches!(e, InvalidInput::LongitudeOutOfRange)
                    || (lat.is_finite() && !lng.is_finite());
                ConfigError::InvalidValue(
                    if blame_longitude {
                        "longitude"
                    } else {
                        "latitude"
                    }
                    .to_owned(),
                )
            })?),
            (None, None) => None,
            (Some(_), None) | (None, Some(_)) => return Err(ConfigError::IncompleteLocation),
        };
        Ok((config, unknown))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The config alone, for tests of files with only known keys.
    fn parsed(text: &str) -> Result<Config, ConfigError> {
        Config::parse(text).map(|(config, _unknown)| config)
    }

    #[test]
    fn default_round_trips() {
        let config = Config::default();
        assert_eq!(parsed(&config.render()), Ok(config), "round trip");
    }

    #[test]
    fn only_choices_that_differ_from_the_default_are_saved() {
        assert_eq!(
            Config::default().render(),
            "",
            "nothing chosen, nothing saved"
        );
        let config = Config {
            clock: Clock::TwentyFourHour,
            ..Config::default()
        };
        assert_eq!(config.render(), "clock = 24\n");
    }

    #[test]
    fn files_from_before_shafaq_still_load() {
        let old =
            "latitude = 51.5\nlongitude = -0.1\nmethod = moonsighting_committee\nasr = standard\n";
        let config = parsed(old).expect("valid");
        assert_eq!(config.shafaq, Shafaq::General, "defaults");
        assert_eq!(
            parsed("shafaq = pink"),
            Err(ConfigError::InvalidValue("shafaq".into())),
            "unknown shafaq"
        );
        assert_eq!(
            parsed("clock = 13"),
            Err(ConfigError::InvalidValue("clock".into())),
            "unknown clock"
        );
    }

    #[test]
    fn ramadan_announcements_round_trip_and_replace() {
        let mut config = Config::default();
        let late = parse_ramadan("2026-02-19..2026-03-20").expect("valid");
        let next = parse_ramadan("2027-02-08 .. 2027-03-09").expect("valid");
        config.ramadan.announce(next);
        config.ramadan.announce(late);
        assert_eq!(config.ramadan.0.as_slice(), &[late, next], "sorted");
        assert_eq!(parsed(&config.render()), Ok(config.clone()), "round trip");
        let early = parse_ramadan("2026-02-17..2026-03-18").expect("valid");
        config.ramadan.announce(early);
        assert_eq!(
            config.ramadan.0.as_slice(),
            &[early, next],
            "same month replaced"
        );
        let on = |y, m, d| config.ramadan.for_date(Date::new(y, m, d).expect("valid"));
        assert_eq!(on(2026, 3, 1), RamadanDates::Announced(early), "1447");
        assert_eq!(on(2027, 3, 1), RamadanDates::Announced(next), "1448");
        assert_eq!(
            Announcements::default().for_date(Date::new(2026, 3, 1).expect("valid")),
            RamadanDates::Calculated,
            "none announced"
        );
    }

    #[test]
    fn bad_ramadan_lines_are_rejected() {
        for bad in [
            "ramadan = 2026-02-19",
            "ramadan = 2026-02-19..2026-03-25",
            "ramadan = 2027-06-01..2027-06-30",
            "ramadan = 2026-2-x..2026-03-20",
        ] {
            assert_eq!(
                parsed(bad),
                Err(ConfigError::InvalidValue("ramadan".into())),
                "{bad}"
            );
        }
        let twice = "ramadan = 2026-02-19..2026-03-20\nramadan = 2026-02-18..2026-03-19\n";
        assert!(
            matches!(parsed(twice), Err(ConfigError::DuplicateKey(_))),
            "two announcements for one month"
        );
    }

    #[test]
    fn parses_typed_locations() {
        for line in [
            "24.7136 46.6753\n",
            "24.7136,46.6753",
            " 24.7136 ,\t46.6753\r\n",
        ] {
            let c = parse_location(line).expect("valid");
            assert_eq!(
                (c.latitude.get(), c.longitude.get()),
                (24.7136_f64, 46.6753_f64),
                "{line:?}"
            );
        }
        for line in ["", "\n", "24.7", "1 2 3", "a b", "91 0", "NaN 0"] {
            assert!(parse_location(line).is_err(), "{line:?}");
        }
    }

    proptest::proptest! {
        #[test]
        fn any_formatting_of_valid_numbers_parses(lat in -90.0_f64..=90.0_f64, lng in -180.0_f64..=180.0_f64, sep in "[ \t,]{1,4}", end in "[ \t]*\r?\n?") {
            let c = parse_location(&format!("{lat}{sep}{lng}{end}")).expect("valid");
            proptest::prop_assert_eq!((c.latitude.get().to_bits(), c.longitude.get().to_bits()), (lat.to_bits(), lng.to_bits()));
        }

        /// A newer version's settings, anywhere in the file, change nothing this one reads.
        #[test]
        fn unknown_keys_never_change_what_is_read(
            keys in proptest::collection::vec("[a-z_]{1,12}", 0..4),
            values in proptest::collection::vec("[ -~]{0,20}", 4),
            positions in proptest::collection::vec(0_usize..5, 4),
        ) {
            let known = ["clock = 24", "latitude = 1", "longitude = 2", "asr = hanafi"];
            let mut lines: Vec<String> = known.iter().map(|line| (*line).to_owned()).collect();
            let mut added = Vec::new();
            for ((key, value), position) in keys.iter().zip(&values).zip(&positions) {
                if !KEYS.contains(&key.as_str()) {
                    lines.insert((*position).min(lines.len()), format!("{key} = {value}"));
                    added.push(key.clone());
                }
            }
            let (config, mut unknown) = Config::parse(&lines.join("\n")).expect("loads");
            proptest::prop_assert_eq!(Ok(config), parsed(&known.join("\n")));
            unknown.sort();
            added.sort();
            proptest::prop_assert_eq!(unknown, added);
        }

        /// Any text loads or is refused; none makes the parser panic.
        #[test]
        fn parsing_is_total(text in "(\\PC|\n|=|#){0,200}") {
            drop(Config::parse(&text));
        }
    }

    #[test]
    fn unknown_keys_are_skipped_and_named() {
        let (config, unknown) =
            Config::parse("colour = red\nclock = 24\ncolour = blue\nnotify = yes").expect("loads");
        assert_eq!(
            config.clock,
            Clock::TwentyFourHour,
            "known keys still apply"
        );
        assert_eq!(unknown, ["colour", "colour", "notify"]);
    }

    #[test]
    fn rejects_bad_input() {
        assert_eq!(
            parsed("nonsense"),
            Err(ConfigError::Malformed { line: 1 }),
            "malformed"
        );
        assert_eq!(
            parsed("asr = hanafi\nasr = standard"),
            Err(ConfigError::DuplicateKey("asr".into())),
            "duplicate"
        );
        assert_eq!(
            parsed("latitude = 1"),
            Err(ConfigError::IncompleteLocation),
            "half location"
        );
        assert_eq!(
            parsed("latitude = 91\nlongitude = 0"),
            Err(ConfigError::InvalidValue("latitude".into())),
            "latitude out of range"
        );
        assert_eq!(
            parsed("latitude = 1\nlongitude = 181"),
            Err(ConfigError::InvalidValue("longitude".into())),
            "longitude out of range"
        );
        assert_eq!(
            parsed("\u{feff}latitude = 1\nlongitude = 2").map(|c| c.location.is_some()),
            Ok(true),
            "byte-order mark"
        );
    }
}

#[cfg(test)]
mod state_machine {
    //! Stateful, model-based test of the config store.
    //!
    //! proptest generates random *sequences* of operations (set location, change method, reload from
    //! disk, hand-edit comments in…), applies each to both a trivial reference model and the real
    //! system (the persisted file text), and checks after every step that they agree. Failing
    //! sequences are shrunk to the shortest reproduction.

    use super::{Clock, Config, parse_ramadan};
    use hya_engine::{AsrShadow, Coordinates, Method, Shafaq};
    use proptest::prelude::*;
    use proptest::test_runner::Config as ProptestConfig;
    use proptest_state_machine::{ReferenceStateMachine, StateMachineTest, prop_state_machine};

    #[derive(Debug, Clone, PartialEq)]
    struct Model {
        location: Option<(f64, f64)>,
        method: Method,
        asr: AsrShadow,
        shafaq: Shafaq,
        clock: Clock,
        /// (month label, announcement text), sorted by text, one per label.
        ramadan: Vec<(&'static str, &'static str)>,
    }

    /// Valid announcements, labelled by the calculated month each one replaces. The labels are
    /// written by hand so the model does not borrow the code's notion of "same month".
    const ANNOUNCEMENTS: [(&str, &str); 6] = [
        ("1447", "2026-02-17..2026-03-18"),
        ("1447", "2026-02-18..2026-03-19"),
        ("1447", "2026-02-19..2026-03-20"),
        ("1448", "2027-02-08..2027-03-09"),
        ("1448", "2027-02-09..2027-03-09"),
        ("1449", "2028-01-28..2028-02-26"),
    ];

    #[derive(Debug, Clone)]
    enum Operation {
        SetLocation(f64, f64),
        ClearLocation,
        SetMethod(Method),
        SetAsr(AsrShadow),
        SetShafaq(Shafaq),
        SetClock(Clock),
        Announce((&'static str, &'static str)),
        /// Save then load: the file must survive a round trip unchanged.
        Reload,
        /// A user adds a comment by hand; the parser must ignore it.
        AddComment(String),
    }

    struct Reference;

    impl ReferenceStateMachine for Reference {
        type State = Model;
        type Transition = Operation;

        fn init_state() -> BoxedStrategy<Model> {
            Just(Model {
                location: None,
                method: Method::UmmAlQura,
                asr: AsrShadow::Standard,
                shafaq: Shafaq::General,
                clock: Clock::TwelveHour,
                ramadan: Vec::new(),
            })
            .boxed()
        }

        fn transitions(_: &Model) -> BoxedStrategy<Operation> {
            prop_oneof![
                (-90.0_f64..=90.0, -180.0_f64..=180.0)
                    .prop_map(|(lat, lng)| Operation::SetLocation(lat, lng)),
                Just(Operation::ClearLocation),
                proptest::sample::select(Method::ALL.to_vec()).prop_map(Operation::SetMethod),
                prop_oneof![Just(AsrShadow::Standard), Just(AsrShadow::Hanafi)]
                    .prop_map(Operation::SetAsr),
                proptest::sample::select(vec![Shafaq::General, Shafaq::Red, Shafaq::White])
                    .prop_map(Operation::SetShafaq),
                prop_oneof![Just(Clock::TwentyFourHour), Just(Clock::TwelveHour)]
                    .prop_map(Operation::SetClock),
                proptest::sample::select(ANNOUNCEMENTS.to_vec()).prop_map(Operation::Announce),
                Just(Operation::Reload),
                "[a-z =#]{0,20}".prop_map(Operation::AddComment),
            ]
            .boxed()
        }

        fn apply(mut model: Model, operation: &Operation) -> Model {
            match operation {
                Operation::SetLocation(lat, lng) => model.location = Some((*lat, *lng)),
                Operation::ClearLocation => model.location = None,
                Operation::SetMethod(method) => model.method = *method,
                Operation::SetAsr(asr) => model.asr = *asr,
                Operation::SetShafaq(shafaq) => model.shafaq = *shafaq,
                Operation::SetClock(clock) => model.clock = *clock,
                Operation::Announce(entry) => {
                    let entry = *entry;
                    model.ramadan.retain(|(label, _)| *label != entry.0);
                    model.ramadan.push(entry);
                    model.ramadan.sort_by_key(|(_, text)| *text);
                }
                Operation::Reload | Operation::AddComment(_) => {}
            }
            model
        }
    }

    /// The system under test is the file contents on disk.
    struct ConfigFile;

    fn load(text: &str) -> Config {
        match Config::parse(text) {
            Ok((config, unknown)) if unknown.is_empty() => config,
            other => panic!("stored config must parse cleanly: {other:?}\n---\n{text}"),
        }
    }

    impl StateMachineTest for ConfigFile {
        type SystemUnderTest = String;
        type Reference = Reference;

        fn init_test(_: &Model) -> String {
            Config::default().render()
        }

        fn apply(file: String, _: &Model, operation: Operation) -> String {
            let mut config = load(&file);
            match operation {
                Operation::SetLocation(lat, lng) => {
                    config.location = Some(Coordinates::new(lat, lng).expect("generated in range"));
                }
                Operation::ClearLocation => config.location = None,
                Operation::SetMethod(method) => config.method = method,
                Operation::SetAsr(asr) => config.asr = asr,
                Operation::SetShafaq(shafaq) => config.shafaq = shafaq,
                Operation::SetClock(clock) => config.clock = clock,
                Operation::Announce(entry) => config
                    .ramadan
                    .announce(parse_ramadan(entry.1).expect("valid announcement")),
                Operation::Reload => {}
                Operation::AddComment(comment) => return format!("# {comment}\n{file}"),
            }
            config.render()
        }

        fn check_invariants(file: &String, model: &Model) {
            let config = load(file);
            let location = config
                .location
                .map(|c| (c.latitude.get(), c.longitude.get()));
            // Bitwise comparison: rendering must round-trip floats exactly.
            let bits = |l: Option<(f64, f64)>| l.map(|(a, b)| (a.to_bits(), b.to_bits()));
            assert_eq!(bits(location), bits(model.location), "location drifted");
            assert_eq!(config.method, model.method, "method drifted");
            assert_eq!(config.asr, model.asr, "asr drifted");
            assert_eq!(config.shafaq, model.shafaq, "shafaq drifted");
            assert_eq!(config.clock, model.clock, "clock drifted");
            let announced: Vec<String> = config
                .ramadan
                .0
                .iter()
                .map(|r| format!("{}..{}", r.first(), r.last()))
                .collect();
            let expected: Vec<&str> = model.ramadan.iter().map(|(_, text)| *text).collect();
            assert_eq!(announced, expected, "ramadan drifted");
        }
    }

    prop_state_machine! {
        #![proptest_config(ProptestConfig { cases: 512, ..ProptestConfig::default() })]

        #[test]
        fn config_file_matches_model(sequential 1..60 => ConfigFile);
    }
}
