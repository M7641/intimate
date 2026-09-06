//! Données synthétiques déterministes — simule l'ingestion depuis l'OLTP.
//!
//! En archi cible ces lignes viendraient d'un CDC depuis RDS Postgres
//! (Debezium → Kinesis → un writer Iceberg). Ici on les fabrique en mémoire
//! et on construit directement des `arrow::RecordBatch` que le writer Iceberg
//! (version d'arrow 57.x, la même que la dépendance) sait écrire.

use std::sync::Arc;

use arrow_array::{
    ArrayRef, Int32Array, Int64Array, RecordBatch, StringArray, TimestampMicrosecondArray,
};
use arrow_schema::SchemaRef;
use chrono::{NaiveDate, TimeZone, Utc};
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};

const COUNTRIES: [&str; 5] = ["FR", "ES", "PT", "IT", "DE"];

fn micros(year: i32, month: u32, day: u32) -> i64 {
    Utc.from_utc_datetime(
        &NaiveDate::from_ymd_opt(year, month, day)
            .unwrap()
            .and_hms_opt(0, 0, 0)
            .unwrap(),
    )
    .timestamp_micros()
}

/// Convertit `YYYY-MM-DD` en micros epoch (minuit UTC).
pub fn day_micros(day: &str) -> anyhow::Result<i64> {
    let d = NaiveDate::parse_from_str(day, "%Y-%m-%d")?;
    Ok(Utc
        .from_utc_datetime(&d.and_hms_opt(0, 0, 0).unwrap())
        .timestamp_micros())
}

/// Un lot de `n` customers. Colonnes : customer_id, name, country, signup_at.
pub fn customers_batch(schema: &SchemaRef, n: usize, seed: u64) -> RecordBatch {
    let mut rng = StdRng::seed_from_u64(seed);
    let base = micros(2024, 1, 1);

    let ids: Vec<i64> = (0..n as i64).collect();
    let names: Vec<String> = (0..n).map(|i| format!("customer_{i:04}")).collect();
    let countries: Vec<&str> = (0..n).map(|_| COUNTRIES[rng.gen_range(0..5)]).collect();
    let signup: Vec<i64> = (0..n)
        .map(|_| base + rng.gen_range(0..600) * 86_400 * 1_000_000)
        .collect();

    let cols: Vec<ArrayRef> = vec![
        Arc::new(Int64Array::from(ids)),
        Arc::new(StringArray::from(names)),
        Arc::new(StringArray::from(countries)),
        Arc::new(TimestampMicrosecondArray::from(signup)),
    ];
    RecordBatch::try_new(schema.clone(), cols).expect("RecordBatch customers")
}

/// Un lot de `n` orders pour `day_micros`. Si `with_discount`, on populle
/// `discount_cents` ~1/3 du temps (sinon NULL partout = état « avant évolution »).
pub fn orders_batch(
    schema: &SchemaRef,
    n: usize,
    customer_ids: &[i64],
    day_micros: i64,
    seed: u64,
    with_discount: bool,
) -> RecordBatch {
    let mut rng = StdRng::seed_from_u64(seed);

    let mut order_id = Vec::with_capacity(n);
    let mut customer_id = Vec::with_capacity(n);
    let mut amount = Vec::with_capacity(n);
    let mut currency = Vec::with_capacity(n);
    let mut status = Vec::with_capacity(n);
    let mut created = Vec::with_capacity(n);
    let mut discount: Vec<Option<i32>> = Vec::with_capacity(n);

    for i in 0..n {
        let amt = rng.gen_range(500..50_000i64);
        order_id.push(rng.gen_range(1_000_000_000i64..10_000_000_000i64));
        customer_id.push(customer_ids[rng.gen_range(0..customer_ids.len())]);
        amount.push(amt);
        currency.push("EUR");
        // 3 paid pour 1 refunded.
        status.push(if rng.gen_range(0..4) == 0 {
            "refunded"
        } else {
            "paid"
        });
        created.push(day_micros + rng.gen_range(0..86_400i64) * 1_000_000);
        discount.push(if with_discount && i % 3 == 0 {
            Some((amt / 10) as i32)
        } else {
            None
        });
    }

    let cols: Vec<ArrayRef> = vec![
        Arc::new(Int64Array::from(order_id)),
        Arc::new(Int64Array::from(customer_id)),
        Arc::new(Int64Array::from(amount)),
        Arc::new(StringArray::from(currency)),
        Arc::new(StringArray::from(status)),
        Arc::new(TimestampMicrosecondArray::from(created)),
        Arc::new(Int32Array::from(discount)),
    ];
    RecordBatch::try_new(schema.clone(), cols).expect("RecordBatch orders")
}

/// Construit le RecordBatch du mart à partir des lignes agrégées par DuckDB.
pub fn mart_batch(schema: &SchemaRef, rows: &[(String, i64, f64, f64)]) -> RecordBatch {
    let country: Vec<&str> = rows.iter().map(|r| r.0.as_str()).collect();
    let paid: Vec<i64> = rows.iter().map(|r| r.1).collect();
    let gross: Vec<f64> = rows.iter().map(|r| r.2).collect();
    let disc: Vec<f64> = rows.iter().map(|r| r.3).collect();

    let cols: Vec<ArrayRef> = vec![
        Arc::new(StringArray::from(country)),
        Arc::new(Int64Array::from(paid)),
        Arc::new(arrow_array::Float64Array::from(gross)),
        Arc::new(arrow_array::Float64Array::from(disc)),
    ];
    RecordBatch::try_new(schema.clone(), cols).expect("RecordBatch mart")
}
