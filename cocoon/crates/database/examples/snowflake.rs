use database::DBActions;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Reads SNOWFLAKE_ACCOUNT, SNOWFLAKE_OAUTH_TOKEN, etc. from env.
    // For Nimbus OAuth, use DatabaseConfig::Snowflake(SnowflakeConfig::from_nimbus_api()?) instead.
    let db = DBActions::from_name("snowflake")?;

    db.ping()?;
    println!("Connected to Snowflake via REST API (Nimbus OAuth)");

    // Session info
    let rows = db.query(
        "SELECT CURRENT_WAREHOUSE(), CURRENT_DATABASE(), CURRENT_SCHEMA()",
        &[],
    )?;
    println!("Session info: {:?}", rows);

    // Create table in SANDPIT schema
    db.execute(
        "CREATE TABLE IF NOT EXISTS SANDPIT.rust_test (
            id INTEGER,
            name VARCHAR,
            score FLOAT
        )",
        &[],
    )?;
    println!("Table SANDPIT.rust_test created");

    // Insert some data
    db.execute(
        "INSERT INTO SANDPIT.rust_test (id, name, score) VALUES
            (1, 'Alice', 9.5),
            (2, 'Bob', 7.3),
            (3, 'Carol', 8.8)",
        &[],
    )?;
    println!("Inserted 3 rows");

    // Read it back
    let result = db.query("SELECT * FROM SANDPIT.rust_test ORDER BY id", &[])?;
    println!("\nRows in SANDPIT.rust_test:");
    for row in &result {
        println!("  {:?}", row);
    }

    // Clean up
    db.execute("DROP TABLE IF EXISTS SANDPIT.rust_test", &[])?;
    println!("\nTable dropped — clean up complete");

    db.close()?;
    Ok(())
}
