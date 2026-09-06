use database::Row;
use serde::Serialize;
use utoipa::ToSchema;

/// Shared response type for all warehouse analytics endpoints.
/// Matches the Python `{"data": [...], "columns": [...]}` format.
#[derive(Serialize, ToSchema)]
pub struct WarehouseResponse {
    /// Dynamic row data as JSON objects
    #[schema(value_type = Vec<serde_json::Value>)]
    pub data: Vec<Row>,
    /// Column names extracted from the first row
    pub columns: Vec<String>,
}

impl WarehouseResponse {
    /// Build a response from rows, extracting column names from the first row.
    pub fn from_rows(rows: Vec<Row>) -> Self {
        let columns = rows
            .first()
            .map(|row| {
                let mut cols: Vec<String> = row.keys().cloned().collect();
                cols.sort();
                cols
            })
            .unwrap_or_default();
        Self {
            data: rows,
            columns,
        }
    }
}
