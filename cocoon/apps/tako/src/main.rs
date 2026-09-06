//! Thin binary entry point. All logic lives in the library (`crate` / `tako::`)
//! so it can use `crate::` paths and be reached by tests and benches.

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tako::run().await
}
