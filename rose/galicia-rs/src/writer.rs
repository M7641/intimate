//! Le writer Iceberg : RecordBatch Arrow → Parquet → nouveau snapshot.
//!
//! C'est le pipeline bas-niveau d'`iceberg-rust`. PyIceberg cachait tout ça
//! derrière `table.append(arrow_table)` ; en Rust on assemble la chaîne
//! explicitement, ce qui montre bien les étapes :
//!
//! ```text
//!   RecordBatch
//!     └─ DataFileWriter            (logique : 1 fichier de données)
//!          └─ RollingFileWriter    (coupe en plusieurs fichiers si trop gros)
//!               └─ ParquetWriter   (physique : encode le Parquet)
//!   => Vec<DataFile> (descripteurs)
//!     └─ Transaction::fast_append  (ajoute les fichiers, crée un snapshot)
//!          └─ commit               (atomique, écrit un nouveau metadata.json)
//! ```

use std::sync::Arc;

use anyhow::{Context, Result};
use arrow_array::RecordBatch;
use iceberg::spec::DataFileFormat;
use iceberg::table::Table;
use iceberg::transaction::{ApplyTransactionAction, Transaction};
use iceberg::writer::base_writer::data_file_writer::DataFileWriterBuilder;
use iceberg::writer::file_writer::location_generator::{
    DefaultFileNameGenerator, DefaultLocationGenerator,
};
use iceberg::writer::file_writer::rolling_writer::RollingFileWriterBuilder;
use iceberg::writer::file_writer::ParquetWriterBuilder;
use iceberg::writer::{IcebergWriter, IcebergWriterBuilder};
use iceberg::Catalog;
use parquet::file::properties::WriterProperties;

/// Écrit `batch` comme un (ou plusieurs) Parquet sous la table, puis crée un
/// nouveau snapshot par `fast_append`. Renvoie la table rafraîchie.
pub async fn append(
    catalog: &Arc<dyn Catalog>,
    table: &Table,
    batch: RecordBatch,
) -> Result<Table> {
    let parquet_builder = ParquetWriterBuilder::new(
        WriterProperties::default(),
        table.metadata().current_schema().clone(),
    );

    let location_gen = DefaultLocationGenerator::new(table.metadata().clone())
        .context("DefaultLocationGenerator")?;
    // Le générateur repart à 00000 à chaque writer ; on préfixe par le nombre
    // de snapshots déjà présents pour garantir des noms de fichiers uniques
    // entre appends successifs (sinon collision / "file already referenced").
    let seq = table.metadata().snapshots().count();
    let file_name_gen =
        DefaultFileNameGenerator::new(format!("data-{seq}"), None, DataFileFormat::Parquet);

    let rolling = RollingFileWriterBuilder::new_with_default_file_size(
        parquet_builder,
        table.file_io().clone(),
        location_gen,
        file_name_gen,
    );

    let mut writer = DataFileWriterBuilder::new(rolling)
        .build(None)
        .await
        .context("build DataFileWriter")?;

    writer.write(batch).await.context("write batch")?;
    let data_files = writer.close().await.context("close writer")?;

    let tx = Transaction::new(table);
    let action = tx.fast_append().add_data_files(data_files);
    let tx = action.apply(tx).context("apply fast_append")?;
    let table = tx.commit(catalog.as_ref()).await.context("commit")?;
    Ok(table)
}
