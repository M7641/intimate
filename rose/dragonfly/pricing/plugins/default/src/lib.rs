//! Default pricing engine — the safe fallback.
//!
//! No markup, no discounts: list price times quantity. This is what the
//! host falls back to when no department header is supplied, an unknown
//! department is requested, or a department plugin errors out.

use extism_pdk::*;
use pricing_shared::{PricingQuote, PricingRequest};

#[plugin_fn]
pub fn price(Json(req): Json<PricingRequest>) -> FnResult<Json<PricingQuote>> {
    let subtotal = req.base_price_cents * req.quantity;

    Ok(Json(PricingQuote {
        sku: req.sku,
        quantity: req.quantity,
        unit_price_cents: req.base_price_cents,
        subtotal_cents: subtotal,
        discount_cents: 0,
        total_cents: subtotal,
        currency: "GBP".into(),
        engine: "default".into(),
        applied_rules: vec!["list price, no adjustments".into()],
    }))
}
