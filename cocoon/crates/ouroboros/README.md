# ouroboros

Client + CLI **en lecture seule** pour la plateforme [Nimbus](https://nimbus.example),
porté en Rust depuis `blank/ouroboros` (Python/typer).

Comme le package Python d'origine, le crate a deux faces :

- **Librairie** — un `Client` HTTP authentifié et quatre vues de domaine
  (`Images`, `Services`, `Workflows`, `Tenant`), consommables par d'autres crates
  de `cocoon`.
- **Binaire `ouroboros`** — un CLI reproduisant les 7 commandes de lecture.

## CLI

`API_KEY` (jeton Nimbus) doit être défini dans l'environnement.

```bash
export API_KEY="..."

cargo run -p ouroboros -- list-services [--detailed] [--save]
cargo run -p ouroboros -- describe-service <service_id> [--save]
cargo run -p ouroboros -- list-workflows [--detailed] [--save]
cargo run -p ouroboros -- describe-workflow <workflow_id> [--save]
cargo run -p ouroboros -- list-images [--detailed] [--save]
cargo run -p ouroboros -- describe-image <image_id> [--save]
cargo run -p ouroboros -- instance-options <entity_type>
```

| Option         | Effet                                                                   |
| -------------- | ----------------------------------------------------------------------- |
| `-d, --detailed` | Pour les `list-*` : un appel `describe` par élément (sortie enrichie). |
| `-s, --save`     | Écrit le JSON dans un fichier (`services.json`, `service_<id>.json`…). |

`instance-options` affiche les types d'instances groupés par classe sous forme de
table (équivalent du `rich.Table` Python), avec un coût journalier estimé.

Types d'entité acceptés : `api-deployment`, `dataBridge-dataCatalog`,
`dataBridge-dataLake`, `dataBridge-dataWareHouse`, `feed`, `webapp`, `workflow`,
`workspace`, `pat`, `sat`.

## Librairie

```rust
use ouroboros::{Client, Images, Tenant};

let client = Client::from_env()?;            // lit API_KEY

let images = Images::new(&client).list()?;   // Vec<serde_json::Value>
let detail = Images::new(&client).describe("123")?;

Tenant::new(&client).print_instance_options("workflow")?;
```

Les réponses de l'API sont renvoyées en `serde_json::Value` : on ne modélise pas
tout le schéma Nimbus, le CLI ne fait qu'afficher/sauvegarder.

## APIs Nimbus couvertes

| Module       | Base URL                                              |
| ------------ | ----------------------------------------------------- |
| `images`     | `service.nimbus.example/image-management/api/v2`             |
| `services`   | `service.nimbus.example/webapps/api/v1`                      |
| `workflows`  | `service.nimbus.example/workflows/api/v1`                    |
| `tenant`     | `service.nimbus.example/quota/api/v1`                        |

## Déploiement (librairie)

La face écriture est portée et sert au déploiement des apps de `cocoon` en Rust
pur (`data_view`, `warehouse`) :

```rust
use ouroboros::{Client, DeployService, BuildArg, Artifact, deploy_service};

let client = Client::from_env()?;
let opts = DeployService {
    service_name: "data_view".into(),
    target: "cocoon".into(),
    build_args: vec![BuildArg::new("TARGET_ENV", "cocoon")],
    build_secrets: vec!["GITHUB_TOKEN".into()],
    dockerfile_path: "Dockerfile".into(),
    artifact: Some(Artifact { path: ".".into(), ignore_files: vec![".dockerignore".into()] }),
    ..Default::default()
};
deploy_service(&client, &opts)?; // zip + upload + poll build + create/MAJ webapp
```

`deploy_service` enchaîne : `deploy_image` (build de l'artefact zip avec filtrage
`.dockerignore` via le crate `ignore`, upload multipart, élagage à 3 versions),
attente de la fin du build (≤ 600 s), puis `Services::create_or_update`.

## Hors périmètre (à porter ensuite)

`deploy_workflows` (déploiement de workflows) n'est pas encore porté — la chaîne
images est en place, il ne resterait que la partie spécifique aux workflows.
