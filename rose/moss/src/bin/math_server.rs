use moss::proto::math_service_server::{MathService, MathServiceServer};
use moss::proto::{AddRequest, AddResponse, PredictRequest, PredictResponse};
use tonic::{transport::Server, Request, Response, Status};

#[derive(Default)]
struct Math;

#[tonic::async_trait]
impl MathService for Math {
    async fn add(&self, req: Request<AddRequest>) -> Result<Response<AddResponse>, Status> {
        let r = req.into_inner();
        tracing::info!(a = r.a, b = r.b, "add");
        Ok(Response::new(AddResponse { result: r.a + r.b }))
    }

    async fn predict(
        &self,
        req: Request<PredictRequest>,
    ) -> Result<Response<PredictResponse>, Status> {
        let r = req.into_inner();
        let weights = [0.3_f64, 0.5, 0.2];
        let bias = 1.0_f64;
        let prediction: f64 = r
            .features
            .iter()
            .zip(weights.iter())
            .map(|(x, w)| x * w)
            .sum::<f64>()
            + bias;
        tracing::info!(features = ?r.features, prediction, "predict");
        Ok(Response::new(PredictResponse {
            prediction,
            model_version: "v0.1.0-toy".into(),
        }))
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();
    let addr = std::env::var("BIND_ADDR")
        .unwrap_or_else(|_| "127.0.0.1:50051".into())
        .parse()?;
    tracing::info!("math gRPC server listening on {}", addr);
    Server::builder()
        .add_service(MathServiceServer::new(Math))
        .serve(addr)
        .await?;
    Ok(())
}
