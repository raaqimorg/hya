//! Kani proofs: parsing and lookup are total over every input.

use crate::{CountryCode, entry_for};

/// Every byte string of up to three bytes: parsing never panics, and accepts exactly two ASCII
/// letters, stored uppercase. (`CountryCode::new` is `from_bytes` on the string's bytes.)
#[kani::proof]
fn parsing_accepts_exactly_two_ascii_letters() {
    let bytes: [u8; 3] = kani::any();
    let length: usize = kani::any();
    kani::assume(length <= 3);
    let Some(prefix) = bytes.get(..length) else {
        return;
    };
    let parsed = CountryCode::from_bytes(prefix);
    let shape = matches!(prefix, [a, b] if a.is_ascii_alphabetic() && b.is_ascii_alphabetic());
    assert_eq!(parsed.is_ok(), shape);
    if let Ok(code) = parsed {
        let [a, b] = code.letters();
        assert!(a.is_ascii_uppercase() && b.is_ascii_uppercase());
    }
}

/// Lookup never panics for any code, and is case-blind.
#[kani::proof]
#[kani::unwind(6)]
fn lookup_is_total_and_case_blind() {
    let [first, second]: [u8; 2] = kani::any();
    let Ok(code) = CountryCode::from_bytes(&[first, second]) else {
        return;
    };
    let lower = [first.to_ascii_lowercase(), second.to_ascii_lowercase()];
    let Ok(lower_code) = CountryCode::from_bytes(&lower) else {
        return;
    };
    assert_eq!(
        entry_for(code).map(|entry| entry.method),
        entry_for(lower_code).map(|entry| entry.method)
    );
}
