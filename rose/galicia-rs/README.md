# galicia-rs

> Interprétation **Rust** du PoC `galicia` : remplacer Redshift par
> **DuckDB + Apache Iceberg sur S3** pour les charges analytiques d'une équipe
> de ~50 users. Ici « S3 » est mimé par un **dossier local** et « Glue » par un
> **SQLite** — l'API Iceberg est strictement la même qu'en prod.

Différence avec la version Python : on n'utilise pas PyIceberg mais le **vrai
crate `apache/iceberg-rust`** comme *writer*, et le crate `duckdb` (DuckDB
embarqué) comme *reader / moteur SQL*.

## Les trois couches (volontairement découplées)

```
   ┌───────────────────┐    fast_append / commit     ┌──────────────────────────┐
   │  iceberg-rust      │ ──────────────────────────► │  warehouse/   (= "s3://") │
   │  (writer)          │    Parquet + metadata.json  │   ├─ catalog.db   (="Glue")│
   └───────────────────┘                             │   └─ raw/orders/          │
                                                       │       ├─ data/*.parquet   │
   ┌───────────────────┐   read_parquet([fichiers])   │       └─ metadata/*.json  │
   │   DuckDB           │ ◄────────────────────────── │                            │
   │   (reader / SQL)   │   (le catalogue résout       └──────────────────────────┘
   └───────────────────┘    nom logique → fichiers)
          ▲
          │  même SQL qu'en Redshift
   ┌──────┴───────┐
   │  dbt-duckdb   │  (projet réel dans dbt/ — lit les mêmes Parquet)
   └──────────────┘
```

| Couche | Fichier | Rôle | En prod |
|---|---|---|---|
| Object store | `src/objstore.rs` | le « bucket » | `s3://bucket/` |
| Table format | `src/catalog.rs`, `src/writer.rs` | Iceberg + catalogue SQLite | Iceberg + Glue/Nessie/REST |
| Moteur | `src/warehouse.rs` | DuckDB lit les Parquet | DuckDB / dbt-duckdb |

### Le point clé : Iceberg **écrit** des fichiers, DuckDB **lit** des fichiers

`iceberg-rust` et le crate `duckdb` embarquent chacun **leur propre version
d'`arrow`** (57 vs 58). On ne fait donc jamais transiter de `RecordBatch` de
l'un à l'autre — ça ne compilerait pas. À la place :

- le writer Iceberg pose des Parquet + un `metadata.json` dans le bucket ;
- le catalogue résout « table au snapshot N → liste de fichiers » ;
- DuckDB lit cette liste via `read_parquet([...])`.

C'est exactement ce que fait `iceberg_scan('<metadata.json>')` en interne — on
le fait à la main pour rester hors-ligne (pas d'extension à télécharger) et
montrer la mécanique. La variante native est documentée dans `warehouse.rs`.

## Lancer

```bash
cd rose/galicia-rs
cargo run --release -- demo          # narratif end-to-end (8 sections)
```

Premier build : long (DuckDB est compilé depuis les sources, `bundled`).

## CLI

```bash
cargo build --release
BIN=./target/release/galicia

$BIN init --force                    # wipe + crée raw/, mart/
$BIN seed-customers --n 50
$BIN seed-orders 2026-05-15 --n 200
$BIN seed-orders 2026-05-16 --n 300
$BIN evolve                          # explique l'évolution de schéma (voir plus bas)
$BIN seed-orders 2026-05-17 --n 150 --with-discount
$BIN mart                            # (re)build mart.revenue_by_country

$BIN tables                          # raw.customers / raw.orders / mart.…
$BIN snapshots raw.orders            # liste des snapshots Iceberg
$BIN info                            # fichiers + taille sur disque

$BIN query "SELECT country, SUM(amount_cents)/100.0 AS eur
            FROM raw.orders o JOIN raw.customers c USING (customer_id)
            GROUP BY 1 ORDER BY eur DESC"
```

Dans `query`, chaque table Iceberg est pré-enregistrée comme view DuckDB
(`raw.orders`, `mart.revenue_by_country`, …) : pas besoin d'écrire
`read_parquet('…')` à la main. Même rôle que `{{ ref('orders') }}` dans dbt.

## dbt

Le dossier `dbt/` est un **vrai projet dbt-duckdb**. Le modèle
`models/mart/revenue_by_country.sql` est *le même SQL* que la constante
`MART_SQL` de `src/pipeline.rs` :

```bash
pip install dbt-duckdb
cd dbt
GALICIA_WAREHOUSE=../warehouse dbt run     # construit le mart depuis les Parquet
```

- `models/sources.yml` pointe `raw.*` sur les Parquet du bucket via
  `external_location: read_parquet(...)`.
- `galicia mart` et `dbt run` produisent **la même table** à partir des **mêmes
  fichiers**. C'est tout l'intérêt : le SQL est portable, seul le câblage
  source→fichiers change entre Redshift, DuckDB et dbt.

Le glob `data/*.parquet` lit le **snapshot courant** (append-only). Le
time-travel par snapshot reste l'affaire du catalogue Iceberg (`galicia
snapshots`, `count_at`), pas de dbt.

## Ce que la démo montre (8 sections)

1. **Setup** : un dossier = `s3://galicia/`, un SQLite = Glue.
2. **Ingestion** : `raw.customers` + `raw.orders` (jour 1) via le writer Iceberg.
3. **Lecture** : DuckDB agrège `raw.orders` (lecture des Parquet Iceberg).
4. **Append + time-travel** : jour 2 → nouveau snapshot ; l'ancien reste
   interrogeable (`count_at(snap1)` = 200, courant = 500).
5. **« Évolution » de schéma** : `discount_cents` populé partiellement (voir
   caveat ci-dessous).
6. **Transformation dbt-style** : DuckDB joint `orders × customers`, agrège par
   pays, et réécrit le résultat dans `mart.revenue_by_country` (encore Iceberg).
7. **Lecture du mart**.
8. **Empreinte disque** du bucket.

## Caveats (honnêtes) propres à la version Rust

- **Évolution de schéma** : `apache/iceberg-rust` 0.9 n'expose **pas encore**
  `update_schema` via l'API `Transaction` (uniquement `fast_append`,
  `update_table_properties`, `replace_sort_order`, `update_location`,
  `update_statistics`, `upgrade_table_version`). PyIceberg, lui, a
  `update_schema().add_column()`. On déclare donc `discount_cents` **optionnel
  dès la création** — l'état observable (NULL sur les vieux fichiers, valeurs
  sur les nouveaux) est identique à un `ALTER TABLE ADD COLUMN`.
- **Overwrite / delete** : pas d'action `overwrite` non plus en 0.9. Le rebuild
  du mart fait donc **drop + recreate + append**.
- **Catalogue** : `SqlCatalog` sur SQLite, comme PyIceberg. En prod : Glue,
  Nessie ou un catalogue REST — il suffit d'échanger le `CatalogBuilder`.
- **S3 réel** : renvoyer `s3://bucket/prefix` depuis `ObjectStore::warehouse_uri`
  et injecter un `S3StorageFactory` + creds dans les props du catalogue. Rien
  d'autre ne change.

## Comparaison avec le PoC Python (`../galicia`)

| | Python (`galicia`) | Rust (`galicia-rs`) |
|---|---|---|
| Writer | PyIceberg | `iceberg-rust` (writer bas-niveau explicite) |
| Reader / SQL | DuckDB + `iceberg_scan()` | DuckDB + `read_parquet()` sur fichiers résolus |
| Catalogue | `SqlCatalog` (SQLite) | `SqlCatalog` (SQLite) |
| Évolution de schéma | `update_schema()` réel | colonne optionnelle (caveat) |
| dbt | mentionné | projet `dbt/` réel |
