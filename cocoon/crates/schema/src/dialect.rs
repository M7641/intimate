use polars::prelude::*;

// ── SQL dialect mapping ────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SqlDialect {
    DuckDb,
    Postgres,
    Redshift,
    Snowflake,
}

/// Map a Polars `DataType` to the equivalent SQL type for a given dialect.
pub fn dtype_to_sql(dtype: &DataType, dialect: SqlDialect) -> String {
    use SqlDialect::*;
    match dtype {
        DataType::Boolean => "BOOLEAN".into(),

        // Signed integers — universal
        DataType::Int8 => match dialect {
            DuckDb => "TINYINT",
            Postgres | Redshift | Snowflake => "SMALLINT",
        }
        .into(),
        DataType::Int16 => "SMALLINT".into(),
        DataType::Int32 => "INTEGER".into(),
        DataType::Int64 => "BIGINT".into(),

        // Unsigned integers — DuckDB native, others upsize
        DataType::UInt8 => match dialect {
            DuckDb => "UTINYINT",
            _ => "SMALLINT",
        }
        .into(),
        DataType::UInt16 => match dialect {
            DuckDb => "USMALLINT",
            _ => "INTEGER",
        }
        .into(),
        DataType::UInt32 => match dialect {
            DuckDb => "UINTEGER",
            _ => "BIGINT",
        }
        .into(),
        DataType::UInt64 => match dialect {
            DuckDb => "UBIGINT",
            Snowflake => "NUMBER(20,0)",
            Postgres | Redshift => "NUMERIC(20)",
        }
        .into(),

        // Floats
        DataType::Float32 => match dialect {
            DuckDb | Snowflake => "FLOAT",
            Postgres | Redshift => "REAL",
        }
        .into(),
        DataType::Float64 => match dialect {
            DuckDb => "DOUBLE",
            Postgres | Redshift => "DOUBLE PRECISION",
            Snowflake => "DOUBLE",
        }
        .into(),

        // Text and binary
        DataType::String => match dialect {
            DuckDb | Snowflake => "VARCHAR",
            Postgres => "TEXT",
            Redshift => "VARCHAR(MAX)",
        }
        .into(),
        DataType::Binary => match dialect {
            DuckDb => "BLOB",
            Postgres => "BYTEA",
            Redshift => "VARBYTE",
            Snowflake => "BINARY",
        }
        .into(),

        // Temporal
        DataType::Date => "DATE".into(),
        DataType::Time => "TIME".into(),
        DataType::Datetime(_, _) => "TIMESTAMP".into(),
        DataType::Duration(_) => match dialect {
            DuckDb | Postgres => "INTERVAL",
            Redshift | Snowflake => "VARCHAR",
        }
        .into(),

        // Null — pick a permissive text type
        DataType::Null => match dialect {
            Postgres => "TEXT",
            _ => "VARCHAR",
        }
        .into(),

        // Nested: only DuckDB has native struct/array syntax
        DataType::Struct(fields) => match dialect {
            DuckDb => {
                let inner: Vec<String> = fields
                    .iter()
                    .map(|f| format!("{} {}", f.name(), dtype_to_sql(f.dtype(), dialect)))
                    .collect();
                format!("STRUCT({})", inner.join(", "))
            }
            Postgres => "JSONB".into(),
            Redshift => "SUPER".into(),
            Snowflake => "VARIANT".into(),
        },
        DataType::List(inner) => match dialect {
            DuckDb => format!("{}[]", dtype_to_sql(inner, dialect)),
            Postgres => "JSONB".into(),
            Redshift => "SUPER".into(),
            Snowflake => "ARRAY".into(),
        },

        other => format!("VARCHAR /* unmapped: {other} */"),
    }
}

// ── Tests ──────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ColumnDef;

    #[test]
    fn sql_primitives_duckdb() {
        use SqlDialect::DuckDb;
        assert_eq!(dtype_to_sql(&DataType::Int64, DuckDb), "BIGINT");
        assert_eq!(dtype_to_sql(&DataType::Int32, DuckDb), "INTEGER");
        assert_eq!(dtype_to_sql(&DataType::Float64, DuckDb), "DOUBLE");
        assert_eq!(dtype_to_sql(&DataType::String, DuckDb), "VARCHAR");
        assert_eq!(dtype_to_sql(&DataType::Boolean, DuckDb), "BOOLEAN");
        assert_eq!(dtype_to_sql(&DataType::Date, DuckDb), "DATE");
        assert_eq!(dtype_to_sql(&DataType::Binary, DuckDb), "BLOB");
    }

    #[test]
    fn sql_primitives_postgres() {
        use SqlDialect::Postgres;
        assert_eq!(dtype_to_sql(&DataType::Int64, Postgres), "BIGINT");
        assert_eq!(
            dtype_to_sql(&DataType::Float64, Postgres),
            "DOUBLE PRECISION"
        );
        assert_eq!(dtype_to_sql(&DataType::Float32, Postgres), "REAL");
        assert_eq!(dtype_to_sql(&DataType::String, Postgres), "TEXT");
        assert_eq!(dtype_to_sql(&DataType::Binary, Postgres), "BYTEA");
    }

    #[test]
    fn sql_primitives_redshift() {
        use SqlDialect::Redshift;
        assert_eq!(dtype_to_sql(&DataType::String, Redshift), "VARCHAR(MAX)");
        assert_eq!(dtype_to_sql(&DataType::Binary, Redshift), "VARBYTE");
        assert_eq!(
            dtype_to_sql(&DataType::Float64, Redshift),
            "DOUBLE PRECISION"
        );
    }

    #[test]
    fn sql_primitives_snowflake() {
        use SqlDialect::Snowflake;
        assert_eq!(dtype_to_sql(&DataType::String, Snowflake), "VARCHAR");
        assert_eq!(dtype_to_sql(&DataType::Binary, Snowflake), "BINARY");
        assert_eq!(dtype_to_sql(&DataType::Float64, Snowflake), "DOUBLE");
        assert_eq!(dtype_to_sql(&DataType::UInt64, Snowflake), "NUMBER(20,0)");
    }

    #[test]
    fn sql_unsigned_upsize() {
        // DuckDB has native unsigned types
        assert_eq!(
            dtype_to_sql(&DataType::UInt32, SqlDialect::DuckDb),
            "UINTEGER"
        );
        // Others upsize to next signed type
        assert_eq!(
            dtype_to_sql(&DataType::UInt32, SqlDialect::Postgres),
            "BIGINT"
        );
        assert_eq!(
            dtype_to_sql(&DataType::UInt8, SqlDialect::Redshift),
            "SMALLINT"
        );
    }

    #[test]
    fn sql_nested_duckdb() {
        let struct_dt = DataType::Struct(vec![
            Field::new("name".into(), DataType::String),
            Field::new("age".into(), DataType::Int32),
        ]);
        assert_eq!(
            dtype_to_sql(&struct_dt, SqlDialect::DuckDb),
            "STRUCT(name VARCHAR, age INTEGER)"
        );

        let list_dt = DataType::List(Box::new(DataType::Int64));
        assert_eq!(dtype_to_sql(&list_dt, SqlDialect::DuckDb), "BIGINT[]");
    }

    #[test]
    fn sql_nested_other_dialects() {
        let struct_dt = DataType::Struct(vec![Field::new("x".into(), DataType::String)]);
        assert_eq!(dtype_to_sql(&struct_dt, SqlDialect::Postgres), "JSONB");
        assert_eq!(dtype_to_sql(&struct_dt, SqlDialect::Redshift), "SUPER");
        assert_eq!(dtype_to_sql(&struct_dt, SqlDialect::Snowflake), "VARIANT");

        let list_dt = DataType::List(Box::new(DataType::String));
        assert_eq!(dtype_to_sql(&list_dt, SqlDialect::Postgres), "JSONB");
        assert_eq!(dtype_to_sql(&list_dt, SqlDialect::Redshift), "SUPER");
        assert_eq!(dtype_to_sql(&list_dt, SqlDialect::Snowflake), "ARRAY");
    }

    #[test]
    fn sql_column_formatting() {
        let col = ColumnDef {
            name: "score".to_string(),
            dtype: DataType::Float64,
            constraints: None,
        };
        assert_eq!(col.to_sql_column(SqlDialect::DuckDb), "score DOUBLE");
        assert_eq!(
            col.to_sql_column(SqlDialect::Postgres),
            "score DOUBLE PRECISION"
        );
    }
}
