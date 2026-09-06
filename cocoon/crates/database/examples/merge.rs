use database::{DBActions, MergeCondition, MergeConfig, UpdateColumns};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create an in-memory DuckDB database
    let db = DBActions::from_name("duckdb")?;

    // Create target table
    db.execute(
        "CREATE TABLE users (
            id BIGINT PRIMARY KEY,
            name VARCHAR,
            email VARCHAR,
            updated_at TIMESTAMP
        )",
        &[],
    )?;

    // Insert initial data
    db.execute(
        "INSERT INTO users VALUES
            (1, 'Alice', 'alice@example.com', '2024-01-01'),
            (2, 'Bob', 'bob@example.com', '2024-01-01')",
        &[],
    )?;

    println!("Initial users:");
    let rows = db.query("SELECT * FROM users ORDER BY id", &[])?;
    for row in &rows {
        println!("  {:?}", row);
    }

    // Create staging table with updates and new records
    db.execute(
        "CREATE TABLE staging_users (
            id BIGINT,
            name VARCHAR,
            email VARCHAR,
            updated_at TIMESTAMP
        )",
        &[],
    )?;

    db.execute(
        "INSERT INTO staging_users VALUES
            (1, 'Alice Smith', 'alice.smith@example.com', '2024-06-01'),
            (3, 'Carol', 'carol@example.com', '2024-06-01')",
        &[],
    )?;

    println!("\nStaging data:");
    let rows = db.query("SELECT * FROM staging_users ORDER BY id", &[])?;
    for row in &rows {
        println!("  {:?}", row);
    }

    // Perform MERGE: update existing users, insert new ones
    println!("\nExecuting MERGE...");
    let merge_config = MergeConfig::new("users", "staging_users", MergeCondition::on_column("id"));

    let affected = db.merge(&merge_config)?;
    println!("Rows affected: {}", affected);

    println!("\nUsers after MERGE:");
    let rows = db.query("SELECT * FROM users ORDER BY id", &[])?;
    for row in &rows {
        println!("  {:?}", row);
    }

    // Example with UpdateColumns::Except - don't update certain columns
    println!("\n--- Example: Merge with Except ---");

    db.execute("DELETE FROM users", &[])?;
    db.execute(
        "INSERT INTO users VALUES
            (1, 'Alice', 'alice@example.com', '2024-01-01'),
            (2, 'Bob', 'bob@example.com', '2024-01-01')",
        &[],
    )?;

    let merge_config = MergeConfig::new("users", "staging_users", MergeCondition::on_column("id"))
        .with_update(UpdateColumns::Except(vec!["email".to_string()]));

    let affected = db.merge(&merge_config)?;
    println!("Rows affected: {}", affected);

    println!("\nUsers after MERGE (email preserved for Alice):");
    let rows = db.query("SELECT * FROM users ORDER BY id", &[])?;
    for row in &rows {
        println!("  {:?}", row);
    }

    // Example with raw SQL condition (update-only)
    println!("\n--- Example: Raw SQL Condition (Update Only) ---");

    db.execute("DELETE FROM users", &[])?;
    db.execute(
        "INSERT INTO users VALUES
            (1, 'Alice', 'alice@example.com', '2024-06-15'),
            (2, 'Bob', 'bob@example.com', '2024-01-01')",
        &[],
    )?;

    let merge_config =
        MergeConfig::new("users", "staging_users", MergeCondition::raw("t.id = s.id"))
            .without_insert();

    let affected = db.merge(&merge_config)?;
    println!("Rows affected: {}", affected);

    println!("\nUsers after MERGE (only newer updates applied):");
    let rows = db.query("SELECT * FROM users ORDER BY id", &[])?;
    for row in &rows {
        println!("  {:?}", row);
    }

    Ok(())
}
