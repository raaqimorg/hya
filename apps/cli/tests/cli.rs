//! End to end, against the built binary, each test with its own throwaway config directory.

#![expect(
    clippy::expect_used,
    reason = "test-only helpers; a failed precondition should abort the test loudly"
)]

use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::{env, fs, process};

use hya_engine::{Coordinates, Date, Method, Prayer, compute};
use jiff::Timestamp;
use jiff::tz::TimeZone;

/// A fresh config directory per test, so tests neither share state nor touch the user's.
fn config_dir(name: &str) -> PathBuf {
    let dir = env::temp_dir().join(format!("hya-cli-{name}-{}", process::id()));
    drop(fs::remove_dir_all(&dir));
    fs::create_dir_all(&dir).expect("temp dir");
    dir
}

fn hya(dir: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_hya"))
        .env("XDG_CONFIG_HOME", dir)
        .args(args)
        .output()
        .expect("runs")
}

/// [`hya`] on a system whose time zone is `zone`.
fn hya_in_zone(dir: &Path, zone: &str, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_hya"))
        .env("XDG_CONFIG_HOME", dir)
        .env("TZ", zone)
        .args(args)
        .output()
        .expect("runs")
}

/// The minute of the day a time is printed at, from a 24-hour line such as `  Isha  19:59`.
/// On today's date the next prayer's line goes on with the time left (`19:59   in 45m`), so
/// this reads the time itself, not the line's last word.
fn minute_of(stdout: &str, name: &str) -> i32 {
    let line = stdout
        .lines()
        .find(|line| line.split_whitespace().next() == Some(name))
        .expect("the time is printed");
    let (hours, minutes) = line
        .split_whitespace()
        .find_map(|word| word.split_once(':'))
        .expect("HH:MM");
    hours
        .parse::<i32>()
        .ok()
        .and_then(|hours| hours.checked_mul(60))
        .and_then(|hours| hours.checked_add(minutes.parse().ok()?))
        .expect("HH:MM")
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8(bytes.to_vec()).expect("utf-8")
}

/// The name the CLI prints for a time.
fn name(prayer: Prayer) -> String {
    let id = prayer.id();
    id.get(..1).unwrap_or_default().to_uppercase() + id.get(1..).unwrap_or_default()
}

fn saved_config(dir: &Path) -> PathBuf {
    dir.join("hya").join("config")
}

const AMMAN: [&str; 8] = [
    "--lat",
    "31.95",
    "--lng",
    "35.91",
    "--tz",
    "Asia/Amman",
    "--date",
    "2026-11-30",
];

// ---------- the civil date ----------

#[test]
fn apia_gets_the_schedule_of_its_own_civil_day() {
    let dir = config_dir("apia");
    let output = hya(
        &dir,
        &[
            "--lat",
            "-13.83",
            "--lng",
            "-171.76",
            "--tz",
            "Pacific/Apia",
            "--date",
            "2026-10-30",
            "--method",
            "muslim_world_league",
            "--clock",
            "24",
        ],
    );
    assert!(output.status.success(), "{}", text(&output.stderr));
    let stdout = text(&output.stdout);

    // Apia is UTC+13 at 171.76°W, so 30 October there is the solar day of 29 October.
    let times = compute(
        Date::new(2026, 10, 29).expect("valid"),
        Coordinates::new(-13.83, -171.76).expect("valid"),
        Method::MuslimWorldLeague.parameters(),
    )
    .expect("computable");
    let zone = TimeZone::get("Pacific/Apia").expect("zone");
    for prayer in Prayer::ALL {
        let local = Timestamp::from_second(times.get(prayer).unix_seconds())
            .expect("in range")
            .to_zoned(zone.clone());
        assert_eq!(local.date().to_string(), "2026-10-30", "{prayer} civil day");
        let line = format!("{:<10} {}", name(prayer), local.strftime("%H:%M"));
        assert!(stdout.contains(&line), "{prayer}: {stdout}");
    }
}

// ---------- --country ----------

#[test]
fn a_listed_country_selects_its_method_and_says_why() {
    let dir = config_dir("listed");
    let output = hya(&dir, &[AMMAN.as_slice(), &["--country", "jo"]].concat());
    assert!(output.status.success(), "{}", text(&output.stderr));
    let stderr = text(&output.stderr);
    assert!(
        stderr.contains("note: JO uses Awqaf Jordan, per Ministry of Awqaf"),
        "{stderr}"
    );
}

#[test]
fn an_unlisted_country_is_an_error_not_a_guess() {
    let dir = config_dir("unlisted");
    let output = hya(&dir, &[AMMAN.as_slice(), &["--country", "GB"]].concat());
    assert!(!output.status.success());
    let stderr = text(&output.stderr);
    assert!(
        stderr.contains("no single method is on record for GB"),
        "{stderr}"
    );
    assert!(!saved_config(&dir).exists(), "nothing saved");
}

#[test]
fn malformed_codes_and_conflicting_flags_are_rejected() {
    let dir = config_dir("rejected");
    for args in [
        ["--country", "SAU"].as_slice(),
        ["--country", "S1"].as_slice(),
        ["--country", "SA", "--method", "karachi"].as_slice(),
    ] {
        let output = hya(&dir, &[AMMAN.as_slice(), args].concat());
        assert!(!output.status.success(), "{args:?} accepted");
    }
    assert!(!saved_config(&dir).exists(), "nothing saved");
}

// ---------- saving, prompting, and streams ----------

#[test]
fn flags_apply_to_one_run_unless_saved() {
    let dir = config_dir("save");
    let once = hya(&dir, &[AMMAN.as_slice(), &["--method", "karachi"]].concat());
    assert!(once.status.success(), "{}", text(&once.stderr));
    assert!(!saved_config(&dir).exists(), "nothing saved without --save");

    let saved = hya(
        &dir,
        &[AMMAN.as_slice(), &["--method", "karachi", "--save"]].concat(),
    );
    assert!(saved.status.success(), "{}", text(&saved.stderr));
    let file = fs::read_to_string(saved_config(&dir)).expect("saved");
    assert!(file.contains("method = karachi"), "{file}");

    let later = hya(&dir, &["--tz", "Asia/Amman", "--date", "2026-11-30"]);
    assert!(later.status.success(), "{}", text(&later.stderr));
    assert_eq!(later.stdout, saved.stdout, "saved default used");
    assert_ne!(
        later.stdout,
        hya(&config_dir("save-default"), &AMMAN).stdout,
        "not the default method"
    );
}

#[test]
fn without_a_terminal_a_missing_location_is_an_error_not_a_prompt() {
    let dir = config_dir("no-tty");
    let output = hya(&dir, &["--date", "2026-11-30"]);
    assert!(!output.status.success(), "must not wait for input");
    assert!(text(&output.stdout).is_empty(), "no prompt on stdout");
    let stderr = text(&output.stderr);
    assert!(stderr.contains("pass --lat and --lng"), "{stderr}");
}

#[test]
fn stdout_carries_only_the_times() {
    let dir = config_dir("streams");
    // Oslo in June: the high-latitude note goes to stderr.
    let output = hya(
        &dir,
        &[
            "--lat",
            "59.91",
            "--lng",
            "10.75",
            "--tz",
            "Europe/Oslo",
            "--date",
            "2026-06-21",
            "--method",
            "muslim_world_league",
        ],
    );
    assert!(output.status.success(), "{}", text(&output.stderr));
    let stdout = text(&output.stdout);
    assert_eq!(
        stdout.lines().count(),
        8,
        "six times, midnight, last third: {stdout}"
    );
    assert!(!stdout.contains("note"), "{stdout}");
    assert!(!stdout.contains('\x1b'), "no colour in a pipe: {stdout}");
    assert!(
        text(&output.stderr).contains("note: the high-latitude rule sets"),
        "{}",
        text(&output.stderr)
    );
}

#[test]
fn the_clock_is_twelve_hour_unless_a_twenty_four_hour_one_is_saved() {
    let dir = config_dir("clock");
    let first = hya(&dir, &AMMAN);
    assert!(first.status.success(), "{}", text(&first.stderr));
    let stdout = text(&first.stdout);
    assert!(
        stdout.contains(" AM\n") && stdout.contains(" PM\n"),
        "{stdout}"
    );

    let saved = hya(
        &dir,
        &[AMMAN.as_slice(), &["--clock", "24", "--save"]].concat(),
    );
    assert!(saved.status.success(), "{}", text(&saved.stderr));
    assert!(!text(&saved.stdout).contains(" PM"), "24-hour");

    let later = hya(&dir, &["--tz", "Asia/Amman", "--date", "2026-11-30"]);
    assert!(later.status.success(), "{}", text(&later.stderr));
    assert!(!text(&later.stdout).contains(" PM"), "saved clock used");

    let once = hya(
        &dir,
        &[
            "--tz",
            "Asia/Amman",
            "--date",
            "2026-11-30",
            "--clock",
            "12",
        ],
    );
    assert!(
        text(&once.stdout).contains(" PM\n"),
        "a flag overrides the saved clock"
    );
}

// ---------- the countdown ----------

#[test]
fn another_day_has_no_countdown() {
    let dir = config_dir("other-day");
    let output = hya(&dir, &AMMAN);
    assert!(output.status.success(), "{}", text(&output.stderr));
    let stdout = text(&output.stdout);
    assert_eq!(stdout.lines().count(), 8, "{stdout}");
    assert!(!stdout.contains(" in "), "{stdout}");
}

#[test]
fn today_names_the_next_prayer_and_the_time_left() {
    let dir = config_dir("next");
    let output = hya(
        &dir,
        &[
            "--lat",
            "31.95",
            "--lng",
            "35.91",
            "--tz",
            "Asia/Amman",
            "--next",
        ],
    );
    assert!(output.status.success(), "{}", text(&output.stderr));
    let stdout = text(&output.stdout);
    assert_eq!(stdout.lines().count(), 1, "{stdout}");
    assert!(stdout.contains(" in "), "{stdout}");
    assert!(
        ["Fajr", "Dhuhr", "Asr", "Maghrib", "Isha"]
            .iter()
            .any(|prayer| stdout.starts_with(prayer)),
        "{stdout}"
    );
}

#[test]
fn next_is_only_for_today() {
    let dir = config_dir("next-date");
    let output = hya(&dir, &[AMMAN.as_slice(), &["--next"]].concat());
    assert!(!output.status.success(), "--next with --date accepted");
}

// ---------- what a failed run leaves behind ----------

#[test]
fn a_failed_run_saves_nothing() {
    let dir = config_dir("failed-save");
    for args in [
        ["--tz", "Nope/Zone"].as_slice(),
        ["--tz", "UTC", "--date", "1800-01-01"].as_slice(),
    ] {
        let output = hya(
            &dir,
            &[
                &[
                    "--lat", "10", "--lng", "20", "--method", "karachi", "--save",
                ],
                args,
            ]
            .concat(),
        );
        assert!(!output.status.success(), "{args:?} accepted");
        assert!(!saved_config(&dir).exists(), "{args:?} saved a config");
    }
}

#[test]
fn coordinates_must_be_finite_numbers() {
    let dir = config_dir("finite");
    for (lat, lng) in [("nan", "0"), ("0", "inf"), ("1e400", "0")] {
        let output = hya(&dir, &["--lat", lat, "--lng", lng]);
        assert!(!output.status.success(), "{lat} {lng} accepted");
        let stderr = text(&output.stderr);
        assert!(stderr.contains("expected a finite number"), "{stderr}");
    }
}

// ---------- time zones ----------

#[test]
fn without_tz_the_system_zone_is_used() {
    let dir = config_dir("system-zone");
    let place = ["--lat", "31.95", "--lng", "35.91", "--date", "2026-11-30"];
    let system = hya_in_zone(&dir, "Asia/Amman", &place);
    assert!(system.status.success(), "{}", text(&system.stderr));
    assert_eq!(
        system.stdout,
        hya(&dir, &AMMAN).stdout,
        "same as --tz Asia/Amman"
    );
    assert_ne!(
        system.stdout,
        hya_in_zone(&dir, "Asia/Tokyo", &place).stdout,
        "another system zone, other wall-clock times"
    );
}

#[test]
fn a_daylight_saving_change_moves_the_clock_not_the_sun() {
    // Toronto springs forward at 02:00 on 8 March 2026 and falls back at 02:00 on 1 November.
    let dir = config_dir("dst");
    let toronto = Coordinates::new(43.6532, -79.3832).expect("valid");
    let zone = TimeZone::get("America/Toronto").expect("zone");
    let run = |date: &str| {
        let output = hya(
            &dir,
            &[
                "--lat",
                "43.6532",
                "--lng",
                "-79.3832",
                "--tz",
                "America/Toronto",
                "--method",
                "north_america",
                "--clock",
                "24",
                "--date",
                date,
            ],
        );
        assert!(output.status.success(), "{}", text(&output.stderr));
        text(&output.stdout)
    };
    for (before, change, (month, day), shift) in [
        ("2026-03-07", "2026-03-08", (3_u8, 8_u8), 60_i32),
        ("2026-10-31", "2026-11-01", (11_u8, 1_u8), -60_i32),
    ] {
        let stdout = run(change);
        let times = compute(
            Date::new(2026, month, day).expect("valid"),
            toronto,
            Method::NorthAmerica.parameters(),
        )
        .expect("computable");
        for prayer in Prayer::ALL {
            let local = Timestamp::from_second(times.get(prayer).unix_seconds())
                .expect("in range")
                .to_zoned(zone.clone());
            let line = format!("{:<10} {}", name(prayer), local.strftime("%H:%M"));
            assert!(stdout.contains(&line), "{change} {prayer}: {stdout}");
        }
        // The sun crosses the meridian within a minute of yesterday's time; the clock jumps.
        let jump = minute_of(&stdout, "Dhuhr")
            .checked_sub(minute_of(&run(before), "Dhuhr"))
            .expect("small");
        assert!(
            jump.abs_diff(shift) <= 1,
            "{change}: Dhuhr moved {jump} min"
        );
    }
}

// ---------- method options ----------

#[test]
fn an_announced_ramadan_replaces_the_calculated_one() {
    // The calculated Ramadan 1448 runs 8 February to 8 March 2027. Announced a day later,
    // 8 February drops out and 10 March comes in, with Umm al-Qura's 120-minute Isha.
    let dir = config_dir("ramadan");
    let makkah = [
        "--lat",
        "21.4225",
        "--lng",
        "39.8262",
        "--tz",
        "Asia/Riyadh",
        "--clock",
        "24",
    ];
    let announced = ["--ramadan", "2027-02-09..2027-03-10"];
    let isha_after_maghrib = |args: &[&str]| {
        let output = hya(&dir, &[makkah.as_slice(), args].concat());
        assert!(output.status.success(), "{}", text(&output.stderr));
        let stdout = text(&output.stdout);
        let gap = minute_of(&stdout, "Isha") - minute_of(&stdout, "Maghrib");
        (
            gap,
            text(&output.stderr).contains("Isha uses the Ramadan interval"),
        )
    };
    assert_eq!(
        isha_after_maghrib(&["--date", "2027-03-10"]),
        (90_i32, false)
    );
    assert_eq!(
        isha_after_maghrib(&[&["--date", "2027-03-10"], announced.as_slice()].concat()),
        (120_i32, true)
    );
    assert_eq!(
        isha_after_maghrib(&["--date", "2027-02-08"]),
        (120_i32, true)
    );
    assert_eq!(
        isha_after_maghrib(&[&["--date", "2027-02-08"], announced.as_slice()].concat()),
        (90_i32, false)
    );

    let wrong = hya(
        &dir,
        &[makkah.as_slice(), &["--ramadan", "2027-06-01..2027-06-30"]].concat(),
    );
    assert!(!wrong.status.success(), "a Ramadan in June");
}

#[test]
fn shafaq_is_refused_outside_moonsighting_committee() {
    let dir = config_dir("shafaq");
    let refused = hya(
        &dir,
        &[AMMAN.as_slice(), &["--shafaq", "red", "--save"]].concat(),
    );
    assert!(!refused.status.success(), "accepted");
    let stderr = text(&refused.stderr);
    assert!(
        stderr.contains("--shafaq only applies to moonsighting_committee"),
        "{stderr}"
    );
    assert!(!saved_config(&dir).exists(), "saved anyway");

    // In London in January, the red glow fades first and the white one last.
    let london = [
        "--lat",
        "51.5074",
        "--lng",
        "-0.1278",
        "--tz",
        "Europe/London",
        "--date",
        "2026-01-15",
        "--clock",
        "24",
        "--method",
        "moonsighting_committee",
    ];
    let isha = |shafaq: &str| {
        let output = hya(&dir, &[london.as_slice(), &["--shafaq", shafaq]].concat());
        assert!(output.status.success(), "{}", text(&output.stderr));
        minute_of(&text(&output.stdout), "Isha")
    };
    let (red, general, white) = (isha("red"), isha("general"), isha("white"));
    assert!(red < general && general < white, "{red} {general} {white}");
}

#[test]
fn hanafi_asr_comes_later() {
    let dir = config_dir("asr");
    let asr = |rule: &str| {
        let output = hya(
            &dir,
            &[AMMAN.as_slice(), &["--clock", "24", "--asr", rule]].concat(),
        );
        assert!(output.status.success(), "{}", text(&output.stderr));
        minute_of(&text(&output.stdout), "Asr")
    };
    // Twice the shadow takes most of an hour longer to reach in Amman's late autumn.
    let later = asr("hanafi").checked_sub(asr("standard")).expect("small");
    assert!((30_i32..=60_i32).contains(&later), "{later} min");
}

#[test]
fn polar_day_is_an_error_not_a_schedule() {
    let dir = config_dir("polar");
    let output = hya(
        &dir,
        &[
            "--lat",
            "78.22",
            "--lng",
            "15.65",
            "--tz",
            "Arctic/Longyearbyen",
            "--date",
            "2026-06-21",
        ],
    );
    assert!(!output.status.success(), "times on a day with no sunset");
    assert!(text(&output.stdout).is_empty(), "{}", text(&output.stdout));
    let stderr = text(&output.stderr);
    assert!(
        stderr.contains("the sun does not rise or set on this day"),
        "{stderr}"
    );
}

// ---------- the saved config over time ----------

#[test]
fn reset_forgets_the_saved_defaults() {
    let dir = config_dir("reset");
    let saved = hya(&dir, &[AMMAN.as_slice(), &["--save"]].concat());
    assert!(saved.status.success(), "{}", text(&saved.stderr));
    assert!(saved_config(&dir).exists());

    let reset = hya(&dir, &["--reset"]);
    assert!(reset.status.success(), "{}", text(&reset.stderr));
    assert!(
        text(&reset.stderr).starts_with("removed "),
        "{}",
        text(&reset.stderr)
    );
    assert_eq!(text(&reset.stdout), "");
    assert!(!saved_config(&dir).exists(), "still there");

    let again = hya(&dir, &["--reset"]);
    assert!(again.status.success(), "{}", text(&again.stderr));
    assert!(
        text(&again.stderr).starts_with("nothing saved at "),
        "{}",
        text(&again.stderr)
    );

    let mixed = hya(&dir, &[AMMAN.as_slice(), &["--reset"]].concat());
    assert!(!mixed.status.success(), "--reset with other flags");
}

#[test]
fn a_config_this_version_cannot_read_points_to_reset() {
    let dir = config_dir("broken");
    fs::create_dir_all(dir.join("hya")).expect("dir");
    fs::write(saved_config(&dir), "clock = 13\n").expect("written");
    let broken = hya(&dir, &AMMAN);
    assert!(!broken.status.success(), "a bad value accepted");
    let stderr = text(&broken.stderr);
    assert!(stderr.contains("invalid value for `clock`"), "{stderr}");
    assert!(stderr.contains("hya --reset"), "{stderr}");

    let reset = hya(&dir, &["--reset"]);
    assert!(reset.status.success(), "reset must work on a broken file");
    assert!(hya(&dir, &AMMAN).status.success());
}

#[test]
fn settings_from_a_newer_version_are_skipped_with_a_note() {
    let dir = config_dir("newer");
    fs::create_dir_all(dir.join("hya")).expect("dir");
    fs::write(saved_config(&dir), "clock = 24\nnotify = 10m\n").expect("written");
    let output = hya(&dir, &AMMAN);
    assert!(output.status.success(), "{}", text(&output.stderr));
    let stderr = text(&output.stderr);
    assert!(
        stderr.contains("skipping unknown setting `notify`"),
        "{stderr}"
    );
    assert!(
        !text(&output.stdout).contains(" AM"),
        "the known clock setting still applies"
    );
}

#[test]
fn a_config_path_that_cannot_be_read_or_removed_is_an_error() {
    let dir = config_dir("unreadable");
    // A directory where the file should be: neither readable as text nor removable as a file.
    fs::create_dir_all(saved_config(&dir)).expect("dir");
    for args in [AMMAN.as_slice(), ["--reset"].as_slice()] {
        let output = hya(&dir, args);
        assert!(!output.status.success(), "{args:?} succeeded");
        let stderr = text(&output.stderr);
        assert!(stderr.contains("config"), "names the path: {stderr}");
    }
}

#[test]
fn the_help_ends_with_the_real_config_path_alone() {
    let dir = config_dir("help-path");
    let output = hya(&dir, &["--help"]);
    assert!(output.status.success(), "{}", text(&output.stderr));
    let stdout = text(&output.stdout);
    let last = stdout.trim_end().lines().last().expect("a line");
    let path = saved_config(&dir);
    assert_eq!(
        last,
        format!("Saved defaults: {}", path.display()),
        "copyable as is"
    );
}
