//! Framework heuristics. A hint, never a hard dependency — the generic capture
//! works regardless; detection only steers later specialisation (Shiny ws
//! decoding, Superset dashboard-config fetch).

use crate::models::NetworkRecord;

pub fn detect_framework(network: &[NetworkRecord]) -> Option<String> {
    let urls: Vec<&str> = network.iter().map(|r| r.url.as_str()).collect();

    if urls
        .iter()
        .any(|u| u.contains("/websocket") || u.contains("shiny"))
    {
        return Some("shiny".to_string());
    }
    if urls
        .iter()
        .any(|u| u.contains("superset") || u.contains("/api/v1/chart"))
    {
        return Some("superset".to_string());
    }
    None
}
