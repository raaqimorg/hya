//! `hya`: Islamic prayer times in the terminal.
//!
//! All I/O lives here: arguments, the config file, the clock, the system time zone, the prompt,
//! and output. The calculation is `hya_engine::compute`.

mod config;

use core::fmt::{self, Write as _};
use std::ffi::OsString;
use std::io::{self, IsTerminal as _, Write as _};
use std::path::PathBuf;
use std::process::ExitCode;
use std::{env, fs};

use clap::Parser;
use clap::builder::styling::{AnsiColor, Style};
use clap::builder::{PossibleValuesParser, TypedValueParser as _};
use config::{Clock, Config, parse_asr, parse_clock, parse_location, parse_ramadan, parse_shafaq};
use hya_countries::{CountryCode, Entry};
use hya_engine::{
    AnnouncedRamadan, AsrShadow, Coordinates, Date, HighLatitudeRule, Method, Parameters, Prayer,
    PrayerTimes, Shafaq, compute,
};
use jiff::tz::TimeZone;
use jiff::{Timestamp, Zoned};

/// Islamic prayer times from coordinates alone. `--help` prints [`HELP`].
#[derive(Debug, Parser)]
#[command(name = "hya", version, override_usage = "hya [flags]", override_help = help())]
struct Args {
    #[arg(long, value_name = "deg", allow_hyphen_values = true, requires = "lng",
        value_parser = finite)]
    lat: Option<f64>,
    #[arg(long, value_name = "deg", allow_hyphen_values = true, requires = "lat",
        value_parser = finite)]
    lng: Option<f64>,
    #[arg(long, value_name = "zone")]
    tz: Option<String>,
    #[arg(long, value_name = "id", value_parser = PossibleValuesParser::new(Method::ALL.map(Method::id))
        .try_map(|id| id.parse::<Method>()))]
    method: Option<Method>,
    #[arg(long, value_name = "cc", conflicts_with = "method")]
    country: Option<CountryCode>,
    #[arg(long, value_name = "rule", value_parser = PossibleValuesParser::new(["standard", "hanafi"])
        .try_map(|id| parse_asr(&id)))]
    asr: Option<AsrShadow>,
    #[arg(long, value_name = "glow", value_parser = PossibleValuesParser::new(["general", "red", "white"])
        .try_map(|id| parse_shafaq(&id)))]
    shafaq: Option<Shafaq>,
    #[arg(long, value_name = "a..b", value_parser = parse_ramadan)]
    ramadan: Option<AnnouncedRamadan>,
    #[arg(long, value_name = "date")]
    date: Option<jiff::civil::Date>,
    #[arg(long, value_name = "12|24", value_parser = PossibleValuesParser::new(["24", "12"])
        .try_map(|id| parse_clock(&id)))]
    clock: Option<Clock>,
    #[arg(long, conflicts_with = "date")]
    next: bool,
    #[arg(long)]
    save: bool,
    #[arg(long, exclusive = true)]
    reset: bool,
}

/// A finite number: `nan` and `inf` parse as `f64` but are no coordinate.
fn finite(text: &str) -> Result<f64, String> {
    text.parse::<f64>()
        .ok()
        .filter(|value| value.is_finite())
        .ok_or_else(|| "expected a finite number".to_owned())
}

/// One flag in the help: the flag, what it does, and its default.
type Row = (&'static str, &'static str, &'static str);

/// The help, by group under a heading.
const HELP: [(&str, &[Row]); 4] = [
    (
        "Location",
        &[
            ("--lat <deg>", "Latitude, north positive", ""),
            ("--lng <deg>", "Longitude, east positive", ""),
            ("--tz <zone>", "Time zone, e.g. Asia/Riyadh", "system"),
        ],
    ),
    (
        "Method",
        &[
            (
                "--method <id>",
                "Method, from the list below",
                "umm_al_qura",
            ),
            (
                "--country <cc>",
                "Method of the country's authority, e.g. SA",
                "",
            ),
            ("--asr <rule>", "Asr shadow: standard or hanafi", "standard"),
            (
                "--shafaq <glow>",
                "Isha glow: general, red, or white",
                "general",
            ),
            (
                "--ramadan <a..b>",
                "Announced Ramadan, e.g. 2027-02-09..2027-03-10",
                "",
            ),
        ],
    ),
    (
        "Output",
        &[
            ("--date <date>", "Day to show, e.g. 2027-02-15", "today"),
            ("--clock <12|24>", "Clock", "12"),
            ("--next", "Print only the next prayer", ""),
        ],
    ),
    (
        "",
        &[
            ("--save", "Keep these flags as defaults", ""),
            ("--reset", "Forget every saved default", ""),
            ("-h, --help", "Print this help", ""),
            ("-V, --version", "Print the version", ""),
        ],
    ),
];

/// [`HELP`] in columns, then the method IDs. Clap strips the colour in a pipe and when
/// `NO_COLOR` is set.
fn help() -> String {
    // Writing to a `String` cannot fail.
    write_help().unwrap_or_default()
}

fn write_help() -> Result<String, fmt::Error> {
    let heading = AnsiColor::Cyan.on_default().bold();
    let bold = Style::new().bold();
    let dim = Style::new().dimmed();
    let flag_width = HELP
        .iter()
        .flat_map(|(_, rows)| rows.iter())
        .map(|(flag, _, _)| flag.chars().count())
        .max()
        .unwrap_or(0);
    let text_width = HELP
        .iter()
        .flat_map(|(_, rows)| rows.iter())
        .filter(|(_, _, default)| !default.is_empty())
        .map(|(_, text, _)| text.chars().count())
        .max()
        .unwrap_or(0);

    let mut out = format!(
        "{bold}hya{bold:#} {dim}·{dim:#} Islamic prayer times from coordinates alone\n\n\
         {heading}Usage{heading:#}  hya {dim}[flags]{dim:#}\n"
    );
    for (title, rows) in HELP {
        out.push('\n');
        if !title.is_empty() {
            writeln!(out, "{heading}{title}{heading:#}")?;
        }
        for (flag, text, default) in rows {
            let (name, value) = flag.split_once(' ').unwrap_or((flag, ""));
            let pad = " ".repeat(flag_width.saturating_sub(flag.chars().count()));
            let value = if value.is_empty() {
                String::new()
            } else {
                format!(" {dim}{value}{dim:#}")
            };
            write!(out, "  {bold}{name}{bold:#}{value}{pad}   {text}")?;
            if !default.is_empty() {
                let gap = " ".repeat(text_width.saturating_sub(text.chars().count()));
                write!(out, "{gap}   {dim}{default}{dim:#}")?;
            }
            out.push('\n');
        }
    }

    writeln!(out, "\n{heading}Methods{heading:#}")?;
    let mut line = String::new();
    for id in Method::ALL.map(Method::id) {
        if !line.is_empty() && line.len().saturating_add(id.len()) > 66 {
            writeln!(out, "  {}", line.trim_end())?;
            line.clear();
        }
        line.push_str(id);
        line.push_str("  ");
    }
    writeln!(out, "  {}", line.trim_end())?;
    write!(out, "\n{dim}Defaults live in ~/.config/hya/config.{dim:#}")?;
    Ok(out)
}

/// `$XDG_CONFIG_HOME/hya/config`, else `~/.config/hya/config`, else (Windows)
/// `%APPDATA%\hya\config`. Empty or relative variables are ignored, as the XDG spec requires,
/// so an unset-but-exported `XDG_CONFIG_HOME=` never writes into the current directory.
fn config_path() -> Option<PathBuf> {
    config_path_in(|name| env::var_os(name))
}

/// [`config_path`], with the environment passed in.
fn config_path_in(var: impl Fn(&str) -> Option<OsString>) -> Option<PathBuf> {
    let absolute = |name: &str| {
        var(name)
            .map(PathBuf::from)
            .filter(|path| path.is_absolute())
    };
    let base = absolute("XDG_CONFIG_HOME")
        .or_else(|| absolute("HOME").map(|home| home.join(".config")))
        .or_else(|| absolute("APPDATA"))?;
    Some(base.join("hya").join("config"))
}

/// The saved defaults. A key this version does not know, perhaps from a newer one, gets a
/// note and is skipped; a value it cannot read is an error that names the way out.
fn load_config() -> Result<Config, String> {
    let Some(path) = config_path() else {
        return Ok(Config::default());
    };
    let text = match fs::read_to_string(&path) {
        Ok(text) => text,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Config::default()),
        Err(e) => return Err(format!("{}: {e}", path.display())),
    };
    let (config, unknown) = Config::parse(&text).map_err(|e| {
        format!(
            "{}: {e}; fix it, or start over with `hya --reset`",
            path.display()
        )
    })?;
    let mut notes = io::stderr().lock();
    for key in unknown {
        // A note that cannot be written changes nothing about the times.
        drop(writeln!(
            notes,
            "note: {}: skipping unknown setting `{key}`",
            path.display()
        ));
    }
    Ok(config)
}

/// Deletes the saved defaults, so the next run starts as the first did.
fn reset_config() -> Result<(), String> {
    let path = config_path()
        .ok_or("cannot locate a config directory: set HOME, XDG_CONFIG_HOME or APPDATA")?;
    let message = match fs::remove_file(&path) {
        Ok(()) => format!("removed {}", path.display()),
        Err(e) if e.kind() == io::ErrorKind::NotFound => {
            format!("nothing saved at {}", path.display())
        }
        Err(e) => return Err(format!("{}: {e}", path.display())),
    };
    writeln!(io::stderr().lock(), "{message}").map_err(|e| e.to_string())
}

fn save_config(config: &Config) -> Result<(), String> {
    let path = config_path()
        .ok_or("cannot locate a config directory: set HOME, XDG_CONFIG_HOME or APPDATA")?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("{}: {e}", parent.display()))?;
    }
    fs::write(&path, config.render()).map_err(|e| format!("{}: {e}", path.display()))
}

/// Asks on the terminal, the prompt on stderr so stdout carries only the times. Without a
/// terminal (a pipe, cron, CI) there is no one to ask, so it says which flags to pass instead.
/// Asks for a location until the answer is valid. The input ending (Ctrl-D) gives up.
fn ask_location(
    input: &mut impl io::BufRead,
    prompt: &mut impl io::Write,
) -> Result<Coordinates, String> {
    let fail = |e: io::Error| e.to_string();
    writeln!(prompt, "Enter latitude and longitude, e.g. 24.7136 46.6753").map_err(fail)?;
    loop {
        write!(prompt, "> ")
            .and_then(|()| prompt.flush())
            .map_err(fail)?;
        let mut line = String::new();
        if input.read_line(&mut line).map_err(fail)? == 0 {
            return Err("no location given".into());
        }
        match parse_location(&line) {
            Ok(location) => return Ok(location),
            Err(e) => writeln!(prompt, "{e}").map_err(fail)?,
        }
    }
}

fn run() -> Result<(), String> {
    let args = Args::parse();
    if args.reset {
        return reset_config();
    }
    let mut config = load_config()?;

    if let (Some(lat), Some(lng)) = (args.lat, args.lng) {
        config.location = Some(Coordinates::new(lat, lng).map_err(|e| e.to_string())?);
    }
    let chosen_for = args
        .country
        .map(|country| {
            hya_countries::entry_for(country).ok_or_else(|| {
                format!("no single method is on record for {country}; choose one with --method")
            })
        })
        .transpose()?;
    if let Some(entry) = chosen_for {
        config.method = entry.method;
    }
    if let Some(method) = args.method {
        config.method = method;
    }
    if let Some(asr) = args.asr {
        config.asr = asr;
    }
    let seasonal = matches!(
        config.method.parameters().high_latitude,
        HighLatitudeRule::SeasonAdjusted(_)
    );
    if let Some(shafaq) = args.shafaq {
        if !seasonal {
            return Err(format!(
                "--shafaq only applies to moonsighting_committee, not {}",
                config.method
            ));
        }
        config.shafaq = shafaq;
    }
    if let Some(clock) = args.clock {
        config.clock = clock;
    }
    if let Some(announced) = args.ramadan {
        config.ramadan.announce(announced);
    }
    let asked = config.location.is_none();
    let location = if let Some(location) = config.location {
        location
    } else {
        if !io::stdin().is_terminal() {
            return Err("no saved location: pass --lat and --lng (add --save to keep them)".into());
        }
        let location = ask_location(&mut io::stdin().lock(), &mut io::stderr().lock())?;
        config.location = Some(location);
        location
    };

    let zone = match &args.tz {
        Some(name) => TimeZone::get(name).map_err(|e| e.to_string())?,
        None => TimeZone::system(),
    };
    let today = Zoned::now().with_time_zone(zone.clone()).date();
    let civil = args.date.unwrap_or(today);
    let base = config.method.parameters();
    let parameters_on = |date: Date| Parameters {
        asr: config.asr,
        high_latitude: if seasonal {
            HighLatitudeRule::SeasonAdjusted(config.shafaq)
        } else {
            base.high_latitude
        },
        ramadan: config.ramadan.for_date(date),
        ..base
    };
    let date = engine_date(civil)?;
    // Every input is valid by now, so a run that fails on one saves nothing.
    if args.save || asked {
        save_config(&config)?;
    }
    let times = schedule_on(civil, &zone, date, location, parameters_on(date))?;
    let night = night_on(civil, &zone, location, &parameters_on, &times);

    // The countdown only makes sense for today.
    let next = (civil == today)
        .then(|| {
            next_from(
                Timestamp::now().as_second(),
                &times,
                || time_on(civil, -1, Prayer::Isha, &zone, location, &parameters_on),
                || time_on(civil, 1, Prayer::Fajr, &zone, location, &parameters_on),
            )
        })
        .flatten();

    let screen = Screen {
        zone: &zone,
        config: &config,
        times: &times,
        night: &night,
        next,
        color: io::stdout().is_terminal() && color_allowed(),
    };
    match show(&screen, args.next, chosen_for) {
        // The reader went away (`hya | head -1`): nothing is wrong, and no one is listening.
        Err(e) if e.kind() == io::ErrorKind::BrokenPipe => Ok(()),
        result => result.map_err(|e| e.to_string()),
    }
}

/// The times on stdout; with `--next`, only the next prayer. Notes on stderr.
fn show(screen: &Screen<'_>, next_only: bool, chosen_for: Option<Entry>) -> io::Result<()> {
    if next_only {
        return screen.write_next(&mut io::stdout().lock());
    }
    screen.write(&mut io::stdout().lock())?;
    write_notes(screen.times, chosen_for)
}

fn engine_date(civil: jiff::civil::Date) -> Result<Date, String> {
    Date::new(
        civil.year(),
        u8::try_from(civil.month()).map_err(|e| e.to_string())?,
        u8::try_from(civil.day()).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())
}

/// The next prayer after `now`. After Isha, that is tomorrow's Fajr; before Fajr, the window
/// began with yesterday's Isha.
fn next_from(
    now: i64,
    times: &PrayerTimes,
    yesterday_isha: impl FnOnce() -> Option<i64>,
    tomorrow_fajr: impl FnOnce() -> Option<i64>,
) -> Option<Next> {
    match next_after(times, now) {
        Some(prayer) => Some(Next {
            prayer,
            at: times.get(prayer).unix_seconds(),
            now,
            tomorrow: false,
            since: Prayer::ALL
                .into_iter()
                .map(|time| times.get(time).unix_seconds())
                .rfind(|&at| at <= now)
                .or_else(yesterday_isha),
        }),
        None => tomorrow_fajr().map(|at| Next {
            prayer: Prayer::Fajr,
            at,
            now,
            tomorrow: true,
            since: Some(times.get(Prayer::Isha).unix_seconds()),
        }),
    }
}

/// The first prayer after `now`. Sunrise ends Fajr and is no prayer, so it is never next.
fn next_after(times: &PrayerTimes, now: i64) -> Option<Prayer> {
    Prayer::ALL
        .into_iter()
        .filter(|&prayer| prayer != Prayer::Sunrise)
        .find(|&prayer| times.get(prayer).unix_seconds() > now)
}

/// One time on the civil day `step` days from `civil`, or `None` where that day has none
/// (polar day or night).
fn time_on(
    civil: jiff::civil::Date,
    step: i8,
    prayer: Prayer,
    zone: &TimeZone,
    location: Coordinates,
    parameters_on: &impl Fn(Date) -> Parameters,
) -> Option<i64> {
    schedule_near(civil, step, zone, location, parameters_on)
        .map(|times| times.get(prayer).unix_seconds())
}

/// The schedule of the civil day `step` days from `civil`, or `None` where it has none.
fn schedule_near(
    civil: jiff::civil::Date,
    step: i8,
    zone: &TimeZone,
    location: Coordinates,
    parameters_on: &impl Fn(Date) -> Parameters,
) -> Option<PrayerTimes> {
    let civil = civil.checked_add(jiff::Span::new().days(step)).ok()?;
    let date = engine_date(civil).ok()?;
    schedule_on(civil, zone, date, location, parameters_on(date)).ok()
}

/// Midnight and the last third that fall on `civil` in `zone`, from the night before it and
/// the day's own, `today`'s. The night before usually gives its last third, the day's own
/// night its midnight.
fn night_on(
    civil: jiff::civil::Date,
    zone: &TimeZone,
    location: Coordinates,
    parameters_on: &impl Fn(Date) -> Parameters,
    today: &PrayerTimes,
) -> Vec<(&'static str, i64)> {
    let yesterday = schedule_near(civil, -1, zone, location, parameters_on);
    yesterday
        .iter()
        .chain([today])
        .flat_map(|times| {
            [
                ("Midnight", times.midnight()),
                ("Last third", times.last_third()),
            ]
        })
        .filter_map(|(name, at)| Some((name, at?.unix_seconds())))
        .filter(|&(_, at)| {
            Timestamp::from_second(at).is_ok_and(|at| at.to_zoned(zone.clone()).date() == civil)
        })
        .collect()
}

/// The schedule whose Dhuhr falls on `civil` in `zone`.
///
/// `compute` takes the local *solar* day at the longitude. Where the zone is far from that
/// (Samoa and Tonga at UTC+13, Kiribati at UTC+14), the solar day is a civil day behind, so
/// this tries the neighbouring days too. Elsewhere the first candidate is the answer.
fn schedule_on(
    civil: jiff::civil::Date,
    zone: &TimeZone,
    date: Date,
    location: Coordinates,
    parameters: Parameters,
) -> Result<PrayerTimes, String> {
    let on_civil = |times: &PrayerTimes| {
        Timestamp::from_second(times.dhuhr().unix_seconds())
            .is_ok_and(|dhuhr| dhuhr.to_zoned(zone.clone()).date() == civil)
    };
    let times = compute(date, location, parameters).map_err(|e| e.to_string())?;
    if on_civil(&times) {
        return Ok(times);
    }
    for step in [-1_i32, 1_i32] {
        let neighbour = date
            .days_since_unix_epoch()
            .checked_add(step)
            .and_then(|days| Date::from_days_since_unix_epoch(days).ok());
        if let Some(Ok(other)) = neighbour.map(|day| compute(day, location, parameters))
            && on_civil(&other)
        {
            return Ok(other);
        }
    }
    Ok(times)
}

/// The next prayer, when the day shown is today.
#[derive(Debug, Clone, Copy)]
struct Next {
    prayer: Prayer,
    /// When it begins, in Unix seconds.
    at: i64,
    /// The time now, in Unix seconds.
    now: i64,
    /// It is tomorrow's Fajr: today's Isha has passed.
    tomorrow: bool,
    /// When the current window began, in Unix seconds, for the progress bar.
    since: Option<i64>,
}

/// One run's output.
struct Screen<'a> {
    zone: &'a TimeZone,
    config: &'a Config,
    times: &'a PrayerTimes,
    /// Midnight and the last third on the day shown, from [`night_on`].
    night: &'a [(&'static str, i64)],
    next: Option<Next>,
    color: bool,
}

impl Screen<'_> {
    fn write(&self, out: &mut impl io::Write) -> io::Result<()> {
        if self.color {
            self.write_styled(out)
        } else {
            self.write_plain(out)
        }
    }

    /// For a terminal: the next prayer and the time left first, a bar for how much of the
    /// current window has passed, then the day's times.
    ///
    /// ```text
    ///   Asr in 1h 23m
    ///   ━━━━━━━━━━━━━──────
    ///
    ///     Fajr     04:40
    ///     ...
    ///   ▸ Asr      14:51
    ///     ...
    /// ```
    fn write_styled(&self, out: &mut impl io::Write) -> io::Result<()> {
        writeln!(out)?;
        if let Some(next) = self.next {
            let tomorrow = if next.tomorrow {
                paint(
                    DIM,
                    &format!("  tomorrow {}", self.local(next.at)?.trim_start()),
                )
            } else {
                String::new()
            };
            writeln!(
                out,
                "  {} {}{tomorrow}",
                paint(ACCENT_BOLD, prayer_name(next.prayer)),
                paint(BOLD, &format!("in {}", left(next.now, next.at)))
            )?;
            if let Some(start) = next.since {
                // As wide as a row below it: the marker, the name, and the time.
                let width = self
                    .local(next.at)?
                    .chars()
                    .count()
                    .saturating_add(NAME_WIDTH)
                    .saturating_add(3);
                let filled = filled(start, next.now, next.at, width);
                writeln!(
                    out,
                    "  {}{}",
                    paint(ACCENT, &"━".repeat(filled)),
                    paint(DIM, &"─".repeat(width.saturating_sub(filled)))
                )?;
            }
            writeln!(out)?;
        }
        for (prayer, name, at) in self.rows() {
            let row = format!("{name:<NAME_WIDTH$} {}", self.local(at)?);
            match self.next {
                Some(next) if !next.tomorrow && prayer == Some(next.prayer) => {
                    writeln!(out, "  {} {}", paint(ACCENT, "▸"), paint(ACCENT_BOLD, &row))?;
                }
                // The night's marks are no prayer: always in the background.
                _ if prayer.is_none() => writeln!(out, "    {}", paint(DIM, &row))?,
                Some(next) if at <= next.now => writeln!(out, "    {}", paint(DIM, &row))?,
                _ => writeln!(out, "    {row}")?,
            }
        }
        out.flush()
    }

    /// For a pipe: one line per time, with the time left after the next prayer.
    fn write_plain(&self, out: &mut impl io::Write) -> io::Result<()> {
        for (prayer, name, at) in self.rows() {
            write!(out, "  {name:<NAME_WIDTH$} {}", self.local(at)?)?;
            match self.next {
                Some(next) if !next.tomorrow && prayer == Some(next.prayer) => {
                    writeln!(out, "   in {}", left(next.now, next.at))?;
                }
                _ => writeln!(out)?,
            }
        }
        if let Some(next) = self.next.filter(|next| next.tomorrow) {
            writeln!(
                out,
                "  {:<NAME_WIDTH$} {}   tomorrow, in {}",
                prayer_name(next.prayer),
                self.local(next.at)?,
                left(next.now, next.at)
            )?;
        }
        out.flush()
    }

    /// One line: `Dhuhr 11:38 in 1h 23m`.
    fn write_next(&self, out: &mut impl io::Write) -> io::Result<()> {
        let Some(next) = self.next else {
            return Err(io::Error::other("no prayer is left today or tomorrow"));
        };
        let when = if next.tomorrow { "tomorrow, in" } else { "in" };
        writeln!(
            out,
            "{} {} {when} {}",
            prayer_name(next.prayer),
            self.local(next.at)?.trim_start(),
            left(next.now, next.at)
        )?;
        out.flush()
    }

    /// The six times and the night's marks, in the order they come on the day. Each with its
    /// prayer, if it is one, and in Unix seconds.
    fn rows(&self) -> Vec<(Option<Prayer>, &'static str, i64)> {
        let mut rows: Vec<_> = Prayer::ALL
            .into_iter()
            .map(|prayer| {
                (
                    Some(prayer),
                    prayer_name(prayer),
                    self.times.get(prayer).unix_seconds(),
                )
            })
            .chain(self.night.iter().map(|&(name, at)| (None, name, at)))
            .collect();
        rows.sort_by_key(|&(_, _, at)| at);
        rows
    }

    fn local(&self, at: i64) -> io::Result<String> {
        let instant = Timestamp::from_second(at).map_err(io::Error::other)?;
        Ok(instant
            .to_zoned(self.zone.clone())
            .strftime(self.config.clock.pattern())
            .to_string())
    }
}

/// The name column: as wide as the longest name, "Last third".
const NAME_WIDTH: usize = 10;

const BOLD: &str = "\x1b[1m";
const DIM: &str = "\x1b[2m";
const ACCENT: &str = "\x1b[36m";
const ACCENT_BOLD: &str = "\x1b[1;36m";
const RESET: &str = "\x1b[0m";

fn paint(style: &str, text: &str) -> String {
    format!("{style}{text}{RESET}")
}

/// How many of `width` cells the time from `start` to `now` fills, of the time from `start`
/// to `end`.
fn filled(start: i64, now: i64, end: i64, width: usize) -> usize {
    let span = end.saturating_sub(start);
    let done = now.saturating_sub(start).clamp(0, span);
    i64::try_from(width)
        .ok()
        .and_then(|width| done.checked_mul(width))
        .and_then(|cells| cells.checked_div(span))
        .and_then(|cells| usize::try_from(cells).ok())
        .map_or(0, |cells| cells.min(width))
}

/// Colour unless `NO_COLOR` is set to anything (no-color.org) or the terminal is `dumb`.
fn color_allowed() -> bool {
    env::var_os("NO_COLOR").is_none_or(|value| value.is_empty())
        && env::var_os("TERM").is_none_or(|term| term != "dumb")
}

/// `1h 23m`, `23m`, `2h`: the time left, rounded up to the minute, so a prayer never shows as
/// due before it begins.
fn left(now: i64, at: i64) -> String {
    let minutes = at
        .saturating_sub(now)
        .saturating_add(59)
        .checked_div(60)
        .unwrap_or(0);
    let hours = minutes.checked_div(60).unwrap_or(0);
    let minutes = minutes.checked_rem(60).unwrap_or(0);
    match (hours, minutes) {
        (0, minutes) => format!("{minutes}m"),
        (hours, 0) => format!("{hours}h"),
        (hours, minutes) => format!("{hours}h {minutes}m"),
    }
}

const fn prayer_name(prayer: Prayer) -> &'static str {
    match prayer {
        Prayer::Fajr => "Fajr",
        Prayer::Sunrise => "Sunrise",
        Prayer::Dhuhr => "Dhuhr",
        Prayer::Asr => "Asr",
        Prayer::Maghrib => "Maghrib",
        Prayer::Isha => "Isha",
    }
}

/// A method as people name it, by its authority where it has one.
const fn method_name(method: Method) -> &'static str {
    match method {
        Method::UmmAlQura => "Umm al-Qura",
        Method::MuslimWorldLeague => "Muslim World League",
        Method::Egyptian => "Egyptian",
        Method::Karachi => "Karachi",
        Method::NorthAmerica => "ISNA",
        Method::Dubai => "Dubai",
        Method::Kuwait => "Kuwait",
        Method::Qatar => "Qatar",
        Method::Singapore => "MUIS",
        Method::Turkey => "Diyanet",
        Method::Tehran => "Tehran",
        Method::MoonsightingCommittee => "Moonsighting Committee",
        Method::Malaysia => "JAKIM",
        Method::MalaysiaKelantan => "JAKIM, Kelantan",
        Method::Jordan => "Awqaf Jordan",
    }
}

/// Why a time differs from the plain calculation, on stderr, so a pipe gets only the times.
fn write_notes(times: &PrayerTimes, chosen_for: Option<Entry>) -> io::Result<()> {
    let mut notes = io::stderr().lock();
    let flags = times.notes();
    let set_by_rule = match (flags.fajr_high_latitude, flags.isha_high_latitude) {
        (true, true) => Some("Fajr and Isha"),
        (true, false) => Some("Fajr"),
        (false, true) => Some("Isha"),
        (false, false) => None,
    };
    if let Some(which) = set_by_rule {
        writeln!(notes, "note: the high-latitude rule sets {which}")?;
    }
    if flags.maghrib_at_sunset {
        writeln!(
            notes,
            "note: Maghrib at sunset; the method's angle falls after Isha"
        )?;
    }
    if flags.ramadan_isha {
        writeln!(notes, "note: Isha uses the Ramadan interval")?;
    }
    if let Some(entry) = chosen_for {
        writeln!(
            notes,
            "note: {} uses {}, per {}",
            entry.country,
            method_name(entry.method),
            entry.authority
        )?;
    }
    Ok(())
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            // If stderr itself is gone there is nowhere left to report to.
            drop(writeln!(io::stderr().lock(), "hya: {message}"));
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_prompt_asks_again_until_an_answer_is_valid() {
        let mut prompt = Vec::new();
        let mut input = io::Cursor::new("abc\n91 0\n24.7136 46.6753\n");
        let location = ask_location(&mut input, &mut prompt).expect("the third answer");
        assert_eq!(location, Coordinates::new(24.7136, 46.6753).expect("valid"));
        let prompt = String::from_utf8(prompt).expect("utf-8");
        assert_eq!(prompt.matches("> ").count(), 3, "{prompt}");
        assert!(prompt.contains("expected two numbers"), "{prompt}");
        assert!(prompt.contains("latitude must be within"), "{prompt}");
    }

    #[test]
    fn the_prompt_gives_up_when_the_input_ends() {
        let mut input = io::Cursor::new("abc\n");
        let result = ask_location(&mut input, &mut Vec::new());
        assert_eq!(result, Err("no location given".to_owned()));
    }

    /// The styled screen as plain text, at `now`.
    /// The screen for Riyadh at `now`, with its colour codes.
    fn written(times: &PrayerTimes, now: i64, config: &Config, color: bool) -> String {
        let zone = TimeZone::get("Asia/Riyadh").expect("zone");
        let civil = Timestamp::from_second(times.dhuhr().unix_seconds())
            .expect("in range")
            .to_zoned(zone.clone())
            .date();
        let riyadh = Coordinates::new(24.7136, 46.6753).expect("valid");
        let night = night_on(civil, &zone, riyadh, &|_| Parameters::default(), times);
        let screen = Screen {
            zone: &zone,
            config,
            times,
            night: &night,
            // Near enough for a test: tomorrow's Fajr a day after today's.
            next: next_from(
                now,
                times,
                || None,
                || Some(times.fajr().unix_seconds().saturating_add(86_400)),
            ),
            color,
        };
        let mut out = Vec::new();
        screen.write(&mut out).expect("written");
        String::from_utf8(out).expect("utf-8")
    }

    fn styled_at(now: i64, config: &Config) -> String {
        let text = written(&riyadh(), now, config, true);
        let mut plain = String::new();
        let mut chars = text.chars();
        while let Some(c) = chars.next() {
            if c == '\x1b' {
                let _end = chars.by_ref().find(|&end| end == 'm');
            } else {
                plain.push(c);
            }
        }
        plain
    }

    #[test]
    fn the_screen_leads_with_the_next_prayer_and_marks_it() {
        let times = riyadh();
        let config = Config::default();
        let before_asr = times
            .get(Prayer::Asr)
            .unix_seconds()
            .saturating_sub(83 * 60);
        let screen = styled_at(before_asr, &config);
        let lines: Vec<&str> = screen.lines().collect();
        assert_eq!(lines.first(), Some(&""), "{screen}");
        assert_eq!(lines.get(1), Some(&"  Asr in 1h 23m"), "{screen}");
        assert_eq!(
            lines.iter().filter(|line| line.starts_with("  ▸ ")).count(),
            1,
            "{screen}"
        );
        assert!(screen.contains("  ▸ Asr         2:51 PM"), "{screen}");
        assert_eq!(
            lines.last(),
            Some(&"    Midnight   10:58 PM"),
            "no blank line at the end"
        );
    }

    #[test]
    fn a_pipe_gets_the_time_left_after_the_next_prayer_only() {
        let times = riyadh();
        let config = Config::default();
        let before_asr = times
            .get(Prayer::Asr)
            .unix_seconds()
            .saturating_sub(83 * 60);
        let plain = written(&times, before_asr, &config, false);
        let counted: Vec<&str> = plain.lines().filter(|line| line.contains(" in ")).collect();
        assert_eq!(counted, ["  Asr         2:51 PM   in 1h 23m"], "{plain}");

        let after_isha = times.get(Prayer::Isha).unix_seconds().saturating_add(60);
        let night = written(&times, after_isha, &config, false);
        let tomorrow: Vec<&str> = night.lines().filter(|line| line.contains(" in ")).collect();
        assert_eq!(
            tomorrow,
            ["  Fajr        4:40 AM   tomorrow, in 9h 53m"],
            "{night}"
        );
    }

    #[test]
    fn the_night_takes_its_place_in_the_day_and_stays_gray() {
        let times = riyadh();
        let config = Config::default();
        let before_asr = times
            .get(Prayer::Asr)
            .unix_seconds()
            .saturating_sub(83 * 60);
        let screen = styled_at(before_asr, &config);
        let rows: Vec<&str> = screen.lines().skip(4).collect();
        // The last third of the night before ends the day's small hours; midnight of the
        // day's own night follows Isha. The next night's last third is tomorrow's.
        assert_eq!(
            rows,
            [
                "    Last third 12:52 AM",
                "    Fajr        4:40 AM",
                "    Sunrise     5:58 AM",
                "    Dhuhr      11:38 AM",
                "  ▸ Asr         2:51 PM",
                "    Maghrib     5:16 PM",
                "    Isha        6:46 PM",
                "    Midnight   10:58 PM",
            ],
            "{screen}"
        );

        // Gray whether past or ahead, and never the next prayer.
        let raw = written(&times, before_asr, &config, true);
        assert!(raw.contains(&format!("{DIM}Last third 12:52 AM")), "{raw}");
        assert!(raw.contains(&format!("{DIM}Midnight   10:58 PM")), "{raw}");
        let after_isha = times.get(Prayer::Isha).unix_seconds().saturating_add(60);
        assert_eq!(
            styled_at(after_isha, &config).lines().nth(1),
            Some("  Fajr in 9h 53m  tomorrow 4:40 AM")
        );
    }

    #[test]
    fn without_a_next_fajr_there_is_no_midnight() {
        let last_day = compute(
            Date::new(2100, 12, 31).expect("valid"),
            Coordinates::new(24.7136, 46.6753).expect("valid"),
            Parameters::default(),
        )
        .expect("computable");
        assert_eq!(last_day.midnight(), None);
        let config = Config::default();
        let plain = written(&last_day, 0, &config, false);
        assert!(
            plain.starts_with("  Last third "),
            "the night before still ends: {plain}"
        );
        assert!(!plain.contains("Midnight"), "{plain}");
    }

    #[test]
    fn the_bar_ends_where_the_times_end() {
        let times = riyadh();
        for clock in [Clock::TwelveHour, Clock::TwentyFourHour] {
            let config = Config {
                clock,
                ..Config::default()
            };
            let screen = styled_at(times.get(Prayer::Dhuhr).unix_seconds(), &config);
            let bar = screen.lines().nth(2).expect("the bar");
            let row = screen.lines().last().expect("a row");
            assert_eq!(bar.chars().count(), row.chars().count(), "{screen}");
        }
    }

    #[test]
    fn the_help_lists_every_flag_and_only_real_ones() {
        let listed: Vec<&str> = HELP
            .iter()
            .flat_map(|(_, rows)| rows.iter())
            .filter_map(|(flag, _, _)| flag.split(' ').find_map(|word| word.strip_prefix("--")))
            .collect();
        let mut command = <Args as clap::CommandFactory>::command();
        command.build();
        let real: Vec<&str> = command
            .get_arguments()
            .filter_map(clap::Arg::get_long)
            .collect();
        for flag in &real {
            assert!(listed.contains(flag), "--{flag} is missing from the help");
        }
        for flag in &listed {
            assert!(
                real.contains(flag),
                "the help lists --{flag}, which does not exist"
            );
        }
    }

    /// [`config_path_in`] with only these variables set.
    fn config_path_with(vars: &[(&str, &str)]) -> Option<PathBuf> {
        config_path_in(|name| {
            vars.iter()
                .find(|(key, _)| *key == name)
                .map(|(_, value)| OsString::from(value))
        })
    }

    #[test]
    fn the_config_lives_where_each_platform_expects_it() {
        let config = |base: &str| Some(PathBuf::from(base).join("hya").join("config"));
        let all = [
            ("XDG_CONFIG_HOME", "/xdg"),
            ("HOME", "/home/u"),
            ("APPDATA", "/appdata"),
        ];
        assert_eq!(config_path_with(&all), config("/xdg"), "XDG first");
        assert_eq!(
            config_path_with(all.get(1..).unwrap_or_default()),
            config("/home/u/.config"),
            "then HOME"
        );
        // Windows sets neither XDG_CONFIG_HOME nor HOME.
        assert_eq!(
            config_path_with(&[("APPDATA", "/appdata")]),
            config("/appdata"),
            "then APPDATA"
        );
        assert_eq!(config_path_with(&[]), None, "nowhere to keep it");
    }

    #[test]
    fn empty_or_relative_variables_are_ignored() {
        for unusable in ["", "relative", "./here"] {
            assert_eq!(
                config_path_with(&[("XDG_CONFIG_HOME", unusable), ("HOME", "/home/u")]),
                Some(PathBuf::from("/home/u/.config/hya/config")),
                "XDG_CONFIG_HOME={unusable:?}"
            );
            assert_eq!(
                config_path_with(&[("HOME", unusable), ("APPDATA", "/appdata")]),
                Some(PathBuf::from("/appdata/hya/config")),
                "HOME={unusable:?}"
            );
            assert_eq!(
                config_path_with(&[("APPDATA", unusable)]),
                None,
                "APPDATA={unusable:?}"
            );
        }
    }

    #[test]
    fn time_left_rounds_up_to_the_minute() {
        assert_eq!(left(0, 1), "1m");
        assert_eq!(left(0, 60), "1m");
        assert_eq!(left(0, 61), "2m");
        assert_eq!(left(0, 3600), "1h");
        assert_eq!(left(0, 3600 + 23 * 60), "1h 23m");
        assert_eq!(left(0, 25 * 3600), "25h");
    }

    #[test]
    fn the_bar_fills_with_the_window() {
        assert_eq!(filled(100, 100, 200, 30), 0);
        assert_eq!(filled(100, 150, 200, 30), 15);
        assert_eq!(filled(100, 200, 200, 30), 30);
        assert_eq!(filled(100, 50, 200, 30), 0, "before the window");
        assert_eq!(filled(100, 300, 200, 30), 30, "after the window");
        assert_eq!(filled(100, 100, 100, 30), 0, "an empty window");
    }

    #[test]
    fn the_window_begins_with_the_last_time_passed() {
        let times = riyadh();
        let after_sunrise = times.get(Prayer::Sunrise).unix_seconds();
        let next = next_from(after_sunrise, &times, || None, || None).expect("Dhuhr is left");
        assert_eq!(next.since, Some(after_sunrise));
        let before_fajr = times.get(Prayer::Fajr).unix_seconds().saturating_sub(60);
        let fajr = next_from(before_fajr, &times, || Some(7), || None).expect("Fajr is left");
        assert_eq!(fajr.since, Some(7), "yesterday's Isha");
    }

    fn riyadh() -> PrayerTimes {
        compute(
            Date::new(2026, 10, 29).expect("valid"),
            Coordinates::new(24.7136, 46.6753).expect("valid"),
            Parameters::default(),
        )
        .expect("computable")
    }

    #[test]
    fn sunrise_is_never_next() {
        let times = riyadh();
        let after_fajr = times.get(Prayer::Fajr).unix_seconds();
        let next = next_from(after_fajr, &times, || None, || None).expect("Dhuhr is left");
        assert_eq!(next.prayer, Prayer::Dhuhr);
        assert!(!next.tomorrow);
    }

    #[test]
    fn a_prayer_at_this_very_second_has_begun() {
        let times = riyadh();
        let at_asr = times.get(Prayer::Asr).unix_seconds();
        let next = next_from(at_asr, &times, || None, || None).expect("Maghrib is left");
        assert_eq!(next.prayer, Prayer::Maghrib);
    }

    #[test]
    fn after_isha_the_next_prayer_is_tomorrows_fajr() {
        let times = riyadh();
        let after_isha = times.get(Prayer::Isha).unix_seconds();
        let tomorrow = after_isha.checked_add(36_000).expect("in range");
        let next = next_from(after_isha, &times, || None, || Some(tomorrow)).expect("tomorrow");
        assert_eq!(next.prayer, Prayer::Fajr);
        assert!(next.tomorrow);
        assert_eq!(next.at, tomorrow);
        assert!(
            next_from(after_isha, &times, || None, || None).is_none(),
            "no Fajr tomorrow"
        );
    }
}
