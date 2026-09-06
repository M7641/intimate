//! Client HTTP partagé par tous les clients de domaine.
//!
//! Le code Python relisait `os.getenv("API_KEY")` à *chaque* appel et recréait
//! l'en-tête `Authorization` partout. Ici on construit le client une fois
//! (`Client::from_env`) ; il porte le `reqwest::blocking::Client` réutilisable et
//! la clé d'API, et expose les helpers verbe-par-verbe que les modules de domaine
//! réutilisent.

use std::time::Duration;

use reqwest::blocking::multipart::Form;
use serde_json::Value;

use crate::error::{Error, Result};

/// Nom de la variable d'environnement portant le jeton Nimbus.
const API_KEY_ENV: &str = "API_KEY";

/// Client HTTP authentifié auprès de la plateforme Nimbus.
pub struct Client {
    http: reqwest::blocking::Client,
    api_key: String,
}

impl Client {
    /// Construit un client en lisant la clé depuis `API_KEY`.
    ///
    /// # Errors
    /// Renvoie [`Error::MissingApiKey`] si la variable n'est pas définie.
    pub fn from_env() -> Result<Self> {
        let api_key = std::env::var(API_KEY_ENV).map_err(|_| Error::MissingApiKey)?;
        Self::with_key(api_key)
    }

    /// Construit un client avec une clé explicite (utile pour les tests).
    ///
    /// # Errors
    /// Renvoie [`Error::Http`] si le client `reqwest` ne peut être initialisé.
    pub fn with_key(api_key: String) -> Result<Self> {
        // Timeout global de 30 s : la valeur la plus large utilisée côté Python.
        let http = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(30))
            .build()?;
        Ok(Self { http, api_key })
    }

    /// Envoie une requête (en injectant l'en-tête `Authorization`) et renvoie le
    /// couple (statut, corps brut). La vérification du statut est laissée à
    /// l'appelant, car les endpoints Nimbus attendent des codes variés (200/201/202).
    fn send(&self, request: reqwest::blocking::RequestBuilder) -> Result<(u16, String)> {
        let response = request.header("Authorization", &self.api_key).send()?;
        let status = response.status().as_u16();
        let body = response.text().unwrap_or_default();
        Ok((status, body))
    }

    /// GET `url` (query params optionnels) et renvoie le corps JSON parsé.
    ///
    /// # Errors
    /// [`Error::Http`] en cas d'échec réseau, [`Error::Api`] si le statut n'est
    /// pas un succès, [`Error::Json`] si le corps n'est pas du JSON valide.
    pub fn get_json(&self, url: &str, query: &[(&str, &str)]) -> Result<Value> {
        let (status, body) = self.send(self.http.get(url).query(query))?;
        if !(200..300).contains(&status) {
            return Err(Error::Api { status, body });
        }
        Ok(serde_json::from_str(&body)?)
    }

    /// POST JSON et renvoie (statut, corps parsé). Ne vérifie pas le statut.
    ///
    /// # Errors
    /// [`Error::Http`] en cas d'échec réseau.
    pub fn post_json(&self, url: &str, body: &Value) -> Result<(u16, Value)> {
        let (status, text) = self.send(self.http.post(url).json(body))?;
        Ok((status, parse_body(text)))
    }

    /// PATCH JSON et renvoie (statut, corps parsé). Ne vérifie pas le statut.
    ///
    /// # Errors
    /// [`Error::Http`] en cas d'échec réseau.
    pub fn patch_json(&self, url: &str, body: &Value) -> Result<(u16, Value)> {
        let (status, text) = self.send(self.http.patch(url).json(body))?;
        Ok((status, parse_body(text)))
    }

    /// DELETE et renvoie le corps parsé (le code Python n'en vérifiait pas le statut).
    ///
    /// # Errors
    /// [`Error::Http`] en cas d'échec réseau.
    pub fn delete(&self, url: &str) -> Result<Value> {
        let (_, text) = self.send(self.http.delete(url))?;
        Ok(parse_body(text))
    }

    /// POST multipart/form-data (upload d'artefact) ; renvoie (statut, corps parsé).
    ///
    /// # Errors
    /// [`Error::Http`] en cas d'échec réseau.
    pub fn post_multipart(&self, url: &str, form: Form) -> Result<(u16, Value)> {
        let (status, text) = self.send(self.http.post(url).multipart(form))?;
        Ok((status, parse_body(text)))
    }
}

/// Parse un corps en JSON ; à défaut, l'enveloppe en `Value::String` pour que les
/// messages d'erreur conservent le texte brut renvoyé par l'API.
fn parse_body(text: String) -> Value {
    serde_json::from_str(&text).unwrap_or(Value::String(text))
}

/// Extrait le tableau situé sous `key`, ou un vecteur vide s'il est absent.
///
/// Reproduit le `response.json().get(key, [])` omniprésent côté Python.
pub(crate) fn array_field(value: &Value, key: &str) -> Vec<Value> {
    value
        .get(key)
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
}

/// Lit le champ `id` d'un élément JSON sous forme de chaîne, qu'il soit encodé
/// comme nombre (images/workflows) ou comme chaîne (services).
pub(crate) fn json_id(item: &Value) -> Option<String> {
    item.get("id").map(value_to_id)
}

/// Représente une valeur JSON scalaire (nombre ou chaîne) comme une chaîne, sans
/// les guillemets que `Value::to_string` ajouterait à une `String`.
pub(crate) fn value_to_id(value: &Value) -> String {
    match value {
        Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}
