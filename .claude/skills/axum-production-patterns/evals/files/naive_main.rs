use axum::{routing::get, Router, Json, extract::State};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;

#[derive(Serialize)]
struct Widget {
    id: i64,
    name: String,
}

#[derive(Deserialize)]
struct NewWidget {
    name: String,
}

async fn list_widgets(State(pool): State<PgPool>) -> Json<Vec<Widget>> {
    let rows = sqlx::query_as!(Widget, "SELECT id, name FROM widgets")
        .fetch_all(&pool)
        .await
        .unwrap();
    Json(rows)
}

async fn create_widget(State(pool): State<PgPool>, Json(w): Json<NewWidget>) -> Json<Widget> {
    let row = sqlx::query_as!(
        Widget,
        "INSERT INTO widgets (name) VALUES ($1) RETURNING id, name",
        w.name
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    Json(row)
}

#[tokio::main]
async fn main() {
    let pool = PgPool::connect("postgres://localhost/widgets").await.unwrap();

    let app = Router::new()
        .route("/widgets", get(list_widgets).post(create_widget))
        .with_state(pool);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
