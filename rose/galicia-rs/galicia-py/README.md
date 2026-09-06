# galicia

> PoC : remplacer Redshift par **DuckDB + Iceberg sur S3** pour les charges
> analytiques d'une équipe de ~50 users. Ici, "S3" est mimé par un dossier
> local et Glue par un SQLite — l'API PyIceberg est strictement la même.

## Le pitch

| | Redshift (ra3.xlplus) | DuckDB + Iceberg/S3 |
|---|---|---|
| Coût mensuel | ~$200–2000 | ~$5–50 (S3 + compute on-demand) |
| Dev local | port-forward + creds | lire un dossier |
| Latence sur <100 GB | bonne | souvent meilleure (pas de network shuffle) |
| Time-travel | snapshots manuels | gratuit, par snapshot Iceberg |
| Évolution de schéma | `ALTER TABLE` | métadonnée, pas de réécriture |
| Portabilité dbt | ✅ | ✅ (`dbt-duckdb`, ~95% du SQL passe) |

## Architecture du PoC

```
  ┌──────────────┐    appends/overwrites    ┌─────────────────────┐
  │  PyIceberg   │ ───────────────────────► │  warehouse/         │
  │  (writer)    │                          │   ├─ catalog.db     │ ← "Glue"
  └──────────────┘                          │   └─ raw/orders/    │ ← "s3://"
                                            │       ├─ data/*.parquet
  ┌──────────────┐    iceberg_scan(...)     │       └─ metadata/*.json
  │   DuckDB     │ ◄─────────────────────── │
  │  (reader)    │                          └─────────────────────┘
  └──────────────┘
         ▲
         │  même SQL qu'en Redshift
   ┌─────┴──────┐
   │  dbt-duckdb │  (non installé ici, mais le SQL est identique)
   └────────────┘
```

## Ce que la démo montre

1. **Setup** d'un "bucket" et d'un catalogue Iceberg dans `./warehouse/`.
2. **Ingestion** initiale : `raw.customers` + `raw.orders` (jour 1).
3. **Lecture** analytique via `duckdb.iceberg_scan(metadata.json)`.
4. **Append** d'un second batch → nouveau snapshot ; **time-travel**
   sur l'ancien via `table.scan(snapshot_id=...)`.
5. **Évolution de schéma** : ajout d'une colonne `discount_cents` sans
   réécrire un seul Parquet existant.
6. **Transformation dbt-style** : DuckDB joint `orders` × `customers`,
   agrège par pays, et écrit le résultat dans `mart.revenue_by_country`
   (encore Iceberg). Le SQL est exactement celui qu'on écrirait sur
   Redshift, modulo `iceberg_scan(...)` au lieu d'un nom de table.
7. **Empreinte disque** affichée à la fin — ~quelques dizaines de KiB
   pour ce dataset jouet.

## Lancer

```bash
cd rose/galicia
uv venv --python 3.12 .venv
uv pip install --python .venv/bin/python -e .
source .venv/bin/activate

galicia demo                               # narratif end-to-end (8 sections)
```

## CLI

Une fois installé, `galicia` expose les opérations en commandes composables :

```bash
galicia init --force                       # wipe + crée raw/, mart/
galicia seed-customers --n 50
galicia seed-orders 2026-05-15 --n 200
galicia seed-orders 2026-05-16 --n 300
galicia evolve                             # + discount_cents sur raw.orders
galicia seed-orders 2026-05-17 --n 150 --with-discount
galicia mart                               # (re)build mart.revenue_by_country

galicia tables                             # raw.customers / raw.orders / mart.…
galicia snapshots raw.orders               # liste des snapshots Iceberg
galicia info                               # fichiers + taille sur disque

galicia query "SELECT country, SUM(amount_cents)/100.0 AS eur
               FROM raw.orders o JOIN raw.customers c USING (customer_id)
               GROUP BY 1 ORDER BY eur DESC"
```

Dans `query`, chaque table Iceberg est pré-enregistrée comme view DuckDB
(`raw.orders`, `mart.revenue_by_country`, …) — pas besoin d'écrire
`iceberg_scan('…/metadata.json')` à la main. C'est le même rôle que
`{{ ref('orders') }}` dans dbt.

Sortie attendue de `galicia demo` : 8 sections avec les snapshots, les counts
time-travel, le schéma évolué, et le mart final.

## Ce que ce PoC ne montre **pas** (volontairement)

- **Glue / Nessie réels** : SQLite suffit pour démontrer le contrat.
- **dbt-duckdb** : installable en `pip install dbt-duckdb`, mais le point
  intéressant est que le SQL de la section 6 marche tel quel.
- **Compaction / `OPTIMIZE`** : sur de gros volumes il faut rejouer
  `rewrite_data_files` périodiquement (cron Airflow). Hors scope ici.
- **CDC depuis RDS** : Debezium → Kinesis → un writer PyIceberg est le
  pattern attendu, mais on simule directement avec `seed.py`.
- **Permissions** : sur S3 c'est du IAM ; en local, le filesystem.

## Migration depuis Redshift — coût estimé (rappel)

- 2-3 semaines, principalement reconfiguration dbt + migration des seeds.
- 95% des modèles SQL passent sans modification (DuckDB parle dialecte
  Postgres comme Redshift).
- Pendant la transition, on peut faire tourner les deux en parallèle :
  dbt génère vers Redshift *et* vers Iceberg, on compare les outputs.
