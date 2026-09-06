
> Un panorama opinionné de la meilleure façon de **écrire**, **gérer** et **explorer**
> des bases de données en 2026 — calibré pour ce monorepo (Rust + Python + TS,
> Postgres analytics + SQLite event-sourced).
>
> Point de départ : DBeaver, sans outils IA, UX peu agréable.
> Objectif : peindre la cible, puis tracer un chemin pour y aller.

---

## TL;DR — la pile recommandée pour toi

| Besoin                                 | Outil aujourd'hui               | Cible recommandée                                                       | Pourquoi                                                              |
| -------------------------------------- | ------------------------------- | ----------------------------------------------------------------------- | --------------------------------------------------------------------- |
| **Explorer / requêter (GUI)**          | DBeaver                         | **TablePlus** (quotidien) + DBeaver gardé pour les tâches lourdes       | Natif macOS, rapide, UX propre                                        |
| **IDE SQL avec intelligence**          | —                               | **DataGrip** (si tu veux le top) ou l'éditeur SQL de **Cursor/VS Code** | Autocomplétion schéma-aware, refactor, navigation                     |
| **IA conversationnelle sur la DB**     | —                               | **Serveurs MCP** (Postgres + SQLite) dans Claude Code                   | Interroger/inspecter la DB en langage naturel, sans quitter l'éditeur |
| **Terminal rapide**                    | `psql` / `sqlite3`              | **`pgcli`** + **`harlequin`** (TUI)                                     | Autocomplétion, coloration, multi-dialecte                            |
| **Schéma & migrations**                | sqlx (Rust) + SQL brut (Python) | **Atlas** (déclaratif, multi-DB) à terme                                | Schéma comme code, diffs, dry-run, lint de migration                  |
| **Sortir le SQL des f-strings Python** | templating `.format()`          | requêtes `.sql` versionnées + **aiosql** / query builder                | Lintable, sûr, testable                                               |
| **DB jetable pour les tests**          | SQLite `:memory:`               | **Testcontainers** pour Postgres                                        | Tests d'intégration fidèles au prod                                   |
| **Branches de base / preview**         | —                               | **Neon** ou **Turso**                                                   | Une branche DB par PR, comme du git                                   |

Si tu ne devais retenir que **trois mouvements** : (1) installer les **serveurs MCP** dans
Claude Code, (2) passer à **TablePlus** pour le quotidien, (3) **sortir le SQL Postgres
des chaînes Python** vers des fichiers versionnés.

---

## 1. Le principe directeur : le schéma et les requêtes sont du _code_

La meilleure DX base de données ne vient pas d'un client GUI plus joli. Elle vient
d'un **changement de modèle mental** :

> Tout ce qui touche la structure des données et la logique de requête doit vivre
> dans le dépôt, être versionné, lintable, testable et reviewable — exactement
> comme le reste du code.

Un client GUI où tu cliques pour modifier une colonne est pratique pour _regarder_,
mais c'est un piège pour _changer_ : la modification n'est tracée nulle part, elle
n'est pas reproductible, et elle diverge silencieusement entre dev/staging/prod.

Les cinq couches d'une bonne DX, de la plus structurante à la plus accessoire :

```
┌─────────────────────────────────────────────────────────┐
│ 5. Observabilité    EXPLAIN ANALYZE, pg_stat_statements   │  ← comprendre
├─────────────────────────────────────────────────────────┤
│ 4. Augmentation IA  MCP, text-to-SQL, copilotes           │  ← accélérer
├─────────────────────────────────────────────────────────┤
│ 3. Dev local        Docker, Testcontainers, branches DB   │  ← itérer sans risque
├─────────────────────────────────────────────────────────┤
│ 2. Schéma comme code  migrations, Atlas, ORM schema-first │  ← la fondation
├─────────────────────────────────────────────────────────┤
│ 1. Accès / exploration  GUI, TUI, CLI                     │  ← la surface visible
└─────────────────────────────────────────────────────────┘
```

DBeaver ne couvre que la couche 1. Le saut de qualité vient des couches 2 à 5.

---

## 2. Couche 1 — Accès et exploration

### Clients GUI

| Outil                | Plateforme      | Forces                                                        | Limites                                  | IA                          |
| -------------------- | --------------- | ------------------------------------------------------------- | ---------------------------------------- | --------------------------- |
| **DBeaver**          | Java (multi-OS) | Universel, gratuit, très complet                              | UX Java lourde, lent au démarrage        | ❌ (plugin payant tiers)    |
| **TablePlus**        | Natif macOS/Win | Ultra rapide, élégant, multi-DB, onglets                      | Payant (licence perpétuelle raisonnable) | Partielle (assistant SQL)   |
| **DataGrip**         | JetBrains       | Meilleure intelligence SQL du marché, refactoring, navigation | Abonnement, lourd                        | ✅ **AI Assistant** intégré |
| **Beekeeper Studio** | Electron        | Open source, moderne, agréable                                | Moins de fonctions avancées              | ❌                          |
| **Postico 2**        | Natif macOS     | Très propre, focalisé Postgres                                | Postgres uniquement                      | ❌                          |
| **Outerbase**        | Web/cloud       | **IA-native** : « parle à ta base »                           | Cloud, modèle SaaS                       | ✅✅ conçu autour de l'IA   |

**Recommandation pour toi (macOS, multi-DB Postgres + SQLite) :**

- **TablePlus** comme client quotidien. Natif, instantané, gère Postgres et SQLite
  dans la même fenêtre — c'est le remplaçant direct de DBeaver pour 90 % de tes
  besoins, avec une UX sans commune mesure.
- **Garde DBeaver** pour les tâches occasionnelles très lourdes (import/export
  massif, diagrammes ER complets, drivers exotiques) où son exhaustivité paie.
- **DataGrip** si tu veux _le_ meilleur éditeur SQL et que tu vis déjà dans
  l'écosystème JetBrains : autocomplétion qui comprend ton schéma, renommage de
  colonne propagé, détection d'erreurs avant exécution.

### CLI / TUI — souvent sous-estimés

Pour l'inspection rapide et le scripting, le terminal bat le GUI en vitesse :

- **`pgcli`** — un `psql` avec autocomplétion intelligente, coloration syntaxique
  et historique. Pour SQLite : **`litecli`** (même famille).
- **`harlequin`** — un véritable IDE SQL **dans le terminal** (TUI), magnifique,
  multi-dialecte (Postgres, SQLite, DuckDB…). Idéal quand tu es déjà au clavier.
- **`usql`** — un client universel unique pour tous les dialectes (style `psql`
  mais multi-DB), pratique en monorepo polyglotte comme le tien.

```bash
# À ajouter à ton mise.toml pour standardiser l'outillage de l'équipe
pgcli      = "latest"   # ou via pipx / uv tool install
"pipx:harlequin" = "latest"
```

---

## 3. Couche 2 — Le schéma comme code (la vraie fondation)

C'est ici que se joue 80 % de la qualité de vie. Trois approches, par ordre de
maturité.

### a) Migrations versionnées (le minimum vital)

Des fichiers SQL numérotés, appliqués dans l'ordre, jamais modifiés après coup.
Tu fais **déjà ça** côté Rust avec `sqlx::migrate!()` — c'est exactement la bonne
pratique. Le manque est côté Python/Postgres analytics.

Outils selon le langage :

- **Rust** : `sqlx migrate` (tu l'utilises), ou Diesel.
- **Python** : **Alembic** (l'étalon), ou `dbmate` / `golang-migrate` (agnostiques
  du langage, pilotés par fichiers `.sql` bruts — pratique si tu ne veux pas d'ORM).
- **Agnostique** : `dbmate`, `migrate`, **Flyway** (entreprise), **Liquibase**.

### b) Schéma déclaratif avec diff automatique — **Atlas** ⭐

L'évolution moderne : tu décris **l'état désiré** du schéma, l'outil calcule le
`ALTER` nécessaire (comme Terraform pour ta base).

```hcl
# schema.hcl — l'état désiré, versionné
table "events" {
  schema = schema.public
  column "id"         { type = uuid, null = false }
  column "stream_id"  { type = uuid, null = false }
  column "payload"    { type = jsonb }
  primary_key { columns = [column.id] }
  index "idx_stream"  { columns = [column.stream_id] }
}
```

```bash
atlas schema diff --from "postgres://..." --to file://schema.hcl   # voir le plan
atlas migrate diff --to file://schema.hcl                          # générer la migration
atlas migrate lint                                                  # lint : détecte les changements destructifs/non-rétrocompatibles
```

**Pourquoi Atlas est idéal pour ton cas multi-DB :** il parle Postgres _et_
SQLite, fait du **lint de migration** (te prévient avant un `DROP COLUMN`
destructeur ou un lock de table en prod), et s'intègre en CI. C'est l'outil qui
manque le plus à ta pile actuelle.

### c) Schéma-first via ORM avec excellente DX (si tu ajoutes du TS)

Pour la partie TypeScript du monorepo :

- **Drizzle ORM** — schéma typé en TS, migrations générées, requêtes type-safe.
  La meilleure DX TS actuelle, proche du SQL.
- **Prisma** — schéma déclaratif lisible, client généré, studio visuel inclus.
  Très bonne DX, un peu plus « magique ».

### Le point sensible chez toi : sortir le SQL des chaînes Python

Ton `.sqlfluff` documente la douleur : _« the analytics queries mix Postgres with
two templating styles (`%(name)s` psycopg params and `{schema}` python `.format`) »_.

Le templating `{schema}.format()` est à la fois **infaisable à linter** et **un
risque d'injection** dès qu'une valeur non-contrôlée s'y glisse. Deux remèdes :

1. **Sortir les requêtes dans des fichiers `.sql` versionnés** et les charger avec
   **`aiosql`** (ou `sql-mason`). Le SQL devient lintable par sqlfluff/sqruff,
   testable, et reviewable proprement :

   ```sql
   -- queries.sql
   -- name: events_by_stream
   SELECT * FROM events WHERE stream_id = :stream_id;
   ```

   ```python
   import aiosql
   queries = aiosql.from_path("queries.sql", "psycopg")
   rows = queries.events_by_stream(conn, stream_id=sid)   # paramétré, sûr
   ```

2. Pour le `{schema}` dynamique (identifiant, pas une valeur), utiliser
   `psycopg.sql.Identifier()` plutôt que `.format()` — c'est la seule façon sûre
   d'injecter un nom de schéma/table.

   ```python
   from psycopg import sql
   q = sql.SQL("SELECT * FROM {}.events").format(sql.Identifier(schema_name))
   ```

Ce seul changement débloque ton `lint:sql` (aujourd'hui opt-in et fragile) pour le
brancher en CI comme le reste.

---

## 4. Couche 3 — Dev local et bases jetables

### En local

- **SQLite `:memory:`** — tu l'utilises déjà pour les tests Rust. Parfait pour la
  rapidité.
- **Docker / `docker-compose`** — un Postgres jetable identique au prod, en une
  commande. Le standard.
- **Testcontainers** — démarre un _vrai_ Postgres éphémère par suite de tests
  (`testcontainers` existe en Rust, Python et TS). **Recommandé** pour tes tests
  analytics : tester contre un vrai Postgres plutôt qu'un mock attrape les bugs de
  dialecte que SQLite masquerait.

### Bases « comme du git » (branching)

La fonctionnalité qui transforme le workflow d'équipe :

- **Neon** (Postgres serverless) — crée une **branche de base par PR** en quelques
  secondes (copy-on-write). Chaque preview a sa propre DB isolée, jetée à la
  fermeture de la PR.
- **Turso / libSQL** — du SQLite distribué à la périphérie, avec branching et
  réplication. **Directement pertinent** vu ton usage SQLite dans `flow` : tu peux
  passer d'un SQLite local à un SQLite répliqué multi-région sans changer ton code
  sqlx.
- **PlanetScale** — MySQL avec branches et diffs de schéma non-bloquants.

---

## 5. Couche 4 — L'augmentation IA (ta plus grande opportunité)

Tu es **déjà dans Claude Code** : c'est ici que ton avantage est le plus grand et
le plus immédiat. Trois niveaux.

### a) Serveurs MCP de base de données ⭐⭐ — à faire en premier

Le **Model Context Protocol** permet à Claude Code de parler **directement** à ta
base : inspecter le schéma, lancer des requêtes (idéalement en lecture seule),
expliquer un plan d'exécution — le tout en langage naturel, sans quitter l'éditeur.

C'est l'équivalent « IA-native » de DBeaver, mais conversationnel et intégré à ton
flux de travail.

```jsonc
// ~/.claude/mcp.json (ou config projet) — exemple Postgres en lecture seule
{
  "mcpServers": {
    "postgres-analytics": {
      "command": "npx",
      "args": [
        "-y",
        "@modelcontextprotocol/server-postgres",
        "postgresql://readonly@localhost/analytics",
      ],
    },
    "sqlite-flow": {
      "command": "npx",
      "args": ["-y", "mcp-server-sqlite", "--db-path", "./rose/flow/flow.db"],
    },
  },
}
```

Une fois branché, tu peux demander en langage naturel :

- _« Montre-moi le schéma de la table events et les index »_
- _« Quelles sont les 10 requêtes les plus lentes ? Explique le plan de la pire »_
- _« Écris une requête qui agrège les events par stream sur les 7 derniers jours »_
  — et Claude voit le vrai schéma, donc la requête est juste du premier coup.

> ⚠️ **Sécurité** : pointe le MCP sur un rôle **read-only** et sur une base de
> dev/staging, jamais sur du prod en écriture. Un agent qui peut `DROP` est un
> agent qui finira par `DROP`. Vois ça comme un principe : la
> simplicité (lecture seule) prime sur la puissance.

### b) Génération de requêtes (text-to-SQL)

- **Dans Claude Code / Cursor** : avec le MCP branché, tu obtiens du text-to-SQL
  _schéma-aware_ gratuitement — c'est supérieur aux outils dédiés car l'agent
  connaît tout ton contexte (migrations, code applicatif, conventions).
- **Outerbase** : client cloud entièrement bâti autour du chat-to-SQL, si tu veux
  une expérience GUI dédiée pour les non-développeurs.
- **Vanna.ai** : open source, text-to-SQL par RAG sur ton schéma — utile pour
  exposer une interface « questions en langage naturel » à des analystes.

### c) Copilotes dans l'éditeur

GitHub Copilot, Cursor et Claude Code complètent le SQL dans tes fichiers `.sql` et
tes migrations. C'est une raison de plus de **sortir le SQL des f-strings** : du
SQL dans un vrai fichier `.sql` est compris par les copilotes ; du SQL noyé dans
une chaîne Python ne l'est pas.

---

## 6. Couche 5 — Observabilité et performance

Comprendre _pourquoi_ une requête est lente :

- **`EXPLAIN (ANALYZE, BUFFERS)`** — la commande à maîtriser en priorité. Colle le
  résultat dans **explain.dalibo.com** (Postgres) pour une visualisation en arbre
  des coûts.
- **`pg_stat_statements`** — l'extension Postgres qui agrège les requêtes par coût
  cumulé. Le point de départ de toute optimisation : « qu'est-ce qui coûte le plus
  cher au total ? ».
- **pganalyze** / **PgHero** — tableaux de bord d'observabilité Postgres (index
  manquants, requêtes lentes, bloat).
- **Metabase** — pour l'exploration/BI côté analytics : interface visuelle de
  requêtage et dashboards, au-dessus de ton Postgres.

Astuce IA : avec le MCP branché, demande à Claude _« lance EXPLAIN ANALYZE sur
cette requête et propose un index »_ — il lit le plan et raisonne dessus.

---

## 7. Le chemin de migration depuis ton DBeaver actuel

Inutile de tout changer d'un coup. Ordre par ratio impact/effort :

1. **Semaine 1 — Brancher les serveurs MCP** (Postgres + SQLite, read-only) dans
   Claude Code. Effort : 30 min. Impact : énorme. C'est le mouvement IA-native qui
   te manque, et il est gratuit.
2. **Semaine 1 — Installer TablePlus** en parallèle de DBeaver. Tu sentiras la
   différence d'UX immédiatement ; garde DBeaver en secours.
3. **Semaine 2 — Adopter `pgcli` + `harlequin`** via `mise` pour l'inspection
   rapide au terminal.
4. **Sprint suivant — Sortir le SQL analytics des f-strings** vers `.sql` + aiosql,
   puis brancher `lint:sql` en CI (résout la dette documentée dans `.sqlfluff`).
5. **Sprint suivant — Évaluer Atlas** pour unifier la gestion de schéma Postgres
   _et_ SQLite avec lint de migration.
6. **Quand le besoin se présente — Testcontainers** pour les tests d'intégration
   Postgres, et **Neon/Turso** pour les branches de base par PR.

---

## 8. Tableau récapitulatif — la « pile de rêve »

| Couche               | Outil de rêve                 | Alternative gratuite/open source     |
| -------------------- | ----------------------------- | ------------------------------------ |
| Exploration GUI      | TablePlus                     | Beekeeper Studio, DBeaver            |
| IDE SQL              | DataGrip                      | éditeur SQL de VS Code/Cursor        |
| Terminal             | harlequin + pgcli             | psql / sqlite3                       |
| IA conversationnelle | Serveurs MCP dans Claude Code | (idem — gratuit)                     |
| Migrations           | Atlas                         | sqlx / Alembic / dbmate              |
| SQL Python sûr       | aiosql + fichiers `.sql`      | psycopg `sql.Identifier`             |
| Dev local            | Testcontainers + Neon/Turso   | Docker Compose + SQLite              |
| Observabilité        | pganalyze + Metabase          | EXPLAIN ANALYZE + pg_stat_statements |

---

## En une phrase

> La meilleure DX base de données, c'est : **le schéma et les requêtes vivent dans
> le dépôt** (Atlas + fichiers `.sql`), **l'IA parle directement à la base** (MCP
> dans Claude Code), **un client natif rapide** remplace le GUI lourd (TablePlus),
> et **chaque branche git a sa propre base jetable** (Neon/Turso). DBeaver n'était
> que la surface ; la valeur est dans les quatre couches en dessous.
