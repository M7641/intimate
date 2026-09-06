use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Range {
    pub min: i64,
    pub max: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Validations {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unique: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub range: Option<Range>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub valid_values: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColumnSchema {
    #[serde(rename = "type")]
    pub column_type: String,
    pub nullable: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub validations: Option<Validations>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Schema {
    pub name: String,
    pub version: String,
    pub columns: HashMap<String, ColumnSchema>,
}

impl Schema {
    /// Load schema from a JSON file
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self, Box<dyn std::error::Error>> {
        let contents = fs::read_to_string(path)?;
        let schema: Schema = serde_json::from_str(&contents)?;
        Ok(schema)
    }

    /// Load schema from a JSON string
    pub fn from_json(json_str: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let schema: Schema = serde_json::from_str(json_str)?;
        Ok(schema)
    }

    /// Save schema to a JSON file
    pub fn to_file<P: AsRef<Path>>(&self, path: P) -> Result<(), Box<dyn std::error::Error>> {
        let json = serde_json::to_string_pretty(self)?;
        fs::write(path, json)?;
        Ok(())
    }

    /// Get column types as a HashMap for validation
    pub fn get_column_types(&self) -> HashMap<String, String> {
        self.columns
            .iter()
            .map(|(name, schema)| (name.clone(), schema.column_type.clone()))
            .collect()
    }

    /// Get list of non-nullable columns
    pub fn get_required_columns(&self) -> Vec<String> {
        self.columns
            .iter()
            .filter(|(_, schema)| !schema.nullable)
            .map(|(name, _)| name.clone())
            .collect()
    }

    /// Get list of unique columns (from validations)
    pub fn get_unique_columns(&self) -> Vec<&str> {
        self.columns
            .iter()
            .filter(|(_, schema)| {
                schema
                    .validations
                    .as_ref()
                    .and_then(|v| v.unique)
                    .unwrap_or(false)
            })
            .map(|(name, _)| name.as_str())
            .collect()
    }

    /// Get list of columns with range validations
    /// Returns (column_name, min, max) tuples
    pub fn get_range_validations(&self) -> Vec<(&str, i64, i64)> {
        self.columns
            .iter()
            .filter_map(|(name, schema)| {
                schema
                    .validations
                    .as_ref()
                    .and_then(|v| v.range.as_ref())
                    .map(|range| (name.as_str(), range.min, range.max))
            })
            .collect()
    }

    /// Get list of columns with valid_values validations
    /// Returns (column_name, valid_values) tuples
    pub fn get_valid_values_validations(&self) -> Vec<(&str, &Vec<String>)> {
        self.columns
            .iter()
            .filter_map(|(name, schema)| {
                schema
                    .validations
                    .as_ref()
                    .and_then(|v| v.valid_values.as_ref())
                    .map(|values| (name.as_str(), values))
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_schema_serialization() {
        let mut columns = HashMap::new();
        columns.insert(
            "id".to_string(),
            ColumnSchema {
                column_type: "int64".to_string(),
                nullable: false,
                description: Some("Unique identifier".to_string()),
                validations: Some(Validations {
                    unique: Some(true),
                    range: None,
                    valid_values: None,
                }),
            },
        );
        columns.insert(
            "name".to_string(),
            ColumnSchema {
                column_type: "string".to_string(),
                nullable: false,
                description: Some("Person's name".to_string()),
                validations: None,
            },
        );

        let schema = Schema {
            name: "test_schema".to_string(),
            version: "1.0.0".to_string(),
            columns,
        };

        let json = serde_json::to_string_pretty(&schema).unwrap();
        let deserialized: Schema = serde_json::from_str(&json).unwrap();

        assert_eq!(schema.name, deserialized.name);
        assert_eq!(schema.columns.len(), deserialized.columns.len());
    }

    #[test]
    fn test_get_unique_columns() {
        let mut columns = HashMap::new();
        columns.insert(
            "id".to_string(),
            ColumnSchema {
                column_type: "int64".to_string(),
                nullable: false,
                description: Some("Unique identifier".to_string()),
                validations: Some(Validations {
                    unique: Some(true),
                    range: None,
                    valid_values: None,
                }),
            },
        );
        columns.insert(
            "email".to_string(),
            ColumnSchema {
                column_type: "string".to_string(),
                nullable: false,
                description: Some("Email address".to_string()),
                validations: Some(Validations {
                    unique: Some(true),
                    range: None,
                    valid_values: None,
                }),
            },
        );
        columns.insert(
            "name".to_string(),
            ColumnSchema {
                column_type: "string".to_string(),
                nullable: false,
                description: Some("Person's name".to_string()),
                validations: None,
            },
        );

        let schema = Schema {
            name: "test_schema".to_string(),
            version: "1.0.0".to_string(),
            columns,
        };

        let unique_cols = schema.get_unique_columns();
        assert_eq!(unique_cols.len(), 2);
        assert!(unique_cols.contains(&"id"));
        assert!(unique_cols.contains(&"email"));
    }
}
