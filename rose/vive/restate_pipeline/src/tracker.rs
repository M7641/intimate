// ---------------------------------------------------------------------------
// PipelineTracker — Virtual Object Restate
//
// gensis.md mentionne que Restate supporte des « Virtual Objects » avec un
// état K/V isolé par clé. Ici, chaque pipeline_id est une clé distincte.
//
// Points clés démontrés :
//   • #[restate_sdk::object] — déclare un Virtual Object
//   • Handlers exclusifs (write) vs #[shared] (read concurrent)
//   • ctx.get / ctx.set — état K/V géré par Restate, pas par nous
//   • Single-writer guarantee — si deux appels arrivent pour la même clé,
//     Restate les sérialise automatiquement (pas de lock manuel)
// ---------------------------------------------------------------------------

use restate_sdk::prelude::*;

use crate::types::TrackerState;

#[restate_sdk::object]
pub trait PipelineTracker {
    /// Enregistre le démarrage d'un pipeline.
    /// Handler exclusif : un seul appel à la fois par clé (pipeline_id).
    async fn record_start(state: Json<TrackerState>) -> Result<(), HandlerError>;

    /// Enregistre la complétion d'une étape.
    /// Handler exclusif : les appels concurrents pour la même clé sont sérialisés.
    async fn record_step(step_name: String) -> Result<(), HandlerError>;

    /// Enregistre la fin du pipeline (succès ou échec).
    async fn record_completion(success: Json<bool>) -> Result<(), HandlerError>;

    /// Lecture de l'état courant — handler partagé, peut s'exécuter en
    /// parallèle avec d'autres lectures (mais pas avec les écritures).
    ///
    /// gensis.md : « query workflow state from outside without interfering »
    #[shared]
    async fn get_state() -> Result<Json<TrackerState>, HandlerError>;
}

pub struct PipelineTrackerImpl;

impl PipelineTracker for PipelineTrackerImpl {
    // -----------------------------------------------------------------------
    // record_start — initialise l'état du tracker pour ce pipeline_id
    //
    // L'état est isolé par clé : le tracker pour "pipeline-abc" est
    // complètement indépendant de "pipeline-xyz". Pas de table partagée,
    // pas de conflit — Restate gère la partition automatiquement.
    // -----------------------------------------------------------------------
    async fn record_start(
        &self,
        ctx: ObjectContext<'_>,
        state: Json<TrackerState>,
    ) -> Result<(), HandlerError> {
        tracing::info!(
            pipeline_id = %state.0.pipeline_id,
            "tracker: recording pipeline start"
        );
        // Json<T> bridge les traits restate_sdk::serde pour nos types customs
        ctx.set("state", Json(state.0));
        Ok(())
    }

    // -----------------------------------------------------------------------
    // record_step — lecture-modification-écriture atomique
    //
    // Grâce à la garantie single-writer-per-key de Restate, ce pattern
    // read-modify-write n'a pas besoin de verrou ou de CAS. Si deux étapes
    // tentent de s'enregistrer simultanément pour le même pipeline_id,
    // Restate met le second appel en file d'attente.
    // -----------------------------------------------------------------------
    async fn record_step(
        &self,
        ctx: ObjectContext<'_>,
        step_name: String,
    ) -> Result<(), HandlerError> {
        let mut state: TrackerState = ctx
            .get::<Json<TrackerState>>("state")
            .await?
            .map(|j| j.0)
            .unwrap_or_default();

        tracing::info!(
            pipeline_id = %state.pipeline_id,
            step = %step_name,
            "tracker: recording step completion"
        );

        state.steps_completed.push(step_name);
        state.status = format!("step_{}_done", state.steps_completed.len());
        ctx.set("state", Json(state));
        Ok(())
    }

    // -----------------------------------------------------------------------
    // record_completion — marque le pipeline comme terminé
    // -----------------------------------------------------------------------
    async fn record_completion(
        &self,
        ctx: ObjectContext<'_>,
        success: Json<bool>,
    ) -> Result<(), HandlerError> {
        let mut state: TrackerState = ctx
            .get::<Json<TrackerState>>("state")
            .await?
            .map(|j| j.0)
            .unwrap_or_default();

        if success.0 {
            state.status = "completed".to_string();
            tracing::info!(pipeline_id = %state.pipeline_id, "tracker: pipeline completed");
        } else {
            state.status = "failed".to_string();
            tracing::warn!(pipeline_id = %state.pipeline_id, "tracker: pipeline failed");
        }

        ctx.set("state", Json(state));
        Ok(())
    }

    // -----------------------------------------------------------------------
    // get_state — lecture partagée (shared handler)
    //
    // Les shared handlers peuvent s'exécuter en parallèle entre eux et même
    // pendant qu'un handler exclusif est en attente. Ils n'ont qu'un accès
    // en lecture au state.
    // -----------------------------------------------------------------------
    async fn get_state(
        &self,
        ctx: SharedObjectContext<'_>,
    ) -> Result<Json<TrackerState>, HandlerError> {
        let state: TrackerState = ctx
            .get::<Json<TrackerState>>("state")
            .await?
            .map(|j| j.0)
            .unwrap_or_default();
        Ok(Json(state))
    }
}
