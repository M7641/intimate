//! Le moteur de requête : DuckDB.
//!
//! DuckDB ne connaît qu'une chose : des fichiers Parquet. Le catalogue Iceberg
//! lui résout « table logique au snapshot N → liste de fichiers », et on lui
//! passe cette liste via `read_parquet([...])`. C'est exactement ce que fait
//! `iceberg_scan()` en interne (cf. la note plus bas).
//!
//! On enregistre chaque table Iceberg comme une *view* DuckDB pour pouvoir
//! écrire `SELECT * FROM raw.orders` au lieu d'un `read_parquet('…')` à la main.
//! Même rôle que `{{ ref('orders') }}` dans dbt.

use anyhow::{Context, Result};
use duckdb::arrow::util::display::{ArrayFormatter, FormatOptions};
use duckdb::Connection;
use futures::TryStreamExt;
use iceberg::table::Table;

use crate::objstore::ObjectStore;

/// Une connexion DuckDB en mémoire (le « compute on-demand »).
pub fn open() -> Result<Connection> {
    Connection::open_in_memory().context("connexion DuckDB")
}

/// Résout les fichiers de données d'une table à un snapshot donné (ou le
/// snapshot courant si `None`). C'est *le* rôle du catalogue : nom → fichiers.
pub async fn data_files(table: &Table, snapshot: Option<i64>) -> Result<Vec<String>> {
    let mut builder = table.scan().select_all();
    if let Some(id) = snapshot {
        builder = builder.snapshot_id(id);
    }
    let scan = builder.build().context("build scan")?;
    let tasks = scan
        .plan_files()
        .await
        .context("plan_files")?
        .try_collect::<Vec<_>>()
        .await
        .context("collecte des FileScanTask")?;
    Ok(tasks
        .iter()
        .map(|t| ObjectStore::strip_scheme(&t.data_file_path))
        .collect())
}

/// Construit l'expression DuckDB qui lit la table au snapshot courant.
///
/// > **Variante native** : avec l'extension Iceberg de DuckDB
/// > (`INSTALL iceberg; LOAD iceberg;`, nécessite le réseau au 1er run), on
/// > écrirait simplement `iceberg_scan('<metadata.json>')`. On préfère ici
/// > `read_parquet([...])` sur les fichiers résolus par le catalogue : aucune
/// > extension, hors-ligne, et ça montre la mécanique à nu.
pub async fn scan_expr(table: &Table) -> Result<String> {
    let files = data_files(table, None).await?;
    Ok(read_parquet_expr(&files))
}

fn read_parquet_expr(files: &[String]) -> String {
    let list = files
        .iter()
        .map(|f| format!("'{f}'"))
        .collect::<Vec<_>>()
        .join(", ");
    format!("read_parquet([{list}], union_by_name = true)")
}

/// Enregistre chaque table `(namespace, nom, &Table)` comme view DuckDB.
pub async fn register_views(con: &Connection, tables: &[(&str, &str, &Table)]) -> Result<()> {
    for (ns, name, table) in tables {
        let files = data_files(table, None).await?;
        if files.is_empty() {
            continue;
        }
        con.execute_batch(&format!("CREATE SCHEMA IF NOT EXISTS \"{ns}\""))?;
        let expr = read_parquet_expr(&files);
        con.execute_batch(&format!(
            "CREATE OR REPLACE VIEW \"{ns}\".\"{name}\" AS SELECT * FROM {expr}"
        ))
        .with_context(|| format!("register view {ns}.{name}"))?;
    }
    Ok(())
}

/// Exécute un SQL et imprime le résultat sous forme de table alignée.
///
/// On passe par `query_arrow` : avec DuckDB les métadonnées de colonnes ne sont
/// disponibles qu'après exécution, et l'`ArrayFormatter` d'Arrow sait afficher
/// n'importe quel type proprement.
pub fn show(con: &Connection, sql: &str) -> Result<()> {
    let mut stmt = con.prepare(sql).context("prepare")?;
    let arrow = stmt.query_arrow([]).context("query_arrow")?;
    let schema = arrow.get_schema();
    let headers: Vec<String> = schema
        .fields()
        .iter()
        .map(|f| f.name().to_string())
        .collect();
    let batches: Vec<_> = arrow.collect();

    let opts = FormatOptions::default().with_null("NULL");
    let mut data: Vec<Vec<String>> = Vec::new();
    for batch in &batches {
        let fmts = (0..batch.num_columns())
            .map(|c| ArrayFormatter::try_new(batch.column(c), &opts))
            .collect::<std::result::Result<Vec<_>, _>>()
            .context("ArrayFormatter")?;
        for row in 0..batch.num_rows() {
            data.push(fmts.iter().map(|f| f.value(row).to_string()).collect());
        }
    }

    if headers.is_empty() {
        println!("(0 lignes)");
        return Ok(());
    }

    let widths: Vec<usize> = (0..headers.len())
        .map(|i| {
            data.iter()
                .map(|r| r[i].len())
                .chain(std::iter::once(headers[i].len()))
                .max()
                .unwrap_or(0)
        })
        .collect();

    let line = |cells: &[String]| {
        cells
            .iter()
            .enumerate()
            .map(|(i, c)| format!("{c:<width$}", width = widths[i]))
            .collect::<Vec<_>>()
            .join("  ")
    };
    println!("{}", line(&headers));
    println!(
        "{}",
        widths
            .iter()
            .map(|w| "-".repeat(*w))
            .collect::<Vec<_>>()
            .join("  ")
    );
    for r in &data {
        println!("{}", line(r));
    }
    println!("({} lignes)", data.len());
    Ok(())
}
