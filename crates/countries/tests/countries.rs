//! The country table and the country-code type.

#![expect(
    clippy::expect_used,
    reason = "test-only helpers; a failed precondition should abort the test loudly"
)]

use hya_countries::{CountryCode, ENTRIES, InvalidCountryCode, entry_for};
use hya_engine::Method;
use proptest::prelude::*;

fn code(text: &str) -> CountryCode {
    CountryCode::new(text).expect("valid code")
}

#[test]
fn entries_are_sorted_unique_and_well_formed() {
    for pair in ENTRIES.windows(2) {
        let [a, b] = pair else {
            continue;
        };
        assert!(a.country < b.country, "{} before {}", a.country, b.country);
    }
    for entry in ENTRIES {
        // Built without `new`, so check each code is one `new` would produce.
        assert_eq!(code(&entry.country.to_string()), entry.country);
        assert!(!entry.authority.is_empty(), "{}", entry.country);
        assert_eq!(entry_for(entry.country), Some(entry));
    }
}

#[test]
fn each_country_gets_its_own_authoritys_method() {
    let expected = [
        ("SA", Method::UmmAlQura),
        ("EG", Method::Egyptian),
        ("PK", Method::Karachi),
        ("AE", Method::Dubai),
        ("KW", Method::Kuwait),
        ("QA", Method::Qatar),
        ("SG", Method::Singapore),
        ("TR", Method::Turkey),
        ("IR", Method::Tehran),
        ("MY", Method::Malaysia),
        ("JO", Method::Jordan),
    ];
    assert_eq!(expected.len(), ENTRIES.len(), "every entry is checked");
    for (country, method) in expected {
        assert_eq!(
            entry_for(code(country)).map(|entry| entry.method),
            Some(method),
            "{country}"
        );
    }
}

#[test]
fn contested_or_unimplemented_customs_are_left_to_the_caller() {
    // Split between methods (UK, North America, Europe), or an authority whose own method the
    // core lacks (Indonesia's Kemenag, Morocco's ministry, Russia's DUM).
    for country in ["GB", "US", "CA", "FR", "DE", "ID", "MA", "RU", "IN", "BD"] {
        assert_eq!(
            entry_for(code(country)).map(|entry| entry.method),
            None,
            "{country}"
        );
    }
}

#[test]
fn error_says_what_a_code_looks_like() {
    let message = InvalidCountryCode.to_string();
    assert!(message.contains("two letters"), "{message}");
    assert_eq!("sau".parse::<CountryCode>(), Err(InvalidCountryCode));
}

proptest! {
    #[test]
    fn any_two_letters_parse_in_either_case(text in "[a-zA-Z]{2}") {
        let parsed = code(&text);
        prop_assert_eq!(parsed.to_string(), text.to_ascii_uppercase());
        let upper = text.to_ascii_uppercase();
        let letters = parsed.letters();
        prop_assert_eq!(letters.as_slice(), upper.as_bytes());
        prop_assert_eq!(text.parse::<CountryCode>(), Ok(parsed));
        prop_assert_eq!(code(&text.to_ascii_lowercase()), parsed);
        prop_assert_eq!(entry_for(parsed).map(|entry| entry.method), entry_for(code(&text.to_ascii_lowercase())).map(|entry| entry.method));
    }

    #[test]
    fn anything_else_is_rejected(text in ".{0,4}") {
        let shape = text.len() == 2 && text.bytes().all(|b| b.is_ascii_alphabetic());
        prop_assert_eq!(CountryCode::new(&text).is_ok(), shape);
    }
}
