//! Plausible input values, derived from the control kind and label/name hints —
//! no site knowledge. The goal is values realistic enough that the app's
//! handlers run a normal path (e.g. a search that returns results), not fuzzing.

use crate::models::{ControlKind, ControlSpec};

/// A value to type into a text-like control, or `None` if the kind isn't typed.
/// `page_token` is a word harvested from the page, used to feed search inputs.
pub fn value_for(c: &ControlSpec, page_token: Option<&str>) -> Option<String> {
    let hint = format!(
        "{} {}",
        c.label.as_deref().unwrap_or(""),
        c.name.as_deref().unwrap_or("")
    )
    .to_lowercase();
    let token = || page_token.unwrap_or("test").to_string();

    match c.kind {
        ControlKind::Email => Some("test@example.com".to_string()),
        ControlKind::Number => Some("1".to_string()),
        ControlKind::Date => Some("2020-06-15".to_string()),
        ControlKind::Search => Some(token()),
        ControlKind::Text => Some(if hint.contains("email") {
            "test@example.com".to_string()
        } else if hint.contains("search") || hint.contains("query") || hint.contains("filter") {
            token()
        } else if hint.contains("name") {
            "Test User".to_string()
        } else if hint.contains("phone") || hint.contains("tel") {
            "02079460000".to_string()
        } else if hint.contains("url") || hint.contains("website") {
            "https://example.com".to_string()
        } else if hint.contains("date") || hint.contains("dob") {
            "2020-01-01".to_string()
        } else if hint.contains("amount") || hint.contains("number") || hint.contains("qty") {
            "1".to_string()
        } else {
            token()
        }),
        _ => None,
    }
}

/// The option value to pick for a `<select>` — the last non-empty option, which
/// most often differs from the default and so triggers the change handler.
pub fn select_value(c: &ControlSpec) -> Option<String> {
    c.options.last().cloned()
}
