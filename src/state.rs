use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use sqlx::SqlitePool;
use tokio::sync::Semaphore;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use crate::config::Config;
use crate::tokenize::TokenizerEngine;

pub struct AppState {
    pub db: SqlitePool,
    pub cpu_sem: Arc<Semaphore>,
    pub inflight: Mutex<HashMap<Uuid, CancellationToken>>,
    pub config: Config,
    pub engine: Arc<TokenizerEngine>,
}

impl AppState {
    pub fn new(db: SqlitePool, config: Config, engine: TokenizerEngine) -> Arc<Self> {
        let cpu_sem = Arc::new(Semaphore::new(config.max_concurrency));
        tracing::info!(cpu_permits = config.max_concurrency, "信号量已初始化");

        Arc::new(Self {
            db,
            cpu_sem,
            inflight: Mutex::new(HashMap::new()),
            config,
            engine: Arc::new(engine),
        })
    }

    pub fn register_inflight(&self, id: Uuid) -> CancellationToken {
        let token = CancellationToken::new();
        let child = token.child_token();
        self.inflight.lock().unwrap().insert(id, token);
        child
    }

    pub fn remove_inflight(&self, id: &Uuid) {
        self.inflight.lock().unwrap().remove(id);
    }

    pub fn cancel_inflight(&self, id: &Uuid) -> bool {
        if let Some(token) = self.inflight.lock().unwrap().get(id) {
            token.cancel();
            true
        } else {
            false
        }
    }
}
