//! Validation primitives for backend routes — port of `src/common/backend/validation.py`.
//!
//! Every parameter that travels into a SQL string-format site (table prefixes, ORDER BY
//! columns, etc.) must pass through one of these patterns. Values that go through
//! `Params` binding are escaped by sqlx and do not need to match these regexes.

use std::sync::LazyLock;

use regex::Regex;
use thiserror::Error;

/// Permissive enough to accept real customer codes ("M&S", "Marks & Spencer",
/// "B&Q (Group)") while still blocking quotes, semicolons, backslashes, and anything
/// else that could escape a single-quoted SQL literal if string-formatted into SQL.
pub static SAFE_CODE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^[A-Za-z0-9_\- $£€'&.,/()]{1,128}$").expect("SAFE_CODE"));

/// ISO date or full timestamp: covers `2026-04-25`, `2026-04-25T14:33:07Z`, and the
/// space-separated form `2026-04-25 14:33:07` that Redshift's timestamp::varchar emits.
pub static SAFE_DATE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\d{4}-\d{2}-\d{2}([T ][\d:.]+Z?)?$").expect("SAFE_DATE"));

/// Sha-256 hex business-key shape (used for SCD2 row keys).
pub static SAFE_HEX_64: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^[a-f0-9]{64}$").expect("SAFE_HEX_64"));

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ValidationError {
    #[error("at most {max} {name:?} values may be supplied")]
    TooMany { name: String, max: usize },

    #[error("invalid {name:?} value: {value:?}")]
    InvalidValue { name: String, value: String },
}

/// Reject oversized filter lists or values that fail `SAFE_CODE`.
/// Returns Ok on a None list (caller's optional filter just isn't set).
pub fn validate_filter_list(
    name: &str,
    values: Option<&[String]>,
    max_items: usize,
) -> Result<(), ValidationError> {
    let Some(values) = values else {
        return Ok(());
    };
    if values.len() > max_items {
        return Err(ValidationError::TooMany {
            name: name.to_string(),
            max: max_items,
        });
    }
    for v in values {
        if !SAFE_CODE.is_match(v) {
            return Err(ValidationError::InvalidValue {
                name: name.to_string(),
                value: v.clone(),
            });
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn safe_code_accepts_real_customer_codes() {
        assert!(SAFE_CODE.is_match("M&S"));
        assert!(SAFE_CODE.is_match("Marks & Spencer"));
        assert!(SAFE_CODE.is_match("B&Q (Group)"));
        assert!(SAFE_CODE.is_match("foo_bar"));
        assert!(SAFE_CODE.is_match("123-456"));
    }

    #[test]
    fn safe_code_rejects_quotes_and_semicolons() {
        assert!(!SAFE_CODE.is_match("foo'; DROP"));
        assert!(!SAFE_CODE.is_match("a;b"));
        assert!(!SAFE_CODE.is_match("\"quoted\""));
        assert!(!SAFE_CODE.is_match(""));
    }

    #[test]
    fn safe_date_accepts_iso_variants() {
        assert!(SAFE_DATE.is_match("2026-04-25"));
        assert!(SAFE_DATE.is_match("2026-04-25T14:33:07Z"));
        assert!(SAFE_DATE.is_match("2026-04-25 14:33:07"));
        assert!(!SAFE_DATE.is_match("2026/04/25"));
    }

    #[test]
    fn safe_hex_64_accepts_sha256_shape() {
        assert!(SAFE_HEX_64.is_match(&"a".repeat(64)));
        assert!(!SAFE_HEX_64.is_match(&"a".repeat(63)));
        assert!(!SAFE_HEX_64.is_match(&"A".repeat(64)));
    }

    #[test]
    fn validate_filter_list_passthrough_on_none() {
        validate_filter_list("mascode", None, 100).unwrap();
    }

    #[test]
    fn validate_filter_list_rejects_oversized() {
        let big: Vec<String> = (0..101).map(|i| format!("v{i}")).collect();
        let err = validate_filter_list("mascode", Some(&big), 100).unwrap_err();
        assert!(matches!(err, ValidationError::TooMany { max: 100, .. }));
    }

    #[test]
    fn validate_filter_list_rejects_unsafe_values() {
        // SAFE_CODE permits ' for customer names like "M&S's Foods" — block via ; instead.
        let bad = vec!["good".to_string(), "bad;val".to_string()];
        let err = validate_filter_list("mascode", Some(&bad), 100).unwrap_err();
        assert!(matches!(err, ValidationError::InvalidValue { .. }));
    }
}
