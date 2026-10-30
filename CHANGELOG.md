# Changelog

All notable changes are listed here, newest first. The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and versions follow [Semantic Versioning](https://semver.org/spec/v2.0.0.html). Before 1.0, a minor version may change the public API.

## [Unreleased]

## [0.2.0] - 2026-10-31

### Added

- `hya --next` prints one line, such as `Asr 2:51 PM in 1h 23m`, for status bars.
- `hya` lists Midnight and the Last third of the night in gray, in time order among the prayers.
- `hya --reset` deletes the saved defaults, even a file this version cannot read.

### Changed

- **Breaking:** the night runs from Maghrib to the next day's Fajr, as adhan-js's `SunnahTimes` and common practice have it, not from sunset to the next sunrise. Midnight and the last third move earlier by up to an hour or so, and now agree with adhan-js within a minute in every golden case.
- **Breaking:** `PrayerTimes::midnight` and `last_third` return `Option<Instant>`, and `midnight` and `lastThird` in JavaScript are `Date | null`. They are absent when the next day has no Fajr: at a polar edge, with no high-latitude rule, or on 31 December 2100.
- In a terminal, `hya` leads with the next prayer and the time left, draws a bar for how much of the current window has passed, and dims past times. It no longer prints the date, method, and place. A pipe gets plain lines.
- `hya` prints a 12-hour clock by default. `--clock 24` switches, and `--save` keeps it.
- Notes name methods as people know them (Umm al-Qura, not umm_al_qura).
- `--save` writes only the settings that differ from the default, so a later change of default reaches everyone who never chose otherwise.
- `hya --help` groups the flags by what they change, in aligned columns with their defaults.
- An unknown setting in the config file, perhaps from a newer version, is skipped with a note instead of stopping `hya`. An unreadable value still stops it, and the error suggests `hya --reset`.

### Fixed

- A run that fails, such as on an unknown `--tz`, no longer saves its flags.
- The location prompt asks again after an answer it cannot read, instead of exiting.
- `--lat nan` and `--lng inf` are rejected by name, like any other bad flag value.
- `--help` ends with the real config path, with no full stop to copy along with it.

## [0.1.1] - 2026-10-30

### Changed

- README: a shorter introduction, separate Install and Usage sections, a table of CLI flags, and crates.io and npm version badges.
- Package descriptions on crates.io and npm.

## [0.1.0] - 2026-10-30

The first release.

### Added

- `hya-engine`: the calculation, as one pure function. It is `no_std`, does no I/O and no allocation, and validates every input through its types.
- 15 methods, including Umm al-Qura, MUIS (Singapore), JAKIM (Malaysia) and the Ministry of Awqaf (Jordan), each tested against the authority's own published timetable.
- High-latitude rules, Umm al-Qura's Ramadan Isha from an offline calendar for 1900–2100, announced Ramadan dates, and per-time minute adjustments.
- `hya-countries`: a country's own authority's method, for 11 countries.
- `@raaqimorg/hya`: the JavaScript package, with the WebAssembly inlined and no network or async start-up.
- `hya-cli`: the `hya` command, with saved defaults (`--save`), `--country`, `--clock 12`, and times on stdout and notes on stderr.

[Unreleased]: https://github.com/raaqimorg/hya/compare/v0.2.0...HEAD
[0.2.0]: https://github.com/raaqimorg/hya/compare/v0.1.1...v0.2.0
[0.1.1]: https://github.com/raaqimorg/hya/compare/v0.1.0...v0.1.1
[0.1.0]: https://github.com/raaqimorg/hya/releases/tag/v0.1.0
