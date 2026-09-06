//! Internal binary boundary demo.
//!
//! Demonstrates the Protobuf side of the registry: types generated from the
//! same `customer.proto` the Python service uses, the compact binary wire
//! format, and the `buf.validate` rules enforced in code (in production the
//! `protovalidate` runtime enforces them from the proto annotations directly).

mod customer {
    // prost writes <package>.rs into OUT_DIR; package is contracts.customer.v1
    include!(concat!(env!("OUT_DIR"), "/contracts.customer.v1.rs"));
}

use customer::{Customer, Tier};
use prost::Message;

/// Mirror of the `(buf.validate)` annotations in customer.proto.
fn validate(c: &Customer) -> Result<(), String> {
    let id_ok = c.customer_id.len() > 5
        && c.customer_id.starts_with("cust_")
        && c.customer_id[5..]
            .chars()
            .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit());
    if !id_ok {
        return Err(format!(
            "customer_id '{}' fails ^cust_[a-z0-9]+$",
            c.customer_id
        ));
    }
    if !c.email.contains('@') {
        return Err(format!("email '{}' is not a valid address", c.email));
    }
    if c.tier == Tier::Unspecified as i32 {
        return Err("tier is unset".into());
    }
    if !(0.0..=1e12).contains(&c.annual_revenue) {
        return Err(format!(
            "annual_revenue {} outside [0, 1e12]",
            c.annual_revenue
        ));
    }
    Ok(())
}

fn main() {
    println!("\nTROY — internal binary boundary (Protobuf)\n");

    let good = Customer {
        customer_id: "cust_abc123".into(),
        email: "ada@acme.com".into(),
        tier: Tier::Enterprise as i32,
        annual_revenue: 5_000_000.0,
    };

    match validate(&good) {
        Ok(()) => println!("  valid customer  -> ACCEPTED"),
        Err(e) => println!("  unexpected error: {e}"),
    }

    // The win of the interior: compact binary wire vs JSON text.
    let wire = good.encode_to_vec();
    let json = format!(
        r#"{{"customer_id":"{}","email":"{}","tier":"ENTERPRISE","annual_revenue":{}}}"#,
        good.customer_id, good.email, good.annual_revenue
    );
    println!(
        "  wire size       -> {} bytes protobuf vs {} bytes JSON",
        wire.len(),
        json.len()
    );

    // Round-trip proves both ends decode the same bytes.
    let decoded = Customer::decode(&wire[..]).expect("decode");
    println!(
        "  round-trip      -> {} / tier={:?}",
        decoded.customer_id,
        decoded.tier()
    );

    let bad = Customer {
        customer_id: "CUST-ABC".into(),
        email: "nope".into(),
        tier: Tier::Unspecified as i32,
        annual_revenue: -42.0,
    };
    match validate(&bad) {
        Ok(()) => println!("  bad customer    -> wrongly accepted!"),
        Err(e) => println!("  bad customer    -> REJECTED ({e})"),
    }
    println!();
}
