//! The pricing contract shared by the host and every department plugin.
//!
//! Both structs cross the WASM boundary as JSON. The host serialises a
//! `PricingRequest`; the plugin returns a `PricingQuote`. Keeping the
//! shape here means a new department plugin starts from a known contract
//! and only has to implement the *logic*, not re-declare the data.

use serde::{Deserialize, Serialize};

/// What the API receives and forwards to whichever engine is selected.
#[derive(Deserialize, Clone)]
pub struct PricingRequest {
    pub sku: String,
    /// List price for a single unit, in minor currency units (e.g. pence).
    pub base_price_cents: i64,
    pub quantity: i64,
    /// Optional customer segment hint (e.g. "vip", "key_account").
    #[serde(default)]
    pub segment: Option<String>,
}

/// The priced result a department engine returns.
#[derive(Serialize)]
pub struct PricingQuote {
    pub sku: String,
    pub quantity: i64,
    pub unit_price_cents: i64,
    pub subtotal_cents: i64,
    pub discount_cents: i64,
    pub total_cents: i64,
    pub currency: String,
    /// Which department engine produced this quote.
    pub engine: String,
    /// Human-readable trace of the rules that fired — makes the
    /// difference between engines visible in the response.
    pub applied_rules: Vec<String>,
}
