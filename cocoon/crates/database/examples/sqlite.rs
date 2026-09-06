use database::DBActions;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let db = DBActions::from_name("sqlite")?;

    db.ping()?;
    println!("Connected to SQLite (in-memory)");

    db.execute(
        "CREATE TABLE users (id INTEGER PRIMARY KEY, name TEXT, score REAL)",
        &[],
    )?;

    db.execute(
        "INSERT INTO users (id, name, score) VALUES (1, 'Alice', 9.5)",
        &[],
    )?;
    db.execute(
        "INSERT INTO users (id, name, score) VALUES (2, 'Bob', 7.3)",
        &[],
    )?;
    db.execute(
        "INSERT INTO users (id, name, score) VALUES (3, 'Carol', 8.8)",
        &[],
    )?;
    println!("Inserted 3 rows");

    let result = db.query("SELECT * FROM users ORDER BY id", &[])?;
    println!("\nRows in users:");
    for row in &result {
        println!("  {:?}", row);
    }

    // Schema introspection via PRAGMA table_info
    let schema = db.as_database().get_table_schema("users")?;
    println!("\nTable schema:");
    for col in &schema.columns {
        println!(
            "  {} ({}) nullable={}",
            col.name, col.data_type, col.is_nullable
        );
    }

    db.close()?;
    Ok(())
}
