//! Client de lecture pour l'API workflows de Nimbus.

use serde_json::Value;

use crate::client::{Client, array_field};
use crate::error::Result;

const BASE_URL: &str = "https://service.nimbus.example/workflows/api/v1";

/// Vue « workflows » au-dessus du [`Client`] partagé.
pub struct Workflows<'a> {
    client: &'a Client,
}

impl<'a> Workflows<'a> {
    #[must_use]
    pub fn new(client: &'a Client) -> Self {
        Self { client }
    }

    /// Liste les workflows déployés.
    ///
    /// # Errors
    /// Propage toute erreur HTTP ou API.
    pub fn list(&self) -> Result<Vec<Value>> {
        let body = self
            .client
            .get_json(&format!("{BASE_URL}/workflows/"), &[])?;
        Ok(array_field(&body, "workflows"))
    }

    /// Décrit un workflow par son identifiant.
    ///
    /// # Errors
    /// Propage toute erreur HTTP ou API.
    pub fn describe(&self, workflow_id: &str) -> Result<Value> {
        self.client
            .get_json(&format!("{BASE_URL}/workflows/{workflow_id}"), &[])
    }
}
