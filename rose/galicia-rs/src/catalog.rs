//! Le catalogue Iceberg (le rôle de « Glue ») + les schémas des tables.
//!
//! On utilise un `SqlCatalog` sur SQLite : exactement l'analogue du
//! `SqlCatalog` de PyIceberg. Il stocke « table logique → emplacement du
//! metadata.json courant ». En prod on brancherait Glue, Nessie ou REST sans
//! toucher au reste.

use std::collections::HashMap;
use std::sync::Arc;

use anyhow::{Context, Result};
use iceberg::io::LocalFsStorageFactory;
use iceberg::spec::{NestedField, PrimitiveType, Schema, Type};
use iceberg::{Catalog, CatalogBuilder, NamespaceIdent, TableCreation, TableIdent};
use iceberg_catalog_sql::{SqlCatalogBuilder, SQL_CATALOG_PROP_URI, SQL_CATALOG_PROP_WAREHOUSE};

use crate::objstore::ObjectStore;

/// Les deux namespaces du PoC : données brutes ingérées, et marts dérivés.
pub const NAMESPACES: [&str; 2] = ["raw", "mart"];

/// Explication affichée par la commande `evolve` / l'étape 5 de la démo.
pub const EVOLVE_NOTE: &str = "note : apache/iceberg-rust 0.9 n'expose pas encore \
update_schema via l'API Transaction (PyIceberg : update_schema().add_column()). \
On déclare donc discount_cents optionnel dès la création — même état observable \
qu'un ALTER TABLE ADD COLUMN : NULL sur les anciens fichiers, valeurs sur les nouveaux.";

/// Ouvre (ou crée) le catalogue SQLite ancré sur le bucket.
pub async fn open(store: &ObjectStore) -> Result<Arc<dyn Catalog>> {
    let catalog = SqlCatalogBuilder::default()
        // La StorageFactory mappe une URI -> FileIO. Ici : filesystem local.
        // En vrai S3 : un S3StorageFactory + creds dans les props.
        .with_storage_factory(Arc::new(LocalFsStorageFactory))
        .load(
            "galicia",
            HashMap::from([
                (SQL_CATALOG_PROP_URI.to_string(), store.catalog_uri()),
                (
                    SQL_CATALOG_PROP_WAREHOUSE.to_string(),
                    store.warehouse_uri(),
                ),
            ]),
        )
        .await
        .context("ouverture du SqlCatalog")?;
    Ok(Arc::new(catalog))
}

/// Crée les namespaces `raw` et `mart` s'ils manquent (idempotent).
pub async fn ensure_namespaces(catalog: &Arc<dyn Catalog>) -> Result<()> {
    for ns in NAMESPACES {
        let ident = NamespaceIdent::new(ns.to_string());
        if !catalog.namespace_exists(&ident).await? {
            catalog.create_namespace(&ident, HashMap::new()).await?;
        }
    }
    Ok(())
}

/// `"raw.orders"` → `TableIdent`.
pub fn ident(dotted: &str) -> Result<TableIdent> {
    let (ns, name) = dotted
        .split_once('.')
        .with_context(|| format!("identifiant de table invalide : {dotted}"))?;
    Ok(TableIdent::from_strs([ns, name])?)
}

/// Charge une table, ou la crée avec `schema` si elle n'existe pas encore.
pub async fn create_if_absent(
    catalog: &Arc<dyn Catalog>,
    dotted: &str,
    schema: Schema,
) -> Result<iceberg::table::Table> {
    let id = ident(dotted)?;
    if catalog.table_exists(&id).await? {
        return Ok(catalog.load_table(&id).await?);
    }
    let ns = NamespaceIdent::new(id.namespace().to_url_string());
    let creation = TableCreation::builder()
        .name(id.name().to_string())
        .schema(schema)
        .build();
    Ok(catalog.create_table(&ns, creation).await?)
}

// ───────────────────────── schémas des tables ─────────────────────────
//
// Les IDs de champ (1, 2, …) sont la clé de voûte d'Iceberg : c'est par eux
// (et pas par le nom ou la position) qu'on suit une colonne à travers les
// snapshots. C'est ce qui rend l'évolution de schéma « gratuite ».

pub fn customers_schema() -> Schema {
    Schema::builder()
        .with_schema_id(0)
        .with_fields(vec![
            NestedField::required(1, "customer_id", Type::Primitive(PrimitiveType::Long)).into(),
            NestedField::optional(2, "name", Type::Primitive(PrimitiveType::String)).into(),
            NestedField::optional(3, "country", Type::Primitive(PrimitiveType::String)).into(),
            NestedField::optional(4, "signup_at", Type::Primitive(PrimitiveType::Timestamp)).into(),
        ])
        .build()
        .expect("schéma customers valide")
}

pub fn orders_schema() -> Schema {
    // NB : `discount_cents` est déclaré optionnel DÈS LA CRÉATION. apache/
    // iceberg-rust 0.9 n'expose pas encore `update_schema` via l'API
    // `Transaction` (uniquement fast_append/props/sort-order/location/stats).
    // On modélise donc l'état post-évolution : une colonne optionnelle,
    // populée partiellement. Le résultat observable (NULL avant, valeurs après)
    // est identique à un vrai `ALTER TABLE ADD COLUMN`.
    Schema::builder()
        .with_schema_id(0)
        .with_fields(vec![
            NestedField::required(1, "order_id", Type::Primitive(PrimitiveType::Long)).into(),
            NestedField::required(2, "customer_id", Type::Primitive(PrimitiveType::Long)).into(),
            NestedField::required(3, "amount_cents", Type::Primitive(PrimitiveType::Long)).into(),
            NestedField::optional(4, "currency", Type::Primitive(PrimitiveType::String)).into(),
            NestedField::optional(5, "status", Type::Primitive(PrimitiveType::String)).into(),
            NestedField::optional(6, "created_at", Type::Primitive(PrimitiveType::Timestamp))
                .into(),
            NestedField::optional(7, "discount_cents", Type::Primitive(PrimitiveType::Int)).into(),
        ])
        .build()
        .expect("schéma orders valide")
}

pub fn mart_schema() -> Schema {
    Schema::builder()
        .with_schema_id(0)
        .with_fields(vec![
            NestedField::optional(1, "country", Type::Primitive(PrimitiveType::String)).into(),
            NestedField::optional(2, "paid_orders", Type::Primitive(PrimitiveType::Long)).into(),
            NestedField::optional(
                3,
                "gross_revenue_eur",
                Type::Primitive(PrimitiveType::Double),
            )
            .into(),
            NestedField::optional(
                4,
                "total_discount_eur",
                Type::Primitive(PrimitiveType::Double),
            )
            .into(),
        ])
        .build()
        .expect("schéma mart valide")
}
