// ---------------------------------------------------------------------------
// Point d'entrée — Serveur HTTP Restate
//
// gensis.md : « Restate is a single binary that sits between your services
// and makes their function calls durable. You write HTTP handlers. Restate
// intercepts the calls, journals every step, and replays on failure. It
// feels more like middleware than an orchestration platform. »
//
// Architecture :
//
//   Client → Restate Server (binary) → Ce process (port 9080)
//                                       ├── DataPipeline  (workflow)
//                                       └── PipelineTracker (virtual object)
//
// Déploiement :
//   1. Lancer ce process : cargo run  (écoute sur :9080)
//   2. Lancer Restate :    restate-server
//   3. Enregistrer :       restate deployments register http://localhost:9080
//   4. Invoquer :          curl -X POST http://localhost:8080/DataPipeline/my-run/run \
//                            -H 'content-type: application/json' \
//                            -d '{"file_id":"f1","file_name":"data.csv",...}'
//
// Les deux services sont servis depuis un seul process sur un seul port.
// Restate route vers le bon handler en fonction du nom de service et du
// nom de handler dans l'URL.
// ---------------------------------------------------------------------------

use restate_sdk::prelude::*;
// Les traits doivent être importés pour que .serve() soit disponible sur les Impl structs
use restate_pipeline::pipeline::DataPipeline;
use restate_pipeline::tracker::PipelineTracker;
use restate_pipeline::{DataPipelineImpl, PipelineTrackerImpl};

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "restate_pipeline=info".into()),
        )
        .init();

    tracing::info!("starting restate pipeline services on 0.0.0.0:9080");

    // Bind les deux services au même endpoint.
    // Le workflow (DataPipeline) et le virtual object (PipelineTracker)
    // cohabitent dans le même process — Restate les distingue par nom.
    let endpoint = Endpoint::builder()
        .bind(DataPipelineImpl.serve())
        .bind(PipelineTrackerImpl.serve())
        .build();

    HttpServer::new(endpoint)
        .listen_and_serve("0.0.0.0:9080".parse().unwrap())
        .await;
}
