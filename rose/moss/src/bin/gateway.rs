use axum::{extract::State, http::StatusCode, routing::post, Json, Router};
use moss::proto::math_service_client::MathServiceClient;
use moss::proto::{AddRequest, PredictRequest};
use serde::{Deserialize, Serialize};
use tonic::transport::Channel;

#[derive(Clone)]
struct AppState {
    math: MathServiceClient<Channel>,
}

#[derive(Deserialize)]
struct AddBody {
    a: f64,
    b: f64,
}

#[derive(Serialize)]
struct AddOut {
    result: f64,
}

#[derive(Deserialize)]
struct PredictBody {
    features: Vec<f64>,
}

#[derive(Serialize)]
struct PredictOut {
    prediction: f64,
    model_version: String,
}

async fn add(
    State(s): State<AppState>,
    Json(b): Json<AddBody>,
) -> Result<Json<AddOut>, (StatusCode, String)> {
    let mut client = s.math.clone();
    let resp = client
        .add(AddRequest { a: b.a, b: b.b })
        .await
        .map_err(|e| (StatusCode::BAD_GATEWAY, e.to_string()))?;
    Ok(Json(AddOut {
        result: resp.into_inner().result,
    }))
}

async fn predict(
    State(s): State<AppState>,
    Json(b): Json<PredictBody>,
) -> Result<Json<PredictOut>, (StatusCode, String)> {
    let mut client = s.math.clone();
    let resp = client
        .predict(PredictRequest {
            features: b.features,
        })
        .await
        .map_err(|e| (StatusCode::BAD_GATEWAY, e.to_string()))?;
    let r = resp.into_inner();
    Ok(Json(PredictOut {
        prediction: r.prediction,
        model_version: r.model_version,
    }))
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    let math_url =
        std::env::var("MATH_SERVICE_URL").unwrap_or_else(|_| "http://127.0.0.1:50051".into());
    let math = MathServiceClient::connect(math_url).await?;
    let state = AppState { math };

    let app = Router::new()
        .route("/add", post(add))
        .route("/predict", post(predict))
        .with_state(state);

    let bind = std::env::var("BIND_ADDR").unwrap_or_else(|_| "127.0.0.1:3000".into());
    let listener = tokio::net::TcpListener::bind(&bind).await?;
    tracing::info!("gateway listening on http://{}", listener.local_addr()?);
    axum::serve(listener, app).await?;
    Ok(())
}
