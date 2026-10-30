//! `hya`: offline prayer times in the terminal.
//!
//! All I/O lives here: arguments, the config file, the clock, the system time zone, the prompt,
//! and output. The calculation is `hya_engine::compute`.

mod config;

use std::io::{self, BufRead as _, IsTerminal as _, Write as _};
use std::path::PathBuf;
use std::process::ExitCode;
use std::{env, fs};

use clap::Parser;
use clap::builder::{PossibleValuesParser, TypedValueParser as _};
use config::{Clock, Config, parse_asr, parse_clock, parse_location, parse_ramadan, parse_shafaq};
use hya_countries::{CountryCode, Entry};
use hya_engine::{
    AnnouncedRamadan, AsrShadow, Coordinates, Date, HighLatitudeRule, Method, Parameters, Prayer,
    PrayerTimes, Shafaq, compute,
};
use jiff::tz::TimeZone;
use jiff::{Timestamp, Zoned};

/// Offline Islamic prayer times.
#[derive(Debug, Parser)]
#[command(
    name = "hya",
    version,
    about,
    after_help = "Defaults are kept in $XDG_CONFIG_HOME/hya/config (usually ~/.config/hya/config,\n\
                  %APPDATA%\\hya\\config on Windows). With no saved location, a run in a terminal\n\
                  asks for one and saves it."
)]
struct Args {
    /// Latitude in degrees (north positive).
    #[arg(long, allow_hyphen_values = true, requires = "lng")]
    lat: Option<f64>,
    /// Longitude in degrees (east positive).
    #[arg(long, allow_hyphen_values = true, requires = "lat")]
    lng: Option<f64>,
    /// Calculation method.
    #[arg(long, value_parser = PossibleValuesParser::new(Method::ALL.map(Method::id))
        .try_map(|id| id.parse::<Method>()))]
    method: Option<Method>,
    /// Use the method of this country's own authority, e.g. SA, MY, JO. Errors where no
    /// single method is on record; choose one with --method instead.
    #[arg(long, conflicts_with = "method")]
    country: Option<CountryCode>,
    /// Asr convention: standard (Shafiʿi, Maliki, Hanbali) or hanafi.
    #[arg(long, value_parser = PossibleValuesParser::new(["standard", "hanafi"])
        .try_map(|id| parse_asr(&id)))]
    asr: Option<AsrShadow>,
    /// Which glow ends Isha under the Moonsighting Committee method: red (Shafiʿi, Maliki,
    /// Hanbali), white (Hanafi), or general, the committee's blend.
    #[arg(long, value_parser = PossibleValuesParser::new(["general", "red", "white"])
        .try_map(|id| parse_shafaq(&id)))]
    shafaq: Option<Shafaq>,
    /// Announced Ramadan as FIRST..LAST, e.g. 2027-02-09..2027-03-10, when the sighting
    /// differs from the Umm al-Qura calendar. Saved ones are kept, one per month.
    #[arg(long, value_parser = parse_ramadan)]
    ramadan: Option<AnnouncedRamadan>,
    /// Date as YYYY-MM-DD (defaults to today in the chosen time zone).
    #[arg(long)]
    date: Option<jiff::civil::Date>,
    /// Print times on a 24-hour clock (18:45) or a 12-hour one (6:45 PM).
    #[arg(long, value_parser = PossibleValuesParser::new(["24", "12"])
        .try_map(|id| parse_clock(&id)))]
    clock: Option<Clock>,
    /// IANA time zone for display, e.g. Asia/Riyadh (defaults to the system zone).
    #[arg(long)]
    tz: Option<String>,
    /// Keep the location and options given here as the defaults for later runs.
    #[arg(long)]
    save: bool,
}

/// `$XDG_CONFIG_HOME/hya/config`, else `~/.config/hya/config`, else (Windows)
/// `%APPDATA%\hya\config`. Empty or relative variables are ignored, as the XDG spec requires,
/// so an unset-but-exported `XDG_CONFIG_HOME=` never writes into the current directory.
fn config_path() -> Option<PathBuf> {
    let absolute = |name: &str| {
        env::var_os(name)
            .map(PathBuf::from)
            .filter(|path| path.is_absolute())
    };
    let base = absolute("XDG_CONFIG_HOME")
        .or_else(|| absolute("HOME").map(|home| home.join(".config")))
        .or_else(|| absolute("APPDATA"))?;
    Some(base.join("hya").join("config"))
}

fn load_config() -> Result<Config, String> {
    let Some(path) = config_path() else {
        return Ok(Config::default());
    };
    match fs::read_to_string(&path) {
        Ok(text) => Config::parse(&text).map_err(|e| format!("{}: {e}", path.display())),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(Config::default()),
        Err(e) => Err(format!("{}: {e}", path.display())),
    }
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
fn ask_location() -> Result<Coordinates, String> {
    if !io::stdin().is_terminal() {
        return Err("no saved location: pass --lat and --lng (add --save to keep them)".into());
    }
    let mut prompt = io::stderr().lock();
    let mut line = String::new();
    writeln!(
        prompt,
        "Where are you? Enter latitude and longitude, e.g. 24.7136 46.6753"
    )
    .map_err(|e| e.to_string())?;
    write!(prompt, "> ")
        .and_then(|()| prompt.flush())
        .map_err(|e| e.to_string())?;
    let _bytes = io::stdin()
        .lock()
        .read_line(&mut line)
        .map_err(|e| e.to_string())?;
    parse_location(&line)
}

fn run() -> Result<(), String> {
    let args = Args::parse();
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
        let location = ask_location()?;
        config.location = Some(location);
        location
    };
    if args.save || asked {
        save_config(&config)?;
    }

    let zone = match &args.tz {
        Some(name) => TimeZone::get(name).map_err(|e| e.to_string())?,
        None => TimeZone::system(),
    };
    let civil = args
        .date
        .unwrap_or_else(|| Zoned::now().with_time_zone(zone.clone()).date());
    let date = Date::new(
        civil.year(),
        u8::try_from(civil.month()).map_err(|e| e.to_string())?,
        u8::try_from(civil.day()).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    let base = config.method.parameters();
    let parameters = Parameters {
        asr: config.asr,
        high_latitude: if seasonal {
            HighLatitudeRule::SeasonAdjusted(config.shafaq)
        } else {
            base.high_latitude
        },
        ramadan: config.ramadan.for_date(date),
        ..base
    };
    let times = schedule_on(civil, &zone, date, location, parameters)?;

    match print_times(date, &zone, &config, &times, chosen_for) {
        // The reader went away (`hya | head -1`): nothing is wrong, and no one is listening.
        Err(e) if e.kind() == io::ErrorKind::BrokenPipe => Ok(()),
        result => result.map_err(|e| e.to_string()),
    }
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

/// The times on stdout, one per line; notes on stderr, so a pipe gets only the times.
fn print_times(
    date: Date,
    zone: &TimeZone,
    config: &Config,
    times: &PrayerTimes,
    chosen_for: Option<Entry>,
) -> io::Result<()> {
    let mut out = io::stdout().lock();
    writeln!(
        out,
        "{date}  {}  ({})",
        config.method,
        zone.iana_name().unwrap_or("local")
    )?;
    for prayer in Prayer::ALL {
        let instant =
            Timestamp::from_second(times.get(prayer).unix_seconds()).map_err(io::Error::other)?;
        let local = instant
            .to_zoned(zone.clone())
            .strftime(config.clock.pattern());
        writeln!(out, "  {:<8} {local}", prayer.id())?;
    }
    out.flush()?;

    let mut notes = io::stderr().lock();
    let flags = times.notes();
    if flags.fajr_high_latitude || flags.isha_high_latitude {
        writeln!(notes, "note: high-latitude rule applied")?;
    }
    if flags.maghrib_at_sunset {
        writeln!(
            notes,
            "note: Maghrib at sunset (the method's angle falls after Isha)"
        )?;
    }
    if flags.ramadan_isha {
        writeln!(notes, "note: Ramadan Isha interval")?;
    }
    if let Some(entry) = chosen_for {
        writeln!(
            notes,
            "note: {} uses {}, per {}",
            entry.country, entry.method, entry.authority
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
