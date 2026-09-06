
## Concrete Example Domain

A fraud detection system for a payments platform. Raw transaction events flow in, features are computed, a model scores transactions in real-time, and a web application displays results and allows analysts to review flagged transactions.

This maps to the three-plane separation:

- **Feature plane**: Ingests raw events, computes features, serves them for training and inference
- **Model plane**: Trains on historical features, serves predictions via a stateless endpoint
- **Decision plane**: Web application that calls the model, applies business rules, and presents results

---

## 1. Schema Contracts Layer

Everything starts with shared type definitions. These live in a dedicated `contracts` repository that all services depend on.

### Directory structure

```
contracts/
├── proto/
│   ├── events/
│   │   └── transaction.proto        # Raw event schema
│   ├── features/
│   │   └── transaction_features.proto # Computed feature vector
│   ├── inference/
│   │   └── fraud_scoring.proto       # Model input/output contract
│   └── api/
│       └── review_service.proto      # Web app API types
├── quality/
│   ├── transaction_events.yaml       # Quality expectations for raw events
│   └── transaction_features.yaml     # Quality expectations for features
├── catalog/
│   └── datasets.yaml                 # Data product registry
├── buf.yaml                          # Buf configuration for linting/breaking change detection
└── buf.gen.yaml                      # Code generation targets (Rust, Python, TypeScript)
```

### Example: transaction event schema

```protobuf
// proto/events/transaction.proto
syntax = "proto3";
package events;

import "google/protobuf/timestamp.proto";

message Transaction {
  string transaction_id = 1;
  string user_id = 2;
  string merchant_id = 3;

  // Amount in minor units (pence/cents) to avoid floating point
  int64 amount_minor_units = 4;
  string currency = 5;

  google.protobuf.Timestamp created_at = 6;
  string payment_method = 7;           // "card", "bank_transfer", "wallet"
  string merchant_category_code = 8;   // ISO 18245 MCC

  // Geolocation of the transaction
  optional double latitude = 9;
  optional double longitude = 10;
  optional string ip_address = 11;
  optional string device_fingerprint = 12;
}
```

### Example: feature vector contract

```protobuf
// proto/features/transaction_features.proto
syntax = "proto3";
package features;

// This is the contract between the feature plane and the model plane.
// The model expects exactly this shape. Any change here requires
// model retraining and a coordinated rollout.
message TransactionFeatures {
  // Identity
  string transaction_id = 1;
  string user_id = 2;

  // Transaction-level features
  double amount_normalised = 3;         // Amount / user's median transaction
  int32 hour_of_day = 4;
  int32 day_of_week = 5;
  bool is_international = 6;

  // User behavioural features (computed from historical data)
  double avg_transaction_amount_30d = 7;
  double std_transaction_amount_30d = 8;
  int32 transaction_count_24h = 9;
  int32 transaction_count_7d = 10;
  int32 unique_merchants_7d = 11;
  double avg_time_between_transactions_hours = 12;

  // Velocity features
  double amount_sum_1h = 13;
  double amount_sum_24h = 14;
  int32 distinct_countries_24h = 15;

  // Device/location features
  bool is_new_device = 16;
  double distance_from_usual_location_km = 17;
  bool is_known_merchant = 18;

  // Metadata (not model inputs, but carried for traceability)
  int64 feature_computation_timestamp_ms = 19;
  string feature_pipeline_version = 20;
}
```

### Example: inference contract

```protobuf
// proto/inference/fraud_scoring.proto
syntax = "proto3";
package inference;

import "features/transaction_features.proto";

message FraudScoreRequest {
  features.TransactionFeatures features = 1;
}

message FraudScoreResponse {
  string transaction_id = 1;
  double fraud_probability = 2;       // 0.0 to 1.0

  // Explainability: top contributing features
  repeated FeatureContribution top_contributors = 3;

  // Model metadata for audit trail
  string model_version = 4;
  int64 inference_timestamp_ms = 5;
  int32 inference_latency_us = 6;
}

message FeatureContribution {
  string feature_name = 1;
  double contribution = 2;  // SHAP value or similar
}
```

### Breaking change detection

Using **Buf** (buf.build) for the proto registry gives you CI-level enforcement:

```yaml
# buf.yaml
version: v1
breaking:
  use:
    - FILE # Catches removed fields, changed types, etc.
  ignore:
    - quality/ # Quality configs aren't compiled
```

On every PR to the contracts repo, CI runs `buf breaking` against the main branch. If a breaking change is detected, the PR is blocked until it's explicitly acknowledged and versioned.

### Code generation

```yaml
# buf.gen.yaml
version: v1
plugins:
  # Rust (using prost)
  - plugin: buf.build/community/neoeinstein-prost
    out: gen/rust/src
    opt:
      - compile_well_known_types
  # Python (using betterproto for cleaner generated code)
  - plugin: buf.build/community/danielgtaylor-python-betterproto
    out: gen/python
  # TypeScript (using ts-proto for idiomatic TS)
  - plugin: buf.build/community/stephenh-ts-proto
    out: gen/typescript
    opt:
      - esModuleInterop=true
      - outputJsonMethods=true
```

This means a schema change in one place propagates typed definitions to all three runtimes. Rust services get compile-time enforcement, TypeScript gets type checking, Python gets runtime validation.

---

## 2. Feature Plane (Rust + Python)

The feature plane has two execution paths that share the same transformation logic:

### Batch path (training)

Used to compute features over historical data for model training.

```
PostgreSQL (raw events)
  → Feature computation (SQL + Python)
  → Feature table in PostgreSQL (or Parquet files in object storage)
  → Training pipeline reads features
```

### Real-time path (serving)

Used to compute features for live inference.

```
Transaction event (via NATS/gRPC)
  → Rust feature service
  → Reads user history from PostgreSQL / Redis cache
  → Computes features
  → Returns TransactionFeatures proto
```

### The training/serving skew problem

The critical constraint: **batch and real-time paths must produce identical features for the same input**. Two approaches that work:

**Approach A: Shared SQL definitions**

Define feature transforms as SQL, use them in both batch (run against the warehouse) and real-time (run against a real-time materialised view or cache). This works well for aggregate features.

```sql
-- features/sql/user_transaction_stats.sql
-- This SQL is used by both the batch pipeline and the real-time feature service.
-- Batch: runs against the full transaction history table.
-- Real-time: runs against a rolling window maintained in a materialised view.

SELECT
  user_id,
  AVG(amount_minor_units) / 100.0 AS avg_transaction_amount_30d,
  STDDEV(amount_minor_units) / 100.0 AS std_transaction_amount_30d,
  COUNT(*) AS transaction_count_7d,
  COUNT(DISTINCT merchant_id) AS unique_merchants_7d
FROM transactions
WHERE created_at > NOW() - INTERVAL '30 days'
GROUP BY user_id;
```

**Approach B: Feature computation in Rust, called from both paths**

For complex features that don't reduce to SQL (geospatial distance, device fingerprint matching), implement them in a Rust library that's called by both the batch pipeline (via PyO3 bindings) and the real-time service (native Rust).

```rust
// feature_engine/src/lib.rs
use prost::Message;

// Generated from proto
include!(concat!(env!("OUT_DIR"), "/features.rs"));

pub struct FeatureComputer {
    // Holds any configuration or lookup data needed
}

impl FeatureComputer {
    /// Compute features for a single transaction.
    /// This function is deterministic: same inputs → same outputs.
    /// Called by both the real-time Axum service and the batch PyO3 bridge.
    pub fn compute(
        &self,
        transaction: &Transaction,
        user_history: &UserHistory,
    ) -> TransactionFeatures {
        TransactionFeatures {
            transaction_id: transaction.transaction_id.clone(),
            user_id: transaction.user_id.clone(),
            amount_normalised: self.normalise_amount(
                transaction.amount_minor_units,
                user_history.median_amount,
            ),
            hour_of_day: extract_hour(transaction.created_at.as_ref()),
            transaction_count_24h: user_history.count_24h,
            distance_from_usual_location_km: self.compute_distance(
                transaction.latitude,
                transaction.longitude,
                &user_history.usual_locations,
            ),
            is_new_device: !user_history
                .known_devices
                .contains(&transaction.device_fingerprint),
            feature_pipeline_version: env!("CARGO_PKG_VERSION").to_string(),
            // ... remaining features
            ..Default::default()
        }
    }
}
```

```python
# PyO3 bridge for batch usage
# feature_engine_py/src/lib.rs exposes compute() to Python

# In the training pipeline:
import feature_engine
import pandas as pd

def compute_features_batch(transactions_df: pd.DataFrame, user_histories: dict):
    """Compute features using the same Rust engine as real-time serving."""
    features = []
    for _, row in transactions_df.iterrows():
        history = user_histories[row["user_id"]]
        feat = feature_engine.compute(row.to_dict(), history)
        features.append(feat)
    return pd.DataFrame(features)
```

### Real-time feature service (Axum)

```rust
// feature_service/src/main.rs
use axum::{Router, routing::post, Json, extract::State};
use sqlx::PgPool;

struct AppState {
    db: PgPool,
    feature_computer: FeatureComputer,
}

async fn compute_features(
    State(state): State<AppState>,
    Json(transaction): Json<Transaction>,
) -> Json<TransactionFeatures> {
    // Fetch user history from Postgres (with Redis cache in front)
    let user_history = get_user_history(&state.db, &transaction.user_id).await;

    // Compute features using the shared engine
    let features = state.feature_computer.compute(&transaction, &user_history);

    // Validate output against quality expectations before returning
    validate_features(&features).expect("Feature quality check failed");

    Json(features)
}
```

### Quality assertions at the feature boundary

```yaml
# quality/transaction_features.yaml
dataset: transaction_features
owner: ml-platform-team
checks:
  - name: amount_normalised_range
    column: amount_normalised
    assertion: "value >= 0 AND value <= 1000"
    severity: critical # Blocks pipeline if violated

  - name: no_null_user_ids
    column: user_id
    assertion: "value IS NOT NULL"
    severity: critical

  - name: transaction_count_non_negative
    column: transaction_count_24h
    assertion: "value >= 0"
    severity: critical

  - name: feature_freshness
    assertion: "MAX(feature_computation_timestamp_ms) > UNIX_MS(NOW() - INTERVAL '2 hours')"
    severity: warning # Alerts but doesn't block

  - name: row_count_stability
    assertion: "COUNT(*) BETWEEN 0.8 * TRAILING_7D_AVG AND 1.2 * TRAILING_7D_AVG"
    severity: warning
```

These assertions run:

- **Batch path**: After feature computation, before writing to the feature table. Failures block the pipeline.
- **Real-time path**: On a sample of computed features, logged asynchronously. Anomalies trigger alerts.

---

## 3. Model Plane (Python)

### Training pipeline

```python
# training/train.py
import mlflow
from feature_engine import compute_features_batch  # Rust via PyO3
from sklearn.ensemble import GradientBoostingClassifier  # or whatever
import onnx
import skl2onnx

def train():
    # Load features from the feature table (batch-computed)
    features_df = load_features_from_postgres(
        query="SELECT * FROM transaction_features_v1 WHERE split = 'train'"
    )

    # The feature columns must match the proto definition exactly
    feature_columns = get_feature_columns_from_proto()  # Parsed from .proto file

    X = features_df[feature_columns]
    y = features_df["is_fraud"]

    model = GradientBoostingClassifier(
        n_estimators=500,
        max_depth=6,
        learning_rate=0.05,
    )
    model.fit(X, y)

    # Evaluate
    metrics = evaluate(model, X_test, y_test)

    # Convert to ONNX for language-agnostic serving
    onnx_model = skl2onnx.convert_sklearn(
        model,
        initial_types=[("features", FloatTensorType([None, len(feature_columns)]))],
    )

    # Log with full provenance
    with mlflow.start_run():
        mlflow.log_params(model.get_params())
        mlflow.log_metrics(metrics)

        # The model manifest — this is the contract
        manifest = {
            "model_version": generate_version(),
            "feature_proto_version": "transaction_features.v1",
            "expected_features": feature_columns,
            "feature_value_ranges": compute_expected_ranges(X),
            "training_data": {
                "table": "transaction_features_v1",
                "date_range": "2024-01-01 to 2025-01-01",
                "row_count": len(features_df),
                "fraud_rate": y.mean(),
            },
            "metrics": metrics,
            "training_code_git_sha": get_git_sha(),
        }
        mlflow.log_dict(manifest, "model_manifest.json")
        mlflow.onnx.log_model(onnx_model, "model")
```

### Model serving (Rust with ONNX Runtime)

The model is served by a Rust service using `ort` (ONNX Runtime bindings). This gives you single-digit millisecond inference latency.

```rust
// model_service/src/main.rs
use axum::{Router, routing::post, Json, extract::State};
use ort::{Environment, Session, Value};
use std::sync::Arc;

struct ModelState {
    session: Session,
    manifest: ModelManifest,
}

#[derive(Deserialize)]
struct ModelManifest {
    model_version: String,
    expected_features: Vec<String>,
    feature_value_ranges: HashMap<String, (f64, f64)>,
}

async fn predict(
    State(state): State<Arc<ModelState>>,
    Json(request): Json<FraudScoreRequest>,
) -> Result<Json<FraudScoreResponse>, AppError> {
    let features = &request.features;

    // Validate input against manifest
    validate_input_against_manifest(features, &state.manifest)?;

    // Convert proto features to model input tensor
    let input_array = features_to_tensor(features, &state.manifest.expected_features);

    // Run inference
    let outputs = state.session.run(
        ort::inputs!["features" => input_array]?
    )?;

    let probabilities = outputs[0].extract_tensor::<f32>()?;
    let fraud_probability = probabilities[[0, 1]] as f64;

    Ok(Json(FraudScoreResponse {
        transaction_id: features.transaction_id.clone(),
        fraud_probability,
        model_version: state.manifest.model_version.clone(),
        inference_timestamp_ms: now_ms(),
        // SHAP values computed separately if needed
        top_contributors: vec![],
        ..Default::default()
    }))
}

fn validate_input_against_manifest(
    features: &TransactionFeatures,
    manifest: &ModelManifest,
) -> Result<(), AppError> {
    // Check that feature values are within expected ranges
    // This catches training/serving skew and upstream data issues
    for (name, (min, max)) in &manifest.feature_value_ranges {
        let value = get_feature_value(features, name);
        if value < *min || value > *max {
            tracing::warn!(
                feature = name,
                value = value,
                expected_min = min,
                expected_max = max,
                "Feature value outside expected range"
            );
            // Don't fail — log and monitor. If this becomes frequent,
            // it indicates drift or an upstream issue.
        }
    }
    Ok(())
}
```

### Model deployment flow

```
1. Training pipeline produces ONNX model + manifest
2. Model is stored in MLflow / S3 with version tag
3. Model service is deployed with new model version (blue-green or canary)
4. Shadow mode: new model runs alongside current model, predictions logged but not acted on
5. Comparison metrics computed (accuracy, latency, feature distribution alignment)
6. If metrics pass thresholds → promote to primary
7. Old model version retained for instant rollback
```

---

## 4. Decision Plane (TypeScript Web Application)

The web application is a consumer of the model's output. It applies business rules and presents results.

```typescript
// services/fraud-review/src/fraud-scoring.ts
import { FraudScoreResponse } from "@company/contracts/inference/fraud_scoring";

interface FraudDecision {
  action: "allow" | "review" | "block";
  reason: string;
  score: number;
  modelVersion: string;
}

// Business rules live HERE, not in the model service.
// Product managers can change these without retraining.
export function makeFraudDecision(
  score: FraudScoreResponse,
  transactionAmount: number,
  userTrustTier: "new" | "standard" | "trusted",
): FraudDecision {
  const thresholds = getThresholds(userTrustTier);

  if (score.fraudProbability >= thresholds.block) {
    return {
      action: "block",
      reason: `Fraud probability ${(score.fraudProbability * 100).toFixed(1)}% exceeds block threshold`,
      score: score.fraudProbability,
      modelVersion: score.modelVersion,
    };
  }

  if (
    score.fraudProbability >= thresholds.review ||
    (score.fraudProbability >= thresholds.reviewHighValue &&
      transactionAmount > 50000)
  ) {
    return {
      action: "review",
      reason: buildReviewReason(score),
      score: score.fraudProbability,
      modelVersion: score.modelVersion,
    };
  }

  return {
    action: "allow",
    reason: "Within acceptable risk parameters",
    score: score.fraudProbability,
    modelVersion: score.modelVersion,
  };
}

function getThresholds(tier: string) {
  // These are configurable without code changes — stored in config/database
  const configs: Record<string, any> = {
    new: { block: 0.8, review: 0.5, reviewHighValue: 0.3 },
    standard: { block: 0.85, review: 0.6, reviewHighValue: 0.4 },
    trusted: { block: 0.92, review: 0.75, reviewHighValue: 0.55 },
  };
  return configs[tier] ?? configs.standard;
}
```

### The integration point: transaction processing

```typescript
// services/payment-processor/src/process-transaction.ts

export async function processTransaction(
  transaction: Transaction,
): Promise<ProcessingResult> {
  // 1. Compute features (calls the Rust feature service)
  const features = await featureClient.computeFeatures(transaction);

  // 2. Score transaction (calls the Rust model service)
  const score = await modelClient.scoreFraud(features);

  // 3. Apply business rules (pure function, no external calls)
  const decision = makeFraudDecision(
    score,
    transaction.amountMinorUnits,
    await getUserTrustTier(transaction.userId),
  );

  // 4. Act on decision
  switch (decision.action) {
    case "allow":
      await completePayment(transaction);
      break;
    case "review":
      await holdForReview(transaction, decision);
      await notifyReviewQueue(transaction, decision, score);
      break;
    case "block":
      await blockPayment(transaction);
      await notifyUser(transaction, decision);
      break;
  }

  // 5. Log everything for monitoring and future training data
  await logDecision({
    transactionId: transaction.transactionId,
    features,
    score,
    decision,
    timestamp: Date.now(),
  });

  return { decision };
}
```

---

## 5. Monitoring and Feedback Loop

### What to monitor at each boundary

```
┌──────────────────────────────────────────────────────────────────┐
│                        Monitoring Points                         │
├────────────────┬─────────────────────────────────────────────────┤
│ Raw Events     │ Volume (events/sec), schema violations,         │
│   → Features   │ null rates, latency of feature computation      │
├────────────────┼─────────────────────────────────────────────────┤
│ Features       │ Feature distribution drift (PSI/KL divergence   │
│   → Model      │ vs training distribution), out-of-range values, │
│                │ feature freshness                                │
├────────────────┼─────────────────────────────────────────────────┤
│ Model          │ Prediction distribution shift, inference         │
│   → Decision   │ latency (p50/p95/p99), model version serving,   │
│                │ error rates                                      │
├────────────────┼─────────────────────────────────────────────────┤
│ Decision       │ Block/review/allow rates over time, false        │
│   → Outcome    │ positive rate (analyst overrides), false         │
│                │ negative rate (chargebacks on allowed txns)      │
└────────────────┴─────────────────────────────────────────────────┘
```

### Feedback loop for retraining

```
Analyst reviews flagged transaction
  → Labels it as fraud/not-fraud
  → Label stored in PostgreSQL with transaction_id
  → Nightly job joins labels with features from feature table
  → Creates new training dataset
  → Triggers retraining pipeline (if enough new labels accumulated)
  → New model evaluated against holdout set AND shadow mode
  → If metrics improve → deploy via canary rollout
```

---

## 6. Complete System Topology

```
                    ┌─────────────────────┐
                    │   contracts repo    │
                    │  (proto + quality)  │
                    └─────┬───┬───┬───────┘
              generates   │   │   │  generates
              Rust types  │   │   │  TS types
                    ┌─────┘   │   └─────┐
                    ▼         │         ▼
             ┌──────────┐    │   ┌──────────────┐
             │  Feature  │    │   │  Web App /    │
             │  Service  │    │   │  Payment      │
             │  (Rust/   │    │   │  Processor    │
             │   Axum)   │    │   │  (TypeScript) │
             └────┬──────┘    │   └──────┬────────┘
                  │           │          │
                  │    ┌──────┘          │ calls feature
                  │    │ generates       │ service + model
                  │    │ Python types    │ service via gRPC
                  │    ▼                 │
                  │  ┌──────────┐       │
                  │  │ Training │       │
                  │  │ Pipeline │       │
                  │  │ (Python) │       │
                  │  └────┬─────┘       │
                  │       │             │
                  │       │ produces    │
                  │       │ ONNX model  │
                  │       ▼             │
                  │  ┌──────────┐       │
                  │  │  Model   │◄──────┘
                  │  │  Service │  gRPC
                  │  │  (Rust/  │
                  │  │   Axum)  │
                  │  └──────────┘
                  │       │
                  └───────┘
              both read from
              ┌──────────┐
              │PostgreSQL │
              │ + Redis   │
              └──────────┘
```

---

## 7. Key Complexity-Reducing Properties

1. **Single source of truth for types**: The contracts repo generates typed code for all three languages. A schema change is a PR to one repo, reviewed by all affected teams.

2. **Feature computation is write-once**: The Rust feature engine is used by both batch and real-time paths. Training/serving skew is structurally impossible for features computed this way.

3. **Model is a pure function**: Features in, score out. No side effects, no database calls, no business logic. This makes it testable, reproducible, and independently deployable.

4. **Business rules are plain code**: The decision logic is ordinary TypeScript that product managers can understand and modify. No ML knowledge required.

5. **Quality assertions at every boundary**: Bad data is caught at the point of production, not downstream. Each plane validates its inputs and guarantees its outputs.

6. **Independent deployment**: Each service can be deployed, scaled, and rolled back independently. A model retrain doesn't require redeploying the web app. A threshold change doesn't require retraining.

7. **Full audit trail**: Every transaction has a logged chain: raw event → features → score → decision → outcome. This supports debugging, compliance, and future model training.
