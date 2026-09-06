//! Cautious-by-default safety heuristics.
//!
//! On an unknown site we must not trigger destructive or irreversible actions.
//! Filling fields is safe; the risk is in *submitting* — so we deny controls
//! whose label/name/selector smells destructive, never touch password/file or
//! payment forms, and gate POST-form submission behind an explicit flag.

use crate::models::{ControlKind, ControlSpec};

/// Words that mark a control as destructive / irreversible / costly.
const DESTRUCTIVE: &[&str] = &[
    "delete",
    "remove",
    "destroy",
    "drop",
    "trash",
    "discard",
    "cancel",
    "deactivate",
    "disable",
    "revoke",
    "reset",
    "clear",
    "wipe",
    "purge",
    "logout",
    "log out",
    "sign out",
    "signout",
    "pay",
    "checkout",
    "buy",
    "order",
    "purchase",
    "confirm",
    "approve",
    "publish",
    "unpublish",
    "archive",
    "send",
    "transfer",
    "withdraw",
    "deposit",
];

/// Whether the control reads as destructive (matched case-insensitively over
/// its label, name, and selector).
pub fn is_destructive(c: &ControlSpec) -> bool {
    let hay = format!(
        "{} {} {}",
        c.label.as_deref().unwrap_or(""),
        c.name.as_deref().unwrap_or(""),
        c.selector
    )
    .to_lowercase();
    DESTRUCTIVE.iter().any(|kw| hay.contains(kw))
}

/// Whether driving this control would submit a form.
fn submits(c: &ControlSpec) -> bool {
    matches!(c.kind, ControlKind::Button | ControlKind::Tab)
}

pub enum Decision {
    Exercise,
    Skip(&'static str),
}

/// The cautious policy. `submit_forms` allows submitting POST forms (still
/// subject to the destructive and sensitive-form rules).
pub fn decide(c: &ControlSpec, submit_forms: bool) -> Decision {
    if c.disabled {
        return Decision::Skip("disabled");
    }
    if is_destructive(c) {
        return Decision::Skip("destructive keyword");
    }
    if let Some(form) = &c.form {
        if form.has_sensitive {
            return Decision::Skip("password/file/payment form");
        }
        if submits(c) && form.method == "post" && !submit_forms {
            return Decision::Skip("POST submit disabled (use --submit-forms)");
        }
    }
    Decision::Exercise
}
