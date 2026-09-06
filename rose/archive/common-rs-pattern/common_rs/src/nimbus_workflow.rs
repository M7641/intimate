//! Client for the Nimbus platform workflows API — port of `common.backend.nimbus_workflow`.
//!
//! Several apps (annual, daily, weekly) need to: resolve a workflow id from a
//! target-keyed name dict; read the latest execution record; trigger a fresh execution.
//! This module collapses those three operations into one set of helpers.

use std::collections::HashMap;
use std::env;
use std::time::Duration;

use reqwest::Client;
use serde::Deserialize;
use thiserror::Error;

use crate::env::Target;

const BASE_URL: &str = "https://service.nimbus.example";

#[derive(Debug, Error)]
pub enum WorkflowError {
    #[error("API_KEY env var is not configured")]
    MissingApiKey,

    #[error("no workflow name configured for target {0}")]
    UnknownTarget(String),

    #[error("workflow {name:?} not found in Nimbus workflows list")]
    NotFound { name: String },

    #[error("Nimbus API request failed: {0}")]
    Request(#[from] reqwest::Error),
}

#[derive(Debug, Clone)]
pub struct NimbusWorkflowClient {
    http: Client,
    api_key: String,
}

impl NimbusWorkflowClient {
    pub fn from_env() -> Result<Self, WorkflowError> {
        let api_key = env::var("API_KEY").map_err(|_| WorkflowError::MissingApiKey)?;
        let http = Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .expect("reqwest client builder");
        Ok(Self { http, api_key })
    }

    async fn request_json<T: for<'de> Deserialize<'de>>(
        &self,
        method: reqwest::Method,
        endpoint: &str,
        body: Option<&serde_json::Value>,
    ) -> Result<T, WorkflowError> {
        let url = format!("{BASE_URL}{endpoint}");
        let mut req = self
            .http
            .request(method, &url)
            .header("Authorization", &self.api_key);
        if let Some(b) = body {
            req = req.json(b);
        }
        let resp = req.send().await?.error_for_status()?;
        Ok(resp.json::<T>().await?)
    }

    pub async fn get_workflow_id(
        &self,
        workflow_names: &HashMap<Target, String>,
        target: Target,
    ) -> Result<String, WorkflowError> {
        let want = workflow_names
            .get(&target)
            .ok_or_else(|| WorkflowError::UnknownTarget(target.as_str().to_string()))?;

        #[derive(Deserialize)]
        struct ListResp {
            workflows: Vec<Workflow>,
        }
        #[derive(Deserialize)]
        struct Workflow {
            id: String,
            name: String,
        }

        let resp: ListResp = self
            .request_json(reqwest::Method::GET, "/workflows/api/v1/workflows", None)
            .await?;

        resp.workflows
            .into_iter()
            .find(|w| &w.name == want)
            .map(|w| w.id)
            .ok_or_else(|| WorkflowError::NotFound { name: want.clone() })
    }

    pub async fn get_latest_execution(
        &self,
        workflow_names: &HashMap<Target, String>,
        target: Target,
    ) -> Result<Option<serde_json::Value>, WorkflowError> {
        let id = self.get_workflow_id(workflow_names, target).await?;

        #[derive(Deserialize)]
        struct ExecResp {
            executions: Vec<serde_json::Value>,
        }

        let resp: ExecResp = self
            .request_json(
                reqwest::Method::GET,
                &format!("/workflows/api/v1/workflows/executions/{id}"),
                None,
            )
            .await?;

        Ok(resp.executions.into_iter().max_by_key(|e| {
            e.get("executedAt")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string()
        }))
    }

    pub async fn execute_workflow(
        &self,
        workflow_names: &HashMap<Target, String>,
        target: Target,
    ) -> Result<serde_json::Value, WorkflowError> {
        let id = self.get_workflow_id(workflow_names, target).await?;
        self.request_json(
            reqwest::Method::POST,
            &format!("/workflows/api/v1/workflows/{id}/execute"),
            None,
        )
        .await
    }
}
