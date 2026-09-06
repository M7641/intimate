use serde_json;

/// Convert a JSON Value to its SQL string representation.
/// Handles proper escaping for strings.
pub fn value_to_sql_string(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::Null => "NULL".to_string(),
        serde_json::Value::Bool(b) => b.to_string(),
        serde_json::Value::Number(n) => n.to_string(),
        serde_json::Value::String(s) => format!("'{}'", s.replace('\'', "''")),
        serde_json::Value::Array(_) | serde_json::Value::Object(_) => {
            let json_str = value.to_string();
            format!("'{}'", json_str.replace('\'', "''"))
        }
    }
}

/// Substitute parameter placeholders ($1, $2, etc.) with their SQL string values.
///
/// **Internal use only** — this performs simple string replacement, not true
/// parameterised queries. It is used by the connector implementations to
/// interpolate `serde_json::Value` params into SQL before sending to backends
/// that don't support native bind parameters (e.g. Snowflake REST API).
/// Do not use for user-facing APIs; prefer native parameterised queries where
/// available.
pub fn substitute_params(sql: &str, params: &[serde_json::Value]) -> String {
    let mut query_str = sql.to_string();
    for (i, param) in params.iter().enumerate() {
        let placeholder = format!("${}", i + 1);
        query_str = query_str.replace(&placeholder, &value_to_sql_string(param));
    }
    query_str
}

/// Convert an f64 to a JSON Value, preserving non-finite values as strings
/// instead of silently converting to Null.
///
/// JSON RFC 7159 forbids NaN/Infinity as numbers, so `serde_json::Number::from_f64`
/// returns `None` for them. Rather than losing the data, we represent them as
/// `"NaN"`, `"Infinity"`, or `"-Infinity"`.
pub fn f64_to_json(v: f64) -> serde_json::Value {
    serde_json::Number::from_f64(v)
        .map(serde_json::Value::Number)
        .unwrap_or_else(|| serde_json::Value::String(v.to_string()))
}

/// Parse a raw text value into the most appropriate JSON type.
///
/// Heuristic: try i64 → f64 → bool → String.
/// Used by Snowflake (REST API returns everything as text) and as a fallback
/// for NUMERIC columns in Postgres.
pub fn parse_text_value(s: &str) -> serde_json::Value {
    // Integer
    if let Ok(n) = s.parse::<i64>() {
        return serde_json::Value::Number(n.into());
    }

    // Float
    if let Ok(f) = s.parse::<f64>()
        && let Some(n) = serde_json::Number::from_f64(f)
    {
        return serde_json::Value::Number(n);
    }

    // Boolean (case-insensitive common variants)
    match s {
        "true" | "TRUE" | "True" => return serde_json::Value::Bool(true),
        "false" | "FALSE" | "False" => return serde_json::Value::Bool(false),
        _ => {}
    }

    serde_json::Value::String(s.to_string())
}

#[cfg(test)]
// 3.14 / etc. sont des valeurs de test, pas des approximations de PI.
#[allow(clippy::approx_constant)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_value_to_sql_string_null() {
        assert_eq!(value_to_sql_string(&serde_json::Value::Null), "NULL");
    }

    #[test]
    fn test_value_to_sql_string_bool() {
        assert_eq!(value_to_sql_string(&json!(true)), "true");
        assert_eq!(value_to_sql_string(&json!(false)), "false");
    }

    #[test]
    fn test_value_to_sql_string_number() {
        assert_eq!(value_to_sql_string(&json!(42)), "42");
        assert_eq!(value_to_sql_string(&json!(3.14)), "3.14");
    }

    #[test]
    fn test_value_to_sql_string_string() {
        assert_eq!(value_to_sql_string(&json!("hello")), "'hello'");
        assert_eq!(value_to_sql_string(&json!("it's")), "'it''s'");
    }

    #[test]
    fn test_value_to_sql_string_array_with_quotes() {
        let val = json!(["it's", "a test"]);
        let result = value_to_sql_string(&val);
        assert_eq!(result, r#"'["it''s","a test"]'"#);
    }

    #[test]
    fn test_value_to_sql_string_object() {
        let val = json!({"key": "val"});
        let result = value_to_sql_string(&val);
        assert!(result.starts_with('\'') && result.ends_with('\''));
    }

    #[test]
    fn test_substitute_params() {
        let sql = "SELECT * FROM users WHERE id = $1 AND name = $2";
        let params = vec![json!(42), json!("Alice")];
        let result = substitute_params(sql, &params);
        assert_eq!(
            result,
            "SELECT * FROM users WHERE id = 42 AND name = 'Alice'"
        );
    }

    #[test]
    fn test_f64_to_json_normal() {
        assert_eq!(f64_to_json(3.14), json!(3.14));
        assert_eq!(f64_to_json(0.0), json!(0.0));
        assert_eq!(f64_to_json(-42.5), json!(-42.5));
    }

    #[test]
    fn test_f64_to_json_non_finite() {
        assert_eq!(f64_to_json(f64::NAN), json!("NaN"));
        assert_eq!(f64_to_json(f64::INFINITY), json!("inf"));
        assert_eq!(f64_to_json(f64::NEG_INFINITY), json!("-inf"));
    }

    #[test]
    fn test_parse_text_value_integer() {
        assert_eq!(parse_text_value("42"), json!(42));
        assert_eq!(parse_text_value("-7"), json!(-7));
        assert_eq!(parse_text_value("0"), json!(0));
    }

    #[test]
    fn test_parse_text_value_float() {
        let val = parse_text_value("3.14");
        assert!(val.is_number());
        assert_eq!(val.as_f64(), Some(3.14));
    }

    #[test]
    fn test_parse_text_value_boolean() {
        assert_eq!(parse_text_value("true"), json!(true));
        assert_eq!(parse_text_value("TRUE"), json!(true));
        assert_eq!(parse_text_value("false"), json!(false));
        assert_eq!(parse_text_value("FALSE"), json!(false));
    }

    #[test]
    fn test_parse_text_value_string() {
        assert_eq!(parse_text_value("hello world"), json!("hello world"));
        assert_eq!(parse_text_value("2024-01-15"), json!("2024-01-15"));
    }
}
