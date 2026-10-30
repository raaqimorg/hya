//! Concurrency tests.
//!
//! The core holds no shared or mutable state, so there are no interleavings for a model checker
//! like `loom` to explore. What can still go wrong is a type losing `Send`/`Sync` (for example by
//! adding a `Cell` or `Rc`), or hidden global state making results thread-dependent. Both are
//! checked here.

#![expect(
    clippy::expect_used,
    clippy::indexing_slicing,
    reason = "test-only helpers; a failed precondition should abort the test loudly"
)]

use std::thread;

use hya_engine::{
    ComputeError, Coordinates, Date, Instant, InvalidInput, Method, Parameters, PrayerTimes,
    compute,
};

const fn assert_thread_safe<T: Send + Sync + Copy + 'static>() {}

#[test]
fn public_types_are_thread_safe_values() {
    assert_thread_safe::<Coordinates>();
    assert_thread_safe::<Date>();
    assert_thread_safe::<Parameters>();
    assert_thread_safe::<PrayerTimes>();
    assert_thread_safe::<Instant>();
    assert_thread_safe::<ComputeError>();
    assert_thread_safe::<InvalidInput>();
}

fn workload(seed: i32) -> Vec<Result<PrayerTimes, ComputeError>> {
    let mut out = Vec::new();
    for step in 0_i32..400_i32 {
        let day = seed
            .wrapping_mul(7919)
            .wrapping_add(step.wrapping_mul(13))
            .rem_euclid(73_000);
        let date = Date::from_days_since_unix_epoch(day.wrapping_sub(25_567)).expect("in range");
        let lat = f64::from(step.rem_euclid(170)) - 85.0_f64;
        let lng = f64::from(seed.wrapping_add(step).rem_euclid(360)) - 180.0_f64;
        let coords = Coordinates::new(lat, lng).expect("in range");
        let method = Method::ALL[usize::try_from(step.rem_euclid(11)).expect("small")];
        out.push(compute(date, coords, method.parameters()));
    }
    out
}

#[test]
fn parallel_results_equal_sequential_results() {
    let seeds: Vec<i32> = (0_i32..32_i32).collect();
    let sequential: Vec<_> = seeds.iter().map(|&seed| workload(seed)).collect();
    let parallel: Vec<_> = thread::scope(|scope| {
        let handles: Vec<_> = seeds
            .iter()
            .map(|&seed| scope.spawn(move || workload(seed)))
            .collect();
        handles
            .into_iter()
            .map(|h| h.join().expect("no panic"))
            .collect()
    });
    assert_eq!(sequential, parallel, "results depend on threading");
}

#[test]
fn shared_inputs_across_threads() {
    let coords = Coordinates::new(24.7136, 46.6753).expect("valid");
    let params = Parameters::default();
    let date = Date::new(2026, 10, 29).expect("valid");
    let expected = compute(date, coords, params);
    thread::scope(|scope| {
        for _ in 0_i32..16_i32 {
            let handle = scope.spawn(|| {
                (0_i32..1000_i32)
                    .map(|_| compute(date, coords, params))
                    .collect::<Vec<_>>()
            });
            for result in handle.join().expect("no panic") {
                assert_eq!(result, expected, "diverged");
            }
        }
    });
}
