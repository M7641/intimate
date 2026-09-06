use database::DBActions;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Reads REDSHIFT_HOST, REDSHIFT_PORT, REDSHIFT_DATABASE,
    // REDSHIFT_USERNAME, REDSHIFT_PASSWORD from env.
    let db = DBActions::from_name("postgres")?;

    db.ping()?;
    println!("Connected to Redshift via native Postgres");

    // Session info
    let rows = db.query(
        "SELECT current_database(), current_schema(), current_user",
        &[],
    )?;
    println!("Session info: {:?}", rows);

    // Create table in sandpit schema
    db.execute(
        "CREATE TABLE IF NOT EXISTS sandpit.rust_test (
            id INTEGER,
            name VARCHAR(100),
            score FLOAT
        )",
        &[],
    )?;
    println!("Table sandpit.rust_test created");

    // Insert some data
    db.execute(
        "INSERT INTO sandpit.rust_test (id, name, score) VALUES
            (1, 'Alice', 9.5),
            (2, 'Bob', 7.3),
            (3, 'Carol', 8.8)",
        &[],
    )?;
    println!("Inserted 3 rows");

    // Read it back
    let result = db.query("SELECT * FROM sandpit.rust_test ORDER BY id", &[])?;
    println!("\nRows in sandpit.rust_test:");
    for row in &result {
        println!("  {:?}", row);
    }

    // Clean up
    db.execute("DROP TABLE IF EXISTS sandpit.rust_test", &[])?;
    println!("\nTable dropped — clean up complete");

    db.close()?;
    Ok(())
}
