use std::sync::LazyLock;

use regex::Regex;

use service_kit::error::AppError;

/// Regex for safe SQL identifiers: starts with letter or underscore, then
/// alphanumeric or underscores only.
static SAFE_IDENTIFIER: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^[a-zA-Z_][a-zA-Z0-9_]*$").unwrap());

/// Validate that a schema name is a safe SQL identifier.
///
/// Warehouse analytics queries interpolate schema/database names into SQL, so
/// this guards against injection by allowing only safe identifier characters.
pub fn validate_schema_name(schema: &str) -> Result<(), AppError> {
    if !SAFE_IDENTIFIER.is_match(schema) {
        return Err(AppError::Validation(format!(
            "Invalid schema name: {schema}"
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_schema_names() {
        assert!(validate_schema_name("public").is_ok());
        assert!(validate_schema_name("stage").is_ok());
        assert!(validate_schema_name("my_schema_v2").is_ok());
    }

    #[test]
    fn invalid_schema_names() {
        assert!(validate_schema_name("my schema").is_err());
        assert!(validate_schema_name("schema;drop").is_err());
    }
}
