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

fn text(bytes: &[u8]) -> String {
    String::from_utf8(bytes.to_vec()).expect("utf-8")
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
        let line = format!("{:<8} {}", prayer.id(), local.strftime("%H:%M"));
        assert!(stdout.contains(&line), "{prayer}: {stdout}");
    }
}

// ---------- --country ----------

#[test]
fn a_listed_country_selects_its_method_and_says_why() {
    let dir = config_dir("listed");
    let output = hya(&dir, &[AMMAN.as_slice(), &["--country", "jo"]].concat());
    assert!(output.status.success(), "{}", text(&output.stderr));
    assert!(
        text(&output.stdout).contains("jordan"),
        "method in the header"
    );
    let stderr = text(&output.stderr);
    assert!(
        stderr.contains("note: JO uses jordan, per Ministry of Awqaf"),
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
    assert!(
        text(&later.stdout).contains("karachi"),
        "saved default used"
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
    assert_eq!(stdout.lines().count(), 7, "header and six times: {stdout}");
    assert!(!stdout.contains("note"), "{stdout}");
    assert!(text(&output.stderr).contains("note: high-latitude rule applied"));
}

#[test]
fn a_twelve_hour_clock_is_kept_once_saved() {
    let dir = config_dir("clock");
    let saved = hya(
        &dir,
        &[AMMAN.as_slice(), &["--clock", "12", "--save"]].concat(),
    );
    assert!(saved.status.success(), "{}", text(&saved.stderr));
    let stdout = text(&saved.stdout);
    assert!(
        stdout.contains(" AM\n") && stdout.contains(" PM\n"),
        "{stdout}"
    );

    let later = hya(&dir, &["--tz", "Asia/Amman", "--date", "2026-11-30"]);
    assert!(later.status.success(), "{}", text(&later.stderr));
    assert!(text(&later.stdout).contains(" PM\n"), "saved default used");

    let once = hya(
        &dir,
        &[
            "--tz",
            "Asia/Amman",
            "--date",
            "2026-11-30",
            "--clock",
            "24",
        ],
    );
    assert!(
        !text(&once.stdout).contains(" PM"),
        "a flag overrides the saved clock"
    );
}
