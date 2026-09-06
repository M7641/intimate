use extism_pdk::*;
use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
struct Record {
    product_code: String,
    region: String,
    quantity: i64,
    unit_price_cents: i64,
}

#[derive(Serialize)]
struct ValidationResult {
    valid: bool,
    errors: Vec<String>,
}

#[derive(Serialize)]
struct TransformedRecord {
    product_code: String,
    region: String,
    quantity: i64,
    unit_price_cents: i64,
    total_cents: i64,
    currency: String,
}

/// Validates a data record against business rules.
/// Returns pass/fail with a list of violation reasons.
#[plugin_fn]
pub fn validate(Json(record): Json<Record>) -> FnResult<Json<ValidationResult>> {
    let mut errors = Vec::new();

    if record.quantity <= 0 {
        errors.push("quantity must be positive".into());
    }
    if record.unit_price_cents <= 0 {
        errors.push("unit_price_cents must be positive".into());
    }

    let region_upper = record.region.to_uppercase();
    let valid_prefix = match region_upper.as_str() {
        "EU" => record.product_code.starts_with("EU-"),
        "US" => record.product_code.starts_with("US-"),
        "APAC" => record.product_code.starts_with("AP-"),
        _ => {
            errors.push(format!("unknown region: {}", record.region));
            false
        }
    };
    if !valid_prefix && !errors.iter().any(|e| e.starts_with("unknown region")) {
        errors.push(format!(
            "product_code '{}' does not match region '{}'",
            record.product_code, record.region
        ));
    }

    Ok(Json(ValidationResult {
        valid: errors.is_empty(),
        errors,
    }))
}

/// Transforms a data record: normalizes region, computes total, adds currency.
#[plugin_fn]
pub fn transform(Json(record): Json<Record>) -> FnResult<Json<TransformedRecord>> {
    let region = record.region.to_uppercase();
    let currency = match region.as_str() {
        "EU" => "EUR",
        "US" => "USD",
        "APAC" => "SGD",
        _ => "USD",
    };

    Ok(Json(TransformedRecord {
        product_code: record.product_code,
        region,
        quantity: record.quantity,
        unit_price_cents: record.unit_price_cents,
        total_cents: record.quantity * record.unit_price_cents,
        currency: currency.to_string(),
    }))
}
