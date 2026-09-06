// ---------------------------------------------------------------------------
// DataPipeline — Workflow Restate (exécution durable)
//
// Ce fichier est le coeur de l'exemple. Il implémente le pipeline de données
// décrit dans gensis.md :
//
//   receive → validate → enrich → [approve] → inference → rules → write → notify
//
// Concepts démontrés (chacun annoté dans le code) :
//   1. Code séquentiel rendu durable — « you write sequential code, the
//      runtime makes it durable » (gensis.md)
//   2. ctx.run() journaling — chaque side-effect est persisté et rejoué
//   3. Retry policies — backoff exponentiel par étape
//   4. Promises durables — human-in-the-loop (gate d'approbation)
//   5. Saga pattern — compensation sur rejet
//   6. Shared handlers — query du statut en cours d'exécution
//   7. Déterminisme — ctx.rand_uuid() au lieu de rand::random()
//   8. Communication inter-services — appel au PipelineTracker
// ---------------------------------------------------------------------------

use std::time::Duration;

use restate_sdk::prelude::*;

use crate::tracker::PipelineTrackerClient;
use crate::types::*;

// ===========================================================================
// Définition du trait — décrit l'interface publique du workflow
// ===========================================================================

#[restate_sdk::workflow]
pub trait DataPipeline {
    /// Handler principal — s'exécute **exactement une fois** par workflow ID.
    ///
    /// gensis.md : « Workflows are functions that describe the overall process.
    /// They look like normal sequential code — call this, then call that. »
    async fn run(input: Json<DataFile>) -> Result<Json<PipelineReport>, HandlerError>;

    /// Query du statut courant — peut être appelé à tout moment, même pendant
    /// que `run` est en cours d'exécution.
    ///
    /// gensis.md : les workflows de longue durée ont besoin d'observabilité.
    #[shared]
    async fn get_status() -> Result<String, HandlerError>;

    /// Soumettre une décision d'approbation (human-in-the-loop).
    ///
    /// gensis.md : « Some workflows need human input mid-stream — approval
    /// of a pricing change, validation of a data anomaly. »
    #[shared]
    async fn approve(decision: Json<ApprovalDecision>) -> Result<(), HandlerError>;
}

// ===========================================================================
// Implémentation
// ===========================================================================

pub struct DataPipelineImpl;

impl DataPipeline for DataPipelineImpl {
    // -----------------------------------------------------------------------
    // run() — Le pipeline complet, étape par étape
    //
    // CE QUI REND CE CODE SPÉCIAL :
    //   Ce code ressemble à une fonction async normale. Mais si le processus
    //   crash après l'étape 3, Restate relance le workflow, rejoue les étapes
    //   1-3 en retournant les résultats déjà persistés, et reprend à l'étape 4.
    //   Aucune ré-exécution. Aucun code de checkpoint. La durabilité est
    //   invisible.
    //
    // gensis.md : « failure at step 4 means restarting from step 1. This
    // wastes work and can cause side effects. Durable execution solves this
    // structurally. »
    // -----------------------------------------------------------------------
    async fn run(
        &self,
        mut ctx: WorkflowContext<'_>,
        input: Json<DataFile>,
    ) -> Result<Json<PipelineReport>, HandlerError> {
        let file = input.0;
        let file_id = file.file_id.clone();
        tracing::info!(file_id = %file_id, "pipeline: starting");

        // Enregistrer le démarrage dans le tracker (Virtual Object)
        // Cet appel est lui-même durable — journalisé par Restate.
        let tracker_state = TrackerState {
            pipeline_id: file_id.clone(),
            status: "started".to_string(),
            started_at: "2024-01-01T00:00:00Z".to_string(),
            ..Default::default()
        };
        ctx.object_client::<PipelineTrackerClient>(&file_id)
            .record_start(Json(tracker_state))
            .call()
            .await?;

        // -------------------------------------------------------------------
        // ÉTAPE 1 : Validation du schéma
        //
        // ctx.run(|| ...) journalise le résultat. Sur replay, Restate
        // retourne le résultat stocké sans ré-exécuter la closure.
        //
        // Pas de retry policy personnalisé ici — la validation est une
        // opération locale déterministe. Si le schéma est invalide, c'est
        // une TerminalError (pas de retry possible, les données sont mauvaises).
        // -------------------------------------------------------------------
        ctx.set("status", PipelineStatus::Validating.to_string());

        let file_for_validate = file.clone();
        let validation = ctx
            .run(|| async move {
                tracing::info!("step 1: validating schema");

                // Simulation de validation
                if file_for_validate.schema_name.is_empty() {
                    return Err(TerminalError::new("schema_name is required").into());
                }
                if file_for_validate.row_count == 0 {
                    return Err(TerminalError::new("file has zero rows").into());
                }

                // Json<T> wraps nos types customs pour implémenter
                // restate_sdk::serde::{Serialize, Deserialize}
                Ok(Json(ValidationResult {
                    valid: true,
                    errors: vec![],
                    validated_at: "2024-01-01T00:00:01Z".to_string(),
                }))
            })
            .name("validate_schema")
            .await?
            .0;

        tracing::info!(valid = validation.valid, "step 1 complete");

        // -------------------------------------------------------------------
        // ÉTAPE 2 : Enrichissement via API externe
        //
        // gensis.md : « The external API might time out. »
        //
        // On configure un RunRetryPolicy avec backoff exponentiel :
        //   - Délai initial : 200ms
        //   - Facteur exponentiel : 2.0 (200ms → 400ms → 800ms → 1.6s → 3.2s)
        //   - Délai max : 5s
        //   - Tentatives max : 5
        //
        // Si toutes les tentatives échouent, l'erreur devient terminale.
        // Mais sur replay, si l'enrichissement avait réussi, Restate
        // retourne directement le résultat stocké — pas de ré-appel API.
        //
        // gensis.md : « re-enriching from a rate-limited API... durable
        // execution handles retries, timeouts, and partial completion
        // without any manual state management »
        // -------------------------------------------------------------------
        ctx.set("status", PipelineStatus::Enriching.to_string());

        let api_retry_policy = RunRetryPolicy::default()
            .initial_delay(Duration::from_millis(200))
            .exponentiation_factor(2.0)
            .max_delay(Duration::from_secs(5))
            .max_attempts(5)
            .max_duration(Duration::from_secs(30));

        let enrich_file_id = file.file_id.clone();
        let enrich_row_count = file.row_count;
        let enrichment = ctx
            .run(|| async move {
                tracing::info!("step 2: enriching from external API");

                // En production, ceci serait un vrai appel HTTP.
                // La simulation montre que l'opération est non-déterministe
                // (réseau) — c'est pourquoi elle DOIT être dans ctx.run().
                Ok(Json(EnrichedData {
                    file_id: enrich_file_id,
                    original_rows: enrich_row_count,
                    enriched_fields: vec![
                        "industry_code".to_string(),
                        "risk_score".to_string(),
                        "geo_region".to_string(),
                    ],
                    api_version: "v2.3.1".to_string(),
                }))
            })
            .name("enrich_from_api")
            .retry_policy(api_retry_policy)
            .await?
            .0;

        tracing::info!(
            enriched_fields = enrichment.enriched_fields.len(),
            "step 2 complete"
        );

        // -------------------------------------------------------------------
        // GATE D'APPROBATION — Human-in-the-loop via Durable Promise
        //
        // gensis.md : « Some workflows need human input mid-stream —
        // approval of a pricing change, validation of a data anomaly,
        // confirmation of a model retraining trigger. Temporal natively
        // supports "signals"... can pause a workflow for days or weeks
        // waiting for human input, then resume exactly where it left off. »
        //
        // Restate utilise des "promises" durables au lieu de "signals" :
        //   - ctx.promise("approval") bloque jusqu'à résolution
        //   - Le shared handler approve() résout la promise
        //   - La promise survit aux crashes — si le process redémarre,
        //     Restate re-attend la même promise au même endroit
        //   - Le workflow peut attendre des heures, jours, ou semaines
        // -------------------------------------------------------------------
        ctx.set("status", PipelineStatus::AwaitingApproval.to_string());
        tracing::info!("pipeline paused — awaiting human approval");

        let decision: ApprovalDecision = ctx.promise::<Json<ApprovalDecision>>("approval").await?.0;

        tracing::info!(
            approved = decision.approved,
            reviewer = %decision.reviewer,
            "approval decision received"
        );

        // -------------------------------------------------------------------
        // SAGA PATTERN — Compensation sur rejet
        //
        // gensis.md : « When a multi-step process needs to be all-or-nothing
        // across multiple services... each activity has a compensating
        // activity that undoes it on failure. »
        //
        // Si l'approbation est refusée, on compense les étapes précédentes :
        //   - Annuler l'enrichissement (ex: révoquer les données tierces)
        //   - Pas besoin de "dé-valider" (lecture seule, pas de side-effect)
        //
        // En production, chaque compensation serait aussi un ctx.run()
        // pour être elle-même durable.
        // -------------------------------------------------------------------
        if !decision.approved {
            tracing::warn!(
                reviewer = %decision.reviewer,
                comment = %decision.comment,
                "approval rejected — initiating saga compensation"
            );

            let comp_file_id = file_id.clone();
            ctx.run(|| async move {
                tracing::info!(
                    file_id = %comp_file_id,
                    "saga: compensating enrichment (revoking third-party data)"
                );
                // En production : appel API pour révoquer/supprimer les données enrichies
                Ok(())
            })
            .name("compensate_enrichment")
            .await?;

            ctx.set(
                "status",
                PipelineStatus::Failed("rejected by reviewer".to_string()).to_string(),
            );

            ctx.object_client::<PipelineTrackerClient>(&file_id)
                .record_completion(Json(false))
                .call()
                .await?;

            return Err(TerminalError::new(format!(
                "pipeline rejected by {}: {}",
                decision.reviewer, decision.comment
            ))
            .into());
        }

        // -------------------------------------------------------------------
        // ÉTAPE 3 : Inférence ML
        //
        // gensis.md : « The ML service might be overloaded. »
        //
        // Retry policy adapté au ML : délai initial plus long (le service
        // met du temps à se décharger), moins de tentatives.
        // -------------------------------------------------------------------
        ctx.set("status", PipelineStatus::RunningInference.to_string());

        let ml_retry_policy = RunRetryPolicy::default()
            .initial_delay(Duration::from_millis(500))
            .exponentiation_factor(2.0)
            .max_delay(Duration::from_secs(10))
            .max_attempts(3)
            .max_duration(Duration::from_secs(60));

        let inference = ctx
            .run(|| async {
                tracing::info!("step 3: running ML inference");

                Ok(Json(InferenceResult {
                    model_version: "resnet-v4.2".to_string(),
                    predictions: vec![0.92, 0.87, 0.95, 0.73, 0.88],
                    mean_confidence: 0.87,
                }))
            })
            .name("run_inference")
            .retry_policy(ml_retry_policy)
            .await?
            .0;

        tracing::info!(confidence = inference.mean_confidence, "step 3 complete");

        // -------------------------------------------------------------------
        // ÉTAPE 4 : Application des règles métier
        //
        // Opération locale, déterministe. On la met quand même dans ctx.run()
        // pour que le résultat soit journalisé et sauté lors du replay.
        // Pas de retry policy personnalisé — si ça échoue, c'est un bug.
        // -------------------------------------------------------------------
        ctx.set("status", PipelineStatus::ApplyingRules.to_string());

        let rules_inference = inference.clone();
        let rules = ctx
            .run(|| async move {
                tracing::info!("step 4: applying business rules");

                let flagged = rules_inference
                    .predictions
                    .iter()
                    .filter(|&&p| p < 0.80)
                    .count() as u32;

                Ok(Json(RulesResult {
                    passed_rules: 3,
                    flagged_records: flagged,
                    applied_rules: vec![
                        "min_confidence_0.80".to_string(),
                        "max_risk_score_0.95".to_string(),
                        "geo_compliance_check".to_string(),
                    ],
                }))
            })
            .name("apply_rules")
            .await?
            .0;

        tracing::info!(flagged = rules.flagged_records, "step 4 complete");

        // -------------------------------------------------------------------
        // ÉTAPE 5 : Écriture des résultats
        //
        // gensis.md : « Without durable execution, failure after write_results
        // but before notify_customer could cause double-writes on restart.
        // With journaling, the write is recorded as complete and skipped
        // on replay. »
        //
        // DÉTERMINISME : on utilise ctx.rand_uuid() au lieu de
        // uuid::Uuid::new_v4(). Le UUID généré est déterministe (basé sur
        // l'invocation ID), donc identique sur replay.
        //
        // gensis.md : « Workflow code must be deterministic — no random
        // numbers, no current time, no non-deterministic operations. These
        // must be done in activities, not workflows. »
        // -------------------------------------------------------------------
        ctx.set("status", PipelineStatus::WritingResults.to_string());

        let write_id = ctx.rand_uuid().to_string();
        let write_row_count = file.row_count;
        let write_receipt = ctx
            .run(|| async move {
                tracing::info!(write_id = %write_id, "step 5: writing results");

                Ok(Json(WriteReceipt {
                    destination: "snowflake://warehouse/results".to_string(),
                    records_written: write_row_count,
                    write_id,
                }))
            })
            .name("write_results")
            .await?
            .0;

        tracing::info!(records = write_receipt.records_written, "step 5 complete");

        // -------------------------------------------------------------------
        // ÉTAPE 6 : Notification client
        // -------------------------------------------------------------------
        ctx.set("status", PipelineStatus::Notifying.to_string());

        let notify_file_id = file_id.clone();
        let notification_sent = ctx
            .run(|| async move {
                tracing::info!(
                    file_id = %notify_file_id,
                    "step 6: notifying customer"
                );
                // En production : email, webhook, Slack, etc.
                Ok(true)
            })
            .name("notify_customer")
            .await?;

        // -------------------------------------------------------------------
        // FINALISATION
        // -------------------------------------------------------------------
        ctx.set("status", PipelineStatus::Completed.to_string());

        // Notifier le tracker que le pipeline est terminé
        ctx.object_client::<PipelineTrackerClient>(&file_id)
            .record_completion(Json(true))
            .call()
            .await?;

        tracing::info!(file_id = %file_id, "pipeline completed successfully");

        Ok(Json(PipelineReport {
            file_id,
            validation,
            enrichment,
            inference,
            rules,
            write_receipt,
            notification_sent,
        }))
    }

    // -----------------------------------------------------------------------
    // get_status() — Shared handler pour query le statut
    //
    // Les shared handlers s'exécutent en parallèle avec le handler run().
    // Ils ont un accès en lecture seule au K/V state du workflow.
    //
    // Usage : curl http://restate:8080/DataPipeline/my-pipeline-id/get_status
    // -----------------------------------------------------------------------
    async fn get_status(&self, ctx: SharedWorkflowContext<'_>) -> Result<String, HandlerError> {
        let status = ctx
            .get::<String>("status")
            .await?
            .unwrap_or_else(|| "unknown".to_string());
        Ok(status)
    }

    // -----------------------------------------------------------------------
    // approve() — Shared handler pour résoudre la promise d'approbation
    //
    // Ce handler peut être appelé depuis l'extérieur (API REST, UI, CLI)
    // pour débloquer le workflow qui attend à la gate d'approbation.
    //
    // La résolution de la promise est elle-même journalisée — si le
    // process crash après la résolution mais avant que run() la traite,
    // la résolution est rejouée.
    //
    // Usage : curl -X POST http://restate:8080/DataPipeline/my-pipeline-id/approve \
    //           -H 'content-type: application/json' \
    //           -d '{"approved": true, "reviewer": "alice", "comment": "LGTM"}'
    // -----------------------------------------------------------------------
    async fn approve(
        &self,
        ctx: SharedWorkflowContext<'_>,
        decision: Json<ApprovalDecision>,
    ) -> Result<(), HandlerError> {
        tracing::info!(
            reviewer = %decision.0.reviewer,
            approved = decision.0.approved,
            "received approval decision"
        );
        ctx.resolve_promise::<Json<ApprovalDecision>>("approval", decision);
        Ok(())
    }
}
