//! Client pour l'API image-management de Nimbus (lecture + écriture/déploiement).

use std::path::Path;

use reqwest::blocking::multipart::{Form, Part};
use serde_json::Value;

use crate::artifact::build_zip;
use crate::client::{Client, array_field, json_id, value_to_id};
use crate::error::{Error, Result};
use crate::types::Artifact;

const BASE_URL: &str = "https://service.nimbus.example/image-management/api/v2";

/// Types d'image acceptés par l'API.
const IMAGE_TYPES: &[&str] = &[
    "api",
    "webapp",
    "workflow",
    "workspace-python",
    "workspace-r",
];

/// Vue « images » au-dessus du [`Client`] partagé.
///
/// Emprunte le client (`&'a Client`) plutôt que de le posséder, pour que les
/// quatre vues de domaine se partagent une seule connexion.
pub struct Images<'a> {
    client: &'a Client,
}

impl<'a> Images<'a> {
    #[must_use]
    pub fn new(client: &'a Client) -> Self {
        Self { client }
    }

    // ── Lecture ──────────────────────────────────────────────────────────────

    /// Liste **toutes** les images en parcourant chaque page (75 par page).
    ///
    /// Nimbus pagine `GET /images` ; ne lire que la première page ferait « dispa-
    /// raître » toute image au-delà de la 75ᵉ, ce qui pousserait
    /// [`create_or_update_image_version`](Self::create_or_update_image_version) à
    /// recréer une image existante (→ `400 name is already in use`). On boucle
    /// donc jusqu'à `pageCount`.
    ///
    /// # Errors
    /// Propage toute erreur HTTP ou API.
    pub fn list(&self) -> Result<Vec<Value>> {
        let mut images = Vec::new();
        let mut page = 1u64;
        loop {
            let page_str = page.to_string();
            let body = self.client.get_json(
                &format!("{BASE_URL}/images"),
                &[("pageSize", "75"), ("pageNumber", &page_str)],
            )?;

            let batch = array_field(&body, "images");
            let batch_empty = batch.is_empty();
            images.extend(batch);

            // `pageCount` est la borne autoritative ; en son absence on s'arrête
            // après la page courante (comportement d'origine). Une page vide
            // coupe aussi la boucle, par sécurité contre une boucle infinie.
            let page_count = body
                .get("pageCount")
                .and_then(Value::as_u64)
                .unwrap_or(page);
            if page >= page_count || batch_empty {
                break;
            }
            page += 1;
        }
        Ok(images)
    }

    /// Décrit une image par son identifiant.
    ///
    /// # Errors
    /// Propage toute erreur HTTP ou API.
    pub fn describe(&self, image_id: &str) -> Result<Value> {
        self.client
            .get_json(&format!("{BASE_URL}/images/{image_id}"), &[])
    }

    /// Liste les versions d'une image.
    ///
    /// # Errors
    /// Propage toute erreur HTTP ou API.
    pub fn list_versions(&self, image_id: &str) -> Result<Vec<Value>> {
        let body = self
            .client
            .get_json(&format!("{BASE_URL}/images/{image_id}/versions"), &[])?;
        Ok(array_field(&body, "versions"))
    }

    // ── Écriture ─────────────────────────────────────────────────────────────

    /// Crée une nouvelle image en uploadant l'artefact zippé (statut attendu 201).
    ///
    /// # Errors
    /// [`Error::MissingField`]/[`Error::InvalidImageType`] si `type` est absent ou
    /// invalide, sinon toute erreur d'I/O, de zip, HTTP ou API.
    pub fn create_image(&self, body: &Value, artifact: &Artifact) -> Result<Value> {
        self.validate_image_type(body)?;
        let form = self.build_upload_form(body, artifact)?;
        let (status, resp) = self
            .client
            .post_multipart(&format!("{BASE_URL}/images"), form)?;
        expect(status, 201, resp)
    }

    /// Crée une nouvelle version d'une image existante (statut attendu 201).
    ///
    /// # Errors
    /// Toute erreur d'I/O, de zip, HTTP ou API.
    pub fn create_version(
        &self,
        image_id: &str,
        body: &Value,
        artifact: &Artifact,
    ) -> Result<Value> {
        let form = self.build_upload_form(body, artifact)?;
        let (status, resp) = self
            .client
            .post_multipart(&format!("{BASE_URL}/images/{image_id}/versions"), form)?;
        expect(status, 201, resp)
    }

    /// Crée l'image si elle n'existe pas (par nom), sinon ajoute une version.
    ///
    /// # Errors
    /// [`Error::MissingField`] si `name` est absent, sinon erreurs d'upload/API.
    pub fn create_or_update_image_version(
        &self,
        body: &Value,
        artifact: &Artifact,
    ) -> Result<Value> {
        let name = body
            .get("name")
            .and_then(Value::as_str)
            .ok_or_else(|| Error::MissingField("name".to_string()))?;

        let existing = self
            .list()?
            .into_iter()
            .find(|img| img.get("name").and_then(Value::as_str) == Some(name));

        match existing.as_ref().and_then(json_id) {
            Some(image_id) => self.create_version(&image_id, body, artifact),
            None => self.create_image(body, artifact),
        }
    }

    /// Supprime une version d'image.
    ///
    /// # Errors
    /// [`Error::Http`] en cas d'échec réseau.
    pub fn delete_version(&self, image_id: &str, version_id: &str) -> Result<Value> {
        self.client.delete(&format!(
            "{BASE_URL}/images/{image_id}/versions/{version_id}"
        ))
    }

    /// Élague les anciennes versions, n'en conservant que les `keep_last_n` plus
    /// récentes (tri par `createdAt`).
    ///
    /// # Errors
    /// Toute erreur HTTP ou API.
    pub fn delete_all_but_x(&self, image_id: &str, keep_last_n: usize) -> Result<()> {
        let versions = self.list_versions(image_id)?;
        if versions.len() <= keep_last_n {
            return Ok(());
        }

        let mut created: Vec<&str> = versions
            .iter()
            .filter_map(|v| v.get("createdAt").and_then(Value::as_str))
            .collect();
        created.sort_unstable();
        // Les timestamps des `keep_last_n` plus récents sont conservés.
        let keep: std::collections::HashSet<&str> =
            created.iter().rev().take(keep_last_n).copied().collect();

        for version in &versions {
            let created_at = version.get("createdAt").and_then(Value::as_str);
            if created_at.is_some_and(|c| !keep.contains(c))
                && let Some(version_id) = json_id(version)
            {
                self.delete_version(image_id, &version_id)?;
            }
        }
        Ok(())
    }

    // ── Internes ─────────────────────────────────────────────────────────────

    fn validate_image_type(&self, body: &Value) -> Result<()> {
        let image_type = body
            .get("type")
            .and_then(Value::as_str)
            .ok_or_else(|| Error::MissingField("type".to_string()))?;
        if !IMAGE_TYPES.contains(&image_type) {
            return Err(Error::InvalidImageType {
                value: image_type.to_string(),
                allowed: IMAGE_TYPES.join(", "),
            });
        }
        Ok(())
    }

    /// Zippe l'artefact (fichier temporaire auto-supprimé) et construit le
    /// formulaire multipart : champs texte du `body` + partie binaire `artifact`.
    fn build_upload_form(&self, body: &Value, artifact: &Artifact) -> Result<Form> {
        let zip = tempfile::Builder::new().suffix(".zip").tempfile()?;
        build_zip(&artifact.path, zip.path(), &artifact.ignore_files)?;
        let form = multipart_fields(body)
            .into_iter()
            .fold(Form::new(), |form, (key, value)| form.text(key, value));
        Ok(form.part("artifact", zip_part(zip.path())?))
    }
}

/// Convertit un objet JSON en champs multipart : les chaînes restent telles
/// quelles, le reste est sérialisé en JSON (équivalent du `json.dumps` Python).
fn multipart_fields(body: &Value) -> Vec<(String, String)> {
    body.as_object()
        .map(|map| {
            map.iter()
                .map(|(key, value)| (key.clone(), value_to_id(value)))
                .collect()
        })
        .unwrap_or_default()
}

/// Construit la partie multipart `artifact.zip` à partir d'un fichier sur disque.
fn zip_part(zip_path: &Path) -> Result<Part> {
    let bytes = std::fs::read(zip_path)?;
    Ok(Part::bytes(bytes)
        .file_name("artifact.zip")
        .mime_str("application/zip")?)
}

/// Vérifie le statut attendu ; renvoie le corps en cas de succès, [`Error::Api`] sinon.
fn expect(status: u16, expected: u16, body: Value) -> Result<Value> {
    if status == expected {
        Ok(body)
    } else {
        Err(Error::Api {
            status,
            body: body.to_string(),
        })
    }
}

/// Déploie une image : crée/MAJ la version depuis l'artefact, élague les anciennes
/// versions (en garde 3), et renvoie `(imageId, versionId)` en préservant leur type
/// JSON d'origine (souvent numérique).
///
/// # Errors
/// [`Error::MissingField`] si la réponse n'a pas `imageId`/`versionId`, sinon
/// toute erreur d'upload/API.
pub fn deploy_image(client: &Client, body: &Value, artifact: &Artifact) -> Result<(Value, Value)> {
    if let Ok(tenant) = std::env::var("TENANT") {
        tracing::info!("Deploying image for tenant: {tenant}");
    }

    let images = Images::new(client);
    let response = images.create_or_update_image_version(body, artifact)?;

    let image_id = response
        .get("imageId")
        .cloned()
        .ok_or_else(|| Error::MissingField("imageId".to_string()))?;
    let version_id = response
        .get("versionId")
        .cloned()
        .ok_or_else(|| Error::MissingField("versionId".to_string()))?;

    images.delete_all_but_x(&value_to_id(&image_id), 3)?;

    Ok((image_id, version_id))
}
