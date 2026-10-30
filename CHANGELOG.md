# Changelog

All notable changes are listed here, newest first. The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and versions follow [Semantic Versioning](https://semver.org/spec/v2.0.0.html). Before 1.0, a minor version may change the public API.

## [Unreleased]

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

[Unreleased]: https://github.com/raaqimorg/hya/compare/v0.1.1...HEAD
[0.1.1]: https://github.com/raaqimorg/hya/compare/v0.1.0...v0.1.1
[0.1.0]: https://github.com/raaqimorg/hya/releases/tag/v0.1.0
