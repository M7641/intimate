use database::DBActions;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let db = DBActions::from_name("duckdb")?;

    db.ping()?;

    db.execute("CREATE SCHEMA IF NOT EXISTS stage", &[])?;

    db.execute(
        "CREATE TABLE IF NOT EXISTS stage.orgstrauk (id INTEGER, name TEXT, age INTEGER)",
        &[],
    )?;

    db.execute(
        "INSERT INTO stage.orgstrauk (id, name, age) VALUES (1, 'John', 30), (2, 'Jane', 25), (3, 'Bob', 35)",
        &[],
    )?;

    let sql = format!("select * from stage.{} limit {}", "orgstrauk", 3);

    println!("SQL: {}", sql);

    let result = db.query(&sql, &[])?;
    println!("Found {:?}", result);

    db.close()?;

    Ok(())
}
