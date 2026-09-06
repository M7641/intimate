use crate::traits::{Database, DatabaseError};

/// Execute a MERGE (upsert) operation using any Database backend.
pub fn merge(db: &dyn Database, config: &MergeConfig) -> Result<u64, DatabaseError> {
    let schema = db.get_table_schema(&config.source_table)?;
    let source_columns: Vec<String> = schema.columns.iter().map(|c| c.name.clone()).collect();
    let sql = build_merge_sql(config, &source_columns);
    db.execute(&sql, &[])
}

/// Column mapping for MERGE operations.
/// Maps a column in the target table to a column in the source table.
#[derive(Debug, Clone)]
pub struct ColumnMapping {
    pub target: String,
    pub source: String,
}

impl ColumnMapping {
    /// Create a mapping where target and source column names are the same.
    pub fn same(name: &str) -> Self {
        Self {
            target: name.to_string(),
            source: name.to_string(),
        }
    }

    /// Create a mapping with different target and source column names.
    pub fn new(target: &str, source: &str) -> Self {
        Self {
            target: target.to_string(),
            source: source.to_string(),
        }
    }
}

/// Condition for matching rows between target and source tables.
#[derive(Debug, Clone)]
pub enum MergeCondition {
    /// Column equality: target.col = source.col
    Columns(Vec<ColumnMapping>),
    /// Raw SQL for complex conditions.
    RawSql(String),
}

impl MergeCondition {
    /// Create a condition matching on a single column.
    pub fn on_column(name: &str) -> Self {
        Self::Columns(vec![ColumnMapping::same(name)])
    }

    /// Create a condition matching on multiple columns.
    pub fn on_columns(names: &[&str]) -> Self {
        Self::Columns(names.iter().map(|n| ColumnMapping::same(n)).collect())
    }

    /// Create a raw SQL condition.
    pub fn raw(sql: &str) -> Self {
        Self::RawSql(sql.to_string())
    }

    /// Build the ON clause SQL.
    pub fn build_on_clause(&self, target_alias: &str, source_alias: &str) -> String {
        match self {
            MergeCondition::Columns(mappings) => mappings
                .iter()
                .map(|m| {
                    format!(
                        "{}.{} = {}.{}",
                        target_alias, m.target, source_alias, m.source
                    )
                })
                .collect::<Vec<_>>()
                .join(" AND "),
            MergeCondition::RawSql(sql) => sql.clone(),
        }
    }
}

/// Specifies which columns to update when a row matches.
#[derive(Debug, Clone)]
pub enum UpdateColumns {
    /// Update all source columns except key columns.
    All,
    /// Update only these specific columns.
    Only(Vec<ColumnMapping>),
    /// Update all columns except these.
    Except(Vec<String>),
}

/// Specifies which columns to insert when a row doesn't match.
#[derive(Debug, Clone)]
pub enum InsertColumns {
    /// Insert all source columns.
    All,
    /// Insert only these specific columns.
    Only(Vec<ColumnMapping>),
}

/// Configuration for a MERGE operation.
#[derive(Debug, Clone)]
pub struct MergeConfig {
    pub target_table: String,
    pub source_table: String,
    pub target_alias: String,
    pub source_alias: String,
    pub condition: MergeCondition,
    pub update_columns: Option<UpdateColumns>,
    pub insert_columns: Option<InsertColumns>,
}

impl MergeConfig {
    /// Create a new MergeConfig with default aliases ("t" for target, "s" for source).
    pub fn new(target_table: &str, source_table: &str, condition: MergeCondition) -> Self {
        Self {
            target_table: target_table.to_string(),
            source_table: source_table.to_string(),
            target_alias: "t".to_string(),
            source_alias: "s".to_string(),
            condition,
            update_columns: Some(UpdateColumns::All),
            insert_columns: Some(InsertColumns::All),
        }
    }

    /// Set custom target alias.
    pub fn with_target_alias(mut self, alias: &str) -> Self {
        self.target_alias = alias.to_string();
        self
    }

    /// Set custom source alias.
    pub fn with_source_alias(mut self, alias: &str) -> Self {
        self.source_alias = alias.to_string();
        self
    }

    /// Set update columns behavior.
    pub fn with_update(mut self, columns: UpdateColumns) -> Self {
        self.update_columns = Some(columns);
        self
    }

    /// Skip the WHEN MATCHED (UPDATE) clause.
    pub fn without_update(mut self) -> Self {
        self.update_columns = None;
        self
    }

    /// Set insert columns behavior.
    pub fn with_insert(mut self, columns: InsertColumns) -> Self {
        self.insert_columns = Some(columns);
        self
    }

    /// Skip the WHEN NOT MATCHED (INSERT) clause.
    pub fn without_insert(mut self) -> Self {
        self.insert_columns = None;
        self
    }
}

/// Build the SET clause for UPDATE.
pub fn build_set_clause(
    update_columns: &UpdateColumns,
    source_columns: &[String],
    key_columns: &[String],
    source_alias: &str,
) -> String {
    let columns_to_update: Vec<&String> = match update_columns {
        UpdateColumns::All => source_columns
            .iter()
            .filter(|c| !key_columns.contains(c))
            .collect(),
        UpdateColumns::Only(mappings) => {
            return mappings
                .iter()
                .map(|m| format!("{} = {}.{}", m.target, source_alias, m.source))
                .collect::<Vec<_>>()
                .join(", ");
        }
        UpdateColumns::Except(excluded) => source_columns
            .iter()
            .filter(|c| !key_columns.contains(c) && !excluded.contains(c))
            .collect(),
    };

    columns_to_update
        .iter()
        .map(|c| format!("{} = {}.{}", c, source_alias, c))
        .collect::<Vec<_>>()
        .join(", ")
}

/// Build the INSERT clause (column list and values).
pub fn build_insert_clause(
    insert_columns: &InsertColumns,
    source_columns: &[String],
    source_alias: &str,
) -> (String, String) {
    let columns: Vec<&String> = match insert_columns {
        InsertColumns::All => source_columns.iter().collect(),
        InsertColumns::Only(mappings) => {
            let cols: String = mappings
                .iter()
                .map(|m| m.target.as_str())
                .collect::<Vec<_>>()
                .join(", ");
            let vals: String = mappings
                .iter()
                .map(|m| format!("{}.{}", source_alias, m.source))
                .collect::<Vec<_>>()
                .join(", ");
            return (cols, vals);
        }
    };

    let cols = columns
        .iter()
        .map(|c| c.as_str())
        .collect::<Vec<_>>()
        .join(", ");
    let vals = columns
        .iter()
        .map(|c| format!("{}.{}", source_alias, c))
        .collect::<Vec<_>>()
        .join(", ");
    (cols, vals)
}

/// Extract key columns from MergeCondition (for UpdateColumns::All filtering).
pub fn extract_key_columns(condition: &MergeCondition) -> Vec<String> {
    match condition {
        MergeCondition::Columns(mappings) => mappings.iter().map(|m| m.target.clone()).collect(),
        MergeCondition::RawSql(_) => Vec::new(),
    }
}

/// Build the complete MERGE SQL statement.
pub fn build_merge_sql(config: &MergeConfig, source_columns: &[String]) -> String {
    let key_columns = extract_key_columns(&config.condition);
    let on_clause = config
        .condition
        .build_on_clause(&config.target_alias, &config.source_alias);

    let mut sql = format!(
        "MERGE INTO {} {}\nUSING {} {}\nON {}",
        config.target_table,
        config.target_alias,
        config.source_table,
        config.source_alias,
        on_clause
    );

    // WHEN MATCHED THEN UPDATE
    if let Some(ref update_cols) = config.update_columns {
        let set_clause = build_set_clause(
            update_cols,
            source_columns,
            &key_columns,
            &config.source_alias,
        );
        if !set_clause.is_empty() {
            sql.push_str(&format!("\nWHEN MATCHED THEN UPDATE SET {}", set_clause));
        }
    }

    // WHEN NOT MATCHED THEN INSERT
    if let Some(ref insert_cols) = config.insert_columns {
        let (cols, vals) = build_insert_clause(insert_cols, source_columns, &config.source_alias);
        if !cols.is_empty() {
            sql.push_str(&format!(
                "\nWHEN NOT MATCHED THEN INSERT ({}) VALUES ({})",
                cols, vals
            ));
        }
    }

    sql
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_merge_condition_single_column() {
        let cond = MergeCondition::on_column("id");
        assert_eq!(cond.build_on_clause("t", "s"), "t.id = s.id");
    }

    #[test]
    fn test_merge_condition_multiple_columns() {
        let cond = MergeCondition::on_columns(&["id", "version"]);
        assert_eq!(
            cond.build_on_clause("t", "s"),
            "t.id = s.id AND t.version = s.version"
        );
    }

    #[test]
    fn test_merge_condition_raw_sql() {
        let cond = MergeCondition::raw("t.id = s.id AND s.updated_at > t.updated_at");
        assert_eq!(
            cond.build_on_clause("t", "s"),
            "t.id = s.id AND s.updated_at > t.updated_at"
        );
    }

    #[test]
    fn test_build_set_clause_all() {
        let source_cols = vec!["id".to_string(), "name".to_string(), "email".to_string()];
        let key_cols = vec!["id".to_string()];
        let clause = build_set_clause(&UpdateColumns::All, &source_cols, &key_cols, "s");
        assert_eq!(clause, "name = s.name, email = s.email");
    }

    #[test]
    fn test_build_set_clause_only() {
        let mappings = vec![
            ColumnMapping::same("name"),
            ColumnMapping::new("email", "new_email"),
        ];
        let clause = build_set_clause(&UpdateColumns::Only(mappings), &[], &[], "s");
        assert_eq!(clause, "name = s.name, email = s.new_email");
    }

    #[test]
    fn test_build_set_clause_except() {
        let source_cols = vec![
            "id".to_string(),
            "name".to_string(),
            "email".to_string(),
            "created_at".to_string(),
        ];
        let key_cols = vec!["id".to_string()];
        let clause = build_set_clause(
            &UpdateColumns::Except(vec!["created_at".to_string()]),
            &source_cols,
            &key_cols,
            "s",
        );
        assert_eq!(clause, "name = s.name, email = s.email");
    }

    #[test]
    fn test_build_insert_clause_all() {
        let source_cols = vec!["id".to_string(), "name".to_string()];
        let (cols, vals) = build_insert_clause(&InsertColumns::All, &source_cols, "s");
        assert_eq!(cols, "id, name");
        assert_eq!(vals, "s.id, s.name");
    }

    #[test]
    fn test_build_merge_sql() {
        let config = MergeConfig::new("users", "staging_users", MergeCondition::on_column("id"));
        let source_cols = vec![
            "id".to_string(),
            "name".to_string(),
            "email".to_string(),
            "updated_at".to_string(),
        ];
        let sql = build_merge_sql(&config, &source_cols);

        assert!(sql.contains("MERGE INTO users t"));
        assert!(sql.contains("USING staging_users s"));
        assert!(sql.contains("ON t.id = s.id"));
        assert!(sql.contains("WHEN MATCHED THEN UPDATE SET"));
        assert!(sql.contains("name = s.name"));
        assert!(sql.contains("WHEN NOT MATCHED THEN INSERT"));
    }

    #[test]
    fn test_merge_config_without_update() {
        let config = MergeConfig::new("users", "staging_users", MergeCondition::on_column("id"))
            .without_update();
        let source_cols = vec!["id".to_string(), "name".to_string()];
        let sql = build_merge_sql(&config, &source_cols);

        assert!(!sql.contains("WHEN MATCHED"));
        assert!(sql.contains("WHEN NOT MATCHED"));
    }

    #[test]
    fn test_merge_config_without_insert() {
        let config = MergeConfig::new("users", "staging_users", MergeCondition::on_column("id"))
            .without_insert();
        let source_cols = vec!["id".to_string(), "name".to_string()];
        let sql = build_merge_sql(&config, &source_cols);

        assert!(sql.contains("WHEN MATCHED"));
        assert!(!sql.contains("WHEN NOT MATCHED"));
    }
}
