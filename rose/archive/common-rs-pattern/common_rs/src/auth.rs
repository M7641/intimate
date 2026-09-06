//! Shared `X-Auth-Email` extractor + role resolution.
//!
//! macOS dev machines auto-promote to admin so local development works without
//! header injection.
//!
//! `can_edit_plan` is the per-feature capability for reports's Plan Editor —
//! its allowlist is narrower than admin (only Nimbus + UiPath, not Orchard Foods /
//! Greenfield), so it cannot be derived from `role`. The field is exposed
//! to every consumer; apps that don't surface it (e.g. planner) simply omit
//! it from their `get_user` JSON.

use std::convert::Infallible;

use axum::{extract::FromRequestParts, http::request::Parts};
use serde::Serialize;

pub const DEFAULT_ADMIN_EMAIL_DOMAINS: &[&str] = &[
    "nimbus.example",
    "uipath.com",
    "orchardfoods.com",
    "greenfield.co.uk",
];

pub const PLAN_EDITOR_EMAIL_DOMAINS: &[&str] = &["nimbus.example", "uipath.com"];

pub fn is_local_developer() -> bool {
    std::env::consts::OS == "macos"
}

pub fn extract_email(parts: &Parts) -> String {
    parts
        .headers
        .get("x-auth-email")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("na")
        .to_lowercase()
}

pub fn matches_domain(email: &str, domains: &[&str]) -> bool {
    domains.iter().any(|d| email.ends_with(&format!("@{d}")))
}

#[derive(Debug, Clone, Serialize)]
pub struct UserDetails {
    pub email: String,
    pub role: String,
    pub can_run_workflow: bool,
    pub can_edit_plan: bool,
}

pub struct AuthenticatedUser(pub UserDetails);

impl<S: Send + Sync> FromRequestParts<S> for AuthenticatedUser {
    type Rejection = Infallible;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        let email = extract_email(parts);
        let local_user = is_local_developer();
        let is_admin_domain = matches_domain(&email, DEFAULT_ADMIN_EMAIL_DOMAINS);
        let is_plan_editor_domain = matches_domain(&email, PLAN_EDITOR_EMAIL_DOMAINS);
        let role = if local_user || is_admin_domain {
            "admin"
        } else {
            "other"
        };
        Ok(Self(UserDetails {
            email,
            role: role.to_string(),
            can_run_workflow: role == "admin" || local_user,
            can_edit_plan: local_user || is_plan_editor_domain,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_domain_handles_subdomains_correctly() {
        assert!(matches_domain("user@nimbus.example", &["nimbus.example"]));
        assert!(!matches_domain("user@notnimbus.example", &["nimbus.example"]));
        assert!(!matches_domain("nimbus.example", &["nimbus.example"]));
    }

    #[test]
    fn matches_domain_is_case_sensitive() {
        // Caller is responsible for lowercasing — extract_email already does.
        assert!(!matches_domain("user@NIMBUS.EXAMPLE", &["nimbus.example"]));
    }

    #[test]
    fn plan_editor_is_strict_subset_of_admin() {
        // Every plan-editor domain must also be an admin domain — otherwise
        // a non-admin could see the editor tab, which would be a regression.
        for d in PLAN_EDITOR_EMAIL_DOMAINS {
            assert!(
                DEFAULT_ADMIN_EMAIL_DOMAINS.contains(d),
                "plan-editor domain {d:?} is not in admin domains",
            );
        }
    }
}
