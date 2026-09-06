//! Validated SQL identifiers (table / schema names).
//!
//! Identifiers often come from user input but **cannot** be passed as bound
//! parameters in any SQL dialect — bind parameters stand in for *values*, never
//! for identifiers. The warehouse-agnostic defence is therefore a strict
//! allowlist rather than a keyword blocklist: an identifier must be a non-empty
//! run of ASCII letters, digits and underscores, starting with a letter or
//! underscore, within the most restrictive length limit among the supported
//! warehouses.
//!
//! This closes every injection vector at the token boundary — no whitespace,
//! quote, semicolon, dot or comment marker can appear — and needs no fragile,
//! dialect-specific list of reserved words. A reserved word such as `select` is
//! still *accepted* here (it is not an injection risk); it would simply fail
//! when the warehouse executes the statement.

use std::fmt;

/// Maximum identifier length, in bytes.
///
/// Postgres and Redshift truncate at 63 (`NAMEDATALEN - 1`); DuckDB and
/// Snowflake allow more. 63 is thus the value that is safe to send to every
/// supported backend.
pub const MAX_IDENTIFIER_LEN: usize = 63;

/// Why a string was rejected as a SQL identifier.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IdentifierError {
    Empty,
    TooLong { len: usize },
    Invalid { value: String },
}

impl fmt::Display for IdentifierError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            IdentifierError::Empty => write!(f, "identifier must not be empty"),
            IdentifierError::TooLong { len } => write!(
                f,
                "identifier is {len} bytes, exceeding the {MAX_IDENTIFIER_LEN}-byte limit"
            ),
            IdentifierError::Invalid { value } => write!(
                f,
                "identifier {value:?} is not allowed: use only ASCII letters, digits and \
                 underscores, starting with a letter or underscore"
            ),
        }
    }
}

impl std::error::Error for IdentifierError {}

/// A validated SQL identifier.
///
/// The only constructor is [`Identifier::new`], so a value of this type is proof
/// that the name is safe to interpolate directly into SQL or an object key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Identifier(String);

impl Identifier {
    /// Validate `raw` as a SQL identifier, rejecting anything outside the
    /// `[A-Za-z_][A-Za-z0-9_]*` allowlist or over [`MAX_IDENTIFIER_LEN`] bytes.
    pub fn new(raw: impl Into<String>) -> Result<Self, IdentifierError> {
        let raw = raw.into();
        if raw.is_empty() {
            return Err(IdentifierError::Empty);
        }
        if raw.len() > MAX_IDENTIFIER_LEN {
            return Err(IdentifierError::TooLong { len: raw.len() });
        }
        // Iterate by `char` (never by byte index) so multi-byte input is
        // rejected cleanly rather than panicking on a slice boundary.
        let valid = {
            let mut chars = raw.chars();
            let first = chars.next().expect("non-empty checked above");
            (first.is_ascii_alphabetic() || first == '_')
                && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
        };
        if !valid {
            return Err(IdentifierError::Invalid { value: raw });
        }
        Ok(Self(raw))
    }

    /// The validated identifier text.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Identifier {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_plain_names() {
        for ok in ["orders", "_staging", "a1_b2", "ORDERS", "t_2026"] {
            assert!(Identifier::new(ok).is_ok(), "{ok:?} should be valid");
        }
    }

    #[test]
    fn rejects_empty() {
        assert_eq!(Identifier::new(""), Err(IdentifierError::Empty));
    }

    #[test]
    fn rejects_too_long() {
        let long = "a".repeat(MAX_IDENTIFIER_LEN + 1);
        assert_eq!(
            Identifier::new(long),
            Err(IdentifierError::TooLong {
                len: MAX_IDENTIFIER_LEN + 1
            })
        );
        // Exactly at the limit is fine.
        assert!(Identifier::new("a".repeat(MAX_IDENTIFIER_LEN)).is_ok());
    }

    #[test]
    fn rejects_leading_digit() {
        assert!(matches!(
            Identifier::new("1table"),
            Err(IdentifierError::Invalid { .. })
        ));
    }

    #[test]
    fn rejects_injection_and_separators() {
        for bad in [
            "x (y int); DROP TABLE secrets; --", // the classic injection
            "orders; DROP TABLE x",
            "stage.orders", // qualified names must be built from validated parts
            "order's",      // quote
            "my table",     // whitespace
            "my-table",     // hyphen
            "tab\tname",    // control char
            "café",         // non-ASCII (must not panic)
        ] {
            assert!(
                matches!(Identifier::new(bad), Err(IdentifierError::Invalid { .. })),
                "{bad:?} should be rejected"
            );
        }
    }

    #[test]
    fn display_round_trips_the_name() {
        let id = Identifier::new("orders").unwrap();
        assert_eq!(id.to_string(), "orders");
        assert_eq!(id.as_str(), "orders");
    }
}
