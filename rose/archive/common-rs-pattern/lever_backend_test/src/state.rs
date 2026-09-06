use common_rs::db::Db;
use common_rs::env::EnvManager;

#[derive(Clone)]
pub struct AppState {
    pub db: Db,
    pub env: EnvManager,
}

impl AppState {
    pub fn new(db: Db, env: EnvManager) -> Self {
        Self { db, env }
    }
}
