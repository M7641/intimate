//! Wholesale department pricing engine.
//!
//! Wholesale sells *below* list off a negotiated rate and rewards bulk:
//!   * -10% negotiated discount off list (lower unit than retail)
//!   * bulk tiers: 8% from qty>=100, 15% from qty>=500
//!   * "key_account" segment: extra 5% off
//!
//! Same input contract as retail, opposite commercial intent — this is
//! the "two departments, slightly different logic" the pilot simulates.

use extism_pdk::*;
use pricing_shared::{PricingQuote, PricingRequest};

const NEGOTIATED_DISCOUNT_PCT: i64 = 10;

#[plugin_fn]
pub fn price(Json(req): Json<PricingRequest>) -> FnResult<Json<PricingQuote>> {
    let mut rules = Vec::new();

    // Wholesale starts *below* list off a negotiated rate.
    let unit = req.base_price_cents * (100 - NEGOTIATED_DISCOUNT_PCT) / 100;
    rules.push(format!(
        "wholesale negotiated -{NEGOTIATED_DISCOUNT_PCT}% off list"
    ));
    let subtotal = unit * req.quantity;

    // Bulk thresholds are an order of magnitude higher than retail.
    let mut discount_pct = if req.quantity >= 500 {
        rules.push("bulk tier 15% (qty>=500)".into());
        15
    } else if req.quantity >= 100 {
        rules.push("bulk tier 8% (qty>=100)".into());
        8
    } else {
        0
    };

    if req.segment.as_deref() == Some("key_account") {
        discount_pct += 5;
        rules.push("key account +5%".into());
    }

    let discount = subtotal * discount_pct / 100;
    let total = subtotal - discount;

    Ok(Json(PricingQuote {
        sku: req.sku,
        quantity: req.quantity,
        unit_price_cents: unit,
        subtotal_cents: subtotal,
        discount_cents: discount,
        total_cents: total,
        currency: "GBP".into(),
        engine: "wholesale".into(),
        applied_rules: rules,
    }))
}
