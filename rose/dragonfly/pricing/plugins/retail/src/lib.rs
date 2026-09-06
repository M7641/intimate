//! Retail department pricing engine.
//!
//! Retail sells at a margin over list and rewards larger baskets:
//!   * +8% retail markup on the unit price
//!   * volume discount: 5% from qty>=10, 12% from qty>=50
//!   * "vip" segment: extra 5% off
//!
//! Note how this is *genuinely different logic* from wholesale, not just
//! different numbers — retail marks up, wholesale marks down.

use extism_pdk::*;
use pricing_shared::{PricingQuote, PricingRequest};

const MARKUP_PCT: i64 = 8;

#[plugin_fn]
pub fn price(Json(req): Json<PricingRequest>) -> FnResult<Json<PricingQuote>> {
    let mut rules = Vec::new();

    // Retail marks the list price *up*.
    let unit = req.base_price_cents * (100 + MARKUP_PCT) / 100;
    rules.push(format!("retail markup +{MARKUP_PCT}%"));
    let subtotal = unit * req.quantity;

    // Volume tiers reward bigger orders.
    let mut discount_pct = if req.quantity >= 50 {
        rules.push("volume discount 12% (qty>=50)".into());
        12
    } else if req.quantity >= 10 {
        rules.push("volume discount 5% (qty>=10)".into());
        5
    } else {
        0
    };

    if req.segment.as_deref() == Some("vip") {
        discount_pct += 5;
        rules.push("VIP segment +5%".into());
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
        engine: "retail".into(),
        applied_rules: rules,
    }))
}
