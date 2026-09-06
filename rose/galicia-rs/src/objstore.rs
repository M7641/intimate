//! Le « bucket S3 » — mimé par un répertoire local.
//!
//! ## Le seam S3 ↔ local
//!
//! En prod, le warehouse Iceberg vit sur un object store : `s3://my-bucket/`.
//! Ici on pointe sur un dossier. **Rien d'autre ne change** dans le reste du
//! code : Iceberg parle à son `FileIO` (basé sur `opendal`) via une URI, et
//! `opendal` route `file://` vers le filesystem ou `s3://` vers AWS.
//!
//! Pour basculer en vrai S3, il suffirait de :
//! - renvoyer `s3://bucket/prefix` depuis [`ObjectStore::warehouse_uri`] ;
//! - passer les credentials S3 dans les propriétés du catalogue
//!   (`s3.access-key-id`, `s3.secret-access-key`, `s3.region`, `s3.endpoint`…).
//!
//! Le catalogue (« Glue ») reste lui un SQLite local dans les deux cas — en
//! prod on le remplacerait par Glue, Nessie ou un catalogue REST.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

/// Un object store adressé par clé. Ici : un dossier sur disque.
#[derive(Clone, Debug)]
pub struct ObjectStore {
    root: PathBuf,
}

impl ObjectStore {
    /// Ancre le « bucket » sur `root` (créé s'il n'existe pas).
    pub fn new(root: impl AsRef<Path>) -> Result<Self> {
        let root = root.as_ref();
        std::fs::create_dir_all(root)
            .with_context(|| format!("création du warehouse {}", root.display()))?;
        let root = root
            .canonicalize()
            .with_context(|| format!("canonicalize {}", root.display()))?;
        Ok(Self { root })
    }

    /// Chemin local du bucket.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// L'URI passée au `FileIO` d'Iceberg. En prod : `s3://bucket/prefix`.
    pub fn warehouse_uri(&self) -> String {
        format!("file://{}", self.root.display())
    }

    /// L'URI sqlx du catalogue (le rôle de « Glue »). En prod : Glue/Nessie/REST.
    pub fn catalog_uri(&self) -> String {
        // `?mode=rwc` => sqlx crée le fichier SQLite s'il n'existe pas.
        format!("sqlite://{}/catalog.db?mode=rwc", self.root.display())
    }

    /// Y a-t-il déjà un warehouse matérialisé ?
    pub fn exists(&self) -> bool {
        self.root.join("catalog.db").exists()
    }

    /// Empreinte disque du bucket : (nombre de fichiers, octets cumulés).
    /// En prod c'est ce qui se facture ~0,023 $/Go/mois sur S3.
    pub fn footprint(&self) -> Result<(usize, u64)> {
        let mut n = 0usize;
        let mut total = 0u64;
        let mut stack = vec![self.root.clone()];
        while let Some(dir) = stack.pop() {
            for entry in std::fs::read_dir(&dir)? {
                let entry = entry?;
                let meta = entry.metadata()?;
                if meta.is_dir() {
                    stack.push(entry.path());
                } else {
                    n += 1;
                    total += meta.len();
                }
            }
        }
        Ok((n, total))
    }

    /// Convertit un chemin/URI renvoyé par Iceberg (souvent `file:///abs`) en
    /// chemin local exploitable par DuckDB (`read_parquet`).
    pub fn strip_scheme(uri: &str) -> String {
        uri.strip_prefix("file://").unwrap_or(uri).to_string()
    }
}
