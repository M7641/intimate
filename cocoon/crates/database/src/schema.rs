/// Information about a single column in a database table.
#[derive(Debug, Clone)]
pub struct ColumnInfo {
    /// Column name
    pub name: String,
    /// Database data type (e.g., "VARCHAR", "INTEGER")
    pub data_type: String,
    /// Position of the column in the table (1-indexed)
    pub ordinal_position: usize,
    /// Whether the column allows NULL values
    pub is_nullable: bool,
}

/// Schema metadata for a database table.
#[derive(Debug, Clone)]
pub struct TableSchema {
    /// Schema name (e.g., "public"), if applicable
    pub schema_name: Option<String>,
    /// Table name
    pub table_name: String,
    /// Columns in ordinal position order
    pub columns: Vec<ColumnInfo>,
}

impl TableSchema {
    /// Get column names in order.
    pub fn column_names(&self) -> Vec<&str> {
        self.columns.iter().map(|c| c.name.as_str()).collect()
    }

    /// Get a column by name (case-insensitive, normalized).
    pub fn get_column(&self, name: &str) -> Option<&ColumnInfo> {
        let normalized = normalize_column_name(name);
        self.columns
            .iter()
            .find(|c| normalize_column_name(&c.name) == normalized)
    }
}

/// Normalize a column name: trim, lowercase, replace whitespace with underscores.
pub fn normalize_column_name(name: &str) -> String {
    name.trim()
        .to_lowercase()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join("_")
}

/// Parse a "schema.table" string into (Option<schema>, table).
pub fn parse_table_name(table: &str) -> (Option<String>, String) {
    if let Some(dot_pos) = table.find('.') {
        let schema = &table[..dot_pos];
        let tbl = &table[dot_pos + 1..];
        (Some(schema.to_string()), tbl.to_string())
    } else {
        (None, table.to_string())
    }
}

/// Build a `TableSchema` from `information_schema.columns` query results.
///
/// Both DuckDB and Postgres return the same column shape from this query.
/// The only difference is the default schema name ("main" vs "public"),
/// which the caller passes in.
pub fn schema_from_rows(
    rows: Vec<crate::traits::Row>,
    schema_name: Option<String>,
    table_name: String,
    default_schema: &str,
) -> Result<TableSchema, crate::traits::DatabaseError> {
    if rows.is_empty() {
        return Err(crate::traits::DatabaseError::NotFound);
    }

    let columns: Vec<ColumnInfo> = rows
        .into_iter()
        .map(|row| {
            let name = row
                .get("column_name")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let data_type = row
                .get("data_type")
                .and_then(|v| v.as_str())
                .unwrap_or("VARCHAR")
                .to_string();
            let ordinal_position = row
                .get("ordinal_position")
                .and_then(|v| v.as_i64())
                .unwrap_or(0) as usize;
            let is_nullable = row
                .get("is_nullable")
                .and_then(|v| v.as_str())
                .map(|s| s == "YES")
                .unwrap_or(true);

            ColumnInfo {
                name,
                data_type,
                ordinal_position,
                is_nullable,
            }
        })
        .collect();

    let _ = default_schema; // used by callers for the SQL query, not needed here
    Ok(TableSchema {
        schema_name,
        table_name,
        columns,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_column_name() {
        assert_eq!(normalize_column_name("userId"), "userid");
        assert_eq!(normalize_column_name("  Name  "), "name");
        assert_eq!(normalize_column_name("First Name"), "first_name");
        assert_eq!(
            normalize_column_name("  Multiple   Spaces  "),
            "multiple_spaces"
        );
    }

    #[test]
    fn test_parse_table_name() {
        let (schema, table) = parse_table_name("public.users");
        assert_eq!(schema, Some("public".to_string()));
        assert_eq!(table, "users");

        let (schema, table) = parse_table_name("users");
        assert_eq!(schema, None);
        assert_eq!(table, "users");
    }
}
