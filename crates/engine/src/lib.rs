//! Pure, offline Islamic prayer-time calculation.
//!
//! This crate is the core every product (CLI, web, API, widget) is built on. It is a pure
//! function from validated inputs to validated outputs:
//!
//! - **No I/O.** It never reads the clock, files, the network, or the environment. The date is
//!   an input; the caller decides what "today" is.
//! - **No allocation.** `no_std`, no `alloc`. Runs on servers, in WebAssembly, and on
//!   microcontrollers.
//! - **No panics.** Every fallible step returns a typed error; the workspace lints deny
//!   `unwrap`, `expect`, indexing, `as` casts, and unchecked integer arithmetic.
//! - **No invalid values.** Inputs are branded types ([`Latitude`], [`Longitude`], [`Date`], …)
//!   that can only be built through checked constructors.
//! - **No time zones.** Results are UTC [`Instant`]s; formatting into a local zone is the
//!   caller's job.
//!
//! ```
//! use hya_engine::{compute, Coordinates, Date, Parameters};
//!
//! let riyadh = Coordinates::new(24.7136, 46.6753)?;
//! let date = Date::new(2026, 10, 29)?;
//! let times = compute(date, riyadh, Parameters::default())?; // Umm al-Qura
//! assert!(times.fajr() < times.sunrise());
//! # Ok::<(), Box<dyn core::error::Error>>(())
//! ```

#![no_std]

#[cfg(test)]
extern crate std;

mod compute;
mod date;
mod input;
mod method;
mod ramadan;
mod ramadan_table;
mod seasonal;
mod solar;

#[cfg(kani)]
mod verification;

pub use compute::{ComputeError, Notes, Prayer, PrayerTimes, compute};
pub use date::{Date, Instant};
pub use input::{
    Coordinates, Elevation, InvalidInput, IshaInterval, Latitude, Longitude, MinuteAdjustment,
    TwilightAngle,
};
pub use method::{
    Adjustments, AsrShadow, DhuhrRule, HighLatitudeRule, IshaRule, MaghribRule, Method, Parameters,
    Rounding, Shafaq, UnknownMethod,
};
pub use ramadan::{AnnouncedRamadan, RamadanDates, is_ramadan};
