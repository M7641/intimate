use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Types partagés pour le pipeline de données
//
// Chaque struct correspond à une étape du pipeline décrit dans gensis.md :
//   receive → validate → enrich → inference → rules → write → notify
//
// Tous les types implémentent Serialize + Deserialize car Restate journalise
// (persiste) chaque résultat intermédiaire pour permettre le replay.
// ---------------------------------------------------------------------------

/// Fichier de données en entrée du pipeline.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataFile {
    pub file_id: String,
    pub file_name: String,
    pub schema_name: String,
    pub row_count: u64,
    /// Contenu brut (simulé — en production ce serait un chemin S3, etc.)
    pub raw_content: String,
}

/// Résultat de l'étape validate_schema.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationResult {
    pub valid: bool,
    pub errors: Vec<String>,
    pub validated_at: String,
}

/// Résultat de l'étape enrich_from_api.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnrichedData {
    pub file_id: String,
    pub original_rows: u64,
    pub enriched_fields: Vec<String>,
    pub api_version: String,
}

/// Résultat de l'étape run_inference.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InferenceResult {
    pub model_version: String,
    pub predictions: Vec<f64>,
    pub mean_confidence: f64,
}

/// Résultat de l'étape apply_rules.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RulesResult {
    pub passed_rules: u32,
    pub flagged_records: u32,
    pub applied_rules: Vec<String>,
}

/// Reçu de l'étape write_results.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WriteReceipt {
    pub destination: String,
    pub records_written: u64,
    pub write_id: String,
}

/// Rapport final agrégé par le workflow.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelineReport {
    pub file_id: String,
    pub validation: ValidationResult,
    pub enrichment: EnrichedData,
    pub inference: InferenceResult,
    pub rules: RulesResult,
    pub write_receipt: WriteReceipt,
    pub notification_sent: bool,
}

/// Statut du pipeline — stocké dans le K/V state du workflow via ctx.set().
/// Les shared handlers (get_status) peuvent le lire à tout moment.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PipelineStatus {
    Pending,
    Validating,
    Enriching,
    AwaitingApproval,
    RunningInference,
    ApplyingRules,
    WritingResults,
    Notifying,
    Completed,
    Failed(String),
}

impl std::fmt::Display for PipelineStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Pending => write!(f, "pending"),
            Self::Validating => write!(f, "validating"),
            Self::Enriching => write!(f, "enriching"),
            Self::AwaitingApproval => write!(f, "awaiting_approval"),
            Self::RunningInference => write!(f, "running_inference"),
            Self::ApplyingRules => write!(f, "applying_rules"),
            Self::WritingResults => write!(f, "writing_results"),
            Self::Notifying => write!(f, "notifying"),
            Self::Completed => write!(f, "completed"),
            Self::Failed(reason) => write!(f, "failed: {reason}"),
        }
    }
}

/// Décision humaine pour la gate d'approbation (human-in-the-loop).
///
/// gensis.md : « Some workflows need human input mid-stream — approval of a
/// pricing change, validation of a data anomaly. »
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApprovalDecision {
    pub approved: bool,
    pub reviewer: String,
    pub comment: String,
}

/// État suivi par le Virtual Object PipelineTracker.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TrackerState {
    pub pipeline_id: String,
    pub status: String,
    pub started_at: String,
    pub completed_at: Option<String>,
    pub steps_completed: Vec<String>,
    pub error: Option<String>,
}
