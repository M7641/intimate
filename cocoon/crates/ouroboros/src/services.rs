//! Client pour l'API webapps/services de Nimbus (lecture + déploiement).

use std::time::{Duration, Instant};

use serde_json::{Value, json};

use crate::client::{Client, array_field, json_id, value_to_id};
use crate::error::{Error, Result};
use crate::images::{Images, deploy_image};
use crate::types::{Artifact, BuildArg};

const BASE_URL: &str = "https://service.nimbus.example/webapps/api/v1";

/// Délai et cadence du polling de build d'image.
const BUILD_TIMEOUT: Duration = Duration::from_secs(600);
const BUILD_POLL_INTERVAL: Duration = Duration::from_secs(15);

/// Vue « services » au-dessus du [`Client`] partagé.
pub struct Services<'a> {
    client: &'a Client,
}

impl<'a> Services<'a> {
    #[must_use]
    pub fn new(client: &'a Client) -> Self {
        Self { client }
    }

    /// Liste les services déployés.
    ///
    /// # Errors
    /// Propage toute erreur HTTP ou API.
    pub fn list(&self) -> Result<Vec<Value>> {
        let body = self.client.get_json(&format!("{BASE_URL}/webapps/"), &[])?;
        Ok(array_field(&body, "webapps"))
    }

    /// Décrit un service par son identifiant.
    ///
    /// # Errors
    /// Propage toute erreur HTTP ou API.
    pub fn describe(&self, service_id: &str) -> Result<Value> {
        self.client
            .get_json(&format!("{BASE_URL}/webapps/{service_id}"), &[])
    }

    /// Crée un service (statut attendu 202).
    ///
    /// # Errors
    /// [`Error::Api`] si le statut n'est pas 202, sinon erreur HTTP.
    pub fn create(&self, body: &Value) -> Result<Value> {
        let (status, resp) = self
            .client
            .post_json(&format!("{BASE_URL}/webapps"), body)?;
        expect_202(status, resp)
    }

    /// Met à jour un service existant (statut attendu 202).
    ///
    /// # Errors
    /// [`Error::Api`] si le statut n'est pas 202, sinon erreur HTTP.
    pub fn update(&self, body: &Value, service_id: &str) -> Result<Value> {
        let (status, resp) = self
            .client
            .patch_json(&format!("{BASE_URL}/webapps/{service_id}"), body)?;
        expect_202(status, resp)
    }

    /// Crée le service s'il n'existe pas (par nom), sinon le met à jour.
    ///
    /// # Errors
    /// Erreurs HTTP/API ; [`Error::MissingField`] si `body` n'a pas de `name`.
    pub fn create_or_update(&self, body: &Value) -> Result<Value> {
        let name = body
            .get("name")
            .and_then(Value::as_str)
            .ok_or_else(|| Error::MissingField("name".to_string()))?;

        let existing = self
            .list()?
            .into_iter()
            .find(|s| s.get("name").and_then(Value::as_str) == Some(name));

        match existing.as_ref().and_then(json_id) {
            Some(service_id) => self.update(body, &service_id),
            None => self.create(body),
        }
    }
}

fn expect_202(status: u16, body: Value) -> Result<Value> {
    if status == 202 {
        Ok(body)
    } else {
        Err(Error::Api {
            status,
            body: body.to_string(),
        })
    }
}

/// Paramètres de [`deploy_service`]. Les défauts reproduisent la signature Python
/// (`service_type="webapp"`, `target="test"`, `version="v1"`, …).
#[derive(Debug, Clone)]
pub struct DeployService {
    pub service_name: String,
    /// `"webapp"` ou `"api"`.
    pub service_type: String,
    pub target: String,
    pub build_args: Vec<BuildArg>,
    pub build_secrets: Vec<String>,
    pub dockerfile_path: String,
    pub version: String,
    pub artifact: Option<Artifact>,
    pub scale_to_zero: bool,
    pub num_instances: u32,
    /// Surcharge des ressources ; sinon un type d'instance par défaut est choisi
    /// selon `service_type`.
    pub resources: Option<Value>,
}

impl Default for DeployService {
    fn default() -> Self {
        Self {
            service_name: String::new(),
            service_type: "webapp".to_string(),
            target: "test".to_string(),
            build_args: Vec::new(),
            build_secrets: Vec::new(),
            dockerfile_path: "Dockerfile.webapp".to_string(),
            version: "v1".to_string(),
            artifact: None,
            scale_to_zero: false,
            num_instances: 1,
            resources: None,
        }
    }
}

/// Déploie un service de bout en bout : build de l'image (upload artefact),
/// attente de la fin du build, puis création/MAJ de la webapp. Renvoie l'id du
/// service.
///
/// # Errors
/// [`Error::BuildFailed`]/[`Error::BuildTimeout`] selon le build, et toute erreur
/// d'upload, HTTP ou API. [`Error::MissingField`] si aucune ressource ne peut être
/// déterminée pour un `service_type` inconnu.
pub fn deploy_service(client: &Client, opts: &DeployService) -> Result<String> {
    // data_view → "data-view-v1-cocoon"
    let service_name = format!(
        "{}-{}-{}",
        opts.service_name.replace('_', "-").to_lowercase(),
        opts.version,
        opts.target
    );
    tracing::info!("Deploying the service called: {service_name}");

    let artifact = opts.artifact.clone().unwrap_or_default();
    let build_args: Vec<Value> = opts
        .build_args
        .iter()
        .map(|a| json!({ "name": a.name, "value": a.value }))
        .collect();

    let image_body = json!({
        "name": format!("{service_name}-image"),
        "type": opts.service_type,
        "buildDetails": {
            "source": "upload",
            "useCache": true,
            "context": ".",
            "dockerfilePath": opts.dockerfile_path,
            "buildArguments": build_args,
            "secrets": opts.build_secrets,
        }
    });

    let (image_id, version_id) = deploy_image(client, &image_body, &artifact)?;

    wait_for_build(client, &value_to_id(&image_id))?;
    tracing::info!(
        "Image deployed with ID: {} and version ID: {}",
        value_to_id(&image_id),
        value_to_id(&version_id)
    );

    let service_id = build_service(client, &service_name, &image_id, &version_id, opts)?;
    tracing::info!("Web app created or updated successfully: {service_id}");
    Ok(service_id)
}

/// Boucle d'attente de la fin du build de l'image (success / failed / timeout).
fn wait_for_build(client: &Client, image_id: &str) -> Result<()> {
    let images = Images::new(client);
    let start = Instant::now();
    loop {
        if start.elapsed() > BUILD_TIMEOUT {
            return Err(Error::BuildTimeout);
        }
        let details = images.describe(image_id)?;
        let status = details
            .get("latestVersion")
            .and_then(|v| v.get("lastBuildStatus"))
            .and_then(Value::as_str);
        match status {
            Some("success") => {
                tracing::info!("Image version is now available");
                return Ok(());
            }
            Some("failed") => return Err(Error::BuildFailed),
            _ => std::thread::sleep(BUILD_POLL_INTERVAL),
        }
    }
}

/// Crée ou met à jour la webapp pointant sur l'image construite. Renvoie son id.
fn build_service(
    client: &Client,
    service_name: &str,
    image_id: &Value,
    version_id: &Value,
    opts: &DeployService,
) -> Result<String> {
    // Ressources par défaut selon le type, sauf surcharge explicite.
    let resources = match &opts.resources {
        Some(r) => r.clone(),
        None => match opts.service_type.as_str() {
            "webapp" => json!({ "instanceTypeId": 47 }),
            "api" => json!({ "instanceTypeId": 23 }),
            _ => return Err(Error::MissingField("resources".to_string())),
        },
    };

    // L'API attend "web-app" alors que l'entrée utilise "webapp".
    let service_type = if opts.service_type == "webapp" {
        "web-app"
    } else {
        &opts.service_type
    };

    let body = json!({
        "name": service_name,
        "title": format!("{} Service", capitalize(&service_name.replace('_', "-"))),
        "serviceType": service_type,
        "imageDetails": { "imageId": image_id, "versionId": version_id },
        "description": capitalize(&service_type.replace('-', " ")),
        "resources": resources,
        "minInstances": opts.num_instances,
        "scaleToZero": opts.scale_to_zero,
        "tags": [{ "name": opts.target }],
    });

    let response = Services::new(client).create_or_update(&body)?;
    json_id(&response).ok_or_else(|| Error::MissingField("id".to_string()))
}

/// Équivalent de `str.capitalize()` : première lettre en majuscule, reste en minuscules.
fn capitalize(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + &chars.as_str().to_lowercase(),
        None => String::new(),
    }
}
