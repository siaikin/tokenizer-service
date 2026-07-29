/// tokenizer-service 入口
mod auth;
mod config;
mod db;
mod docs;
mod error;
mod routes;
mod state;
mod tokenize;

use std::{net::SocketAddr, sync::Arc};

use axum::{
    http::header,
    middleware,
    response::{Html, IntoResponse},
    routing::{get, post},
    Router,
};
use tower_http::{
    cors::{Any, CorsLayer},
    limit::RequestBodyLimitLayer,
    trace::TraceLayer,
};

use config::Config;
use db::init_db;
use state::AppState;
use tokenize::TokenizerEngine;

static PANEL_HTML: &str = include_str!("../static/index.html");
static TEST_HTML: &str = include_str!("../static/test.html");
static LLMS_TXT: &str = include_str!("../static/llms.txt");

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "tokenizer_service=info,tower_http=info".parse().unwrap()),
        )
        .init();

    tracing::info!("tokenizer-service 正在启动");

    let config = Config::from_env();
    let bind_addr: SocketAddr = config
        .bind
        .parse()
        .expect("TOKENIZER_BIND 地址格式无效");
    let max_body = config.max_body_mb * 1024 * 1024;

    let pool = init_db(&config.db_path)
        .await
        .expect("数据库初始化失败");

    let engine = TokenizerEngine::new().expect("分词引擎初始化失败");
    let state = AppState::new(pool, config, engine);

    let protected = Router::new()
        .route("/api/tokens", post(routes::tokens::tokens_handler))
        .route("/api/admin/requests", get(routes::admin::list_requests))
        .route("/api/admin/requests/{id}", get(routes::admin::get_request))
        .route(
            "/api/admin/requests/{id}/cancel",
            post(routes::admin::cancel_request),
        )
        .route("/api/admin/stats", get(routes::admin::get_stats))
        .route_layer(middleware::from_fn_with_state(
            Arc::clone(&state),
            auth::bearer_auth,
        ))
        .with_state(Arc::clone(&state));

    let app = Router::new()
        .route("/health", get(routes::health::health))
        .route("/llms.txt", get(llms_txt_handler))
        .route("/", get(panel_handler))
        .route("/test", get(test_handler))
        .merge(docs::swagger_router())
        .merge(protected)
        .layer(RequestBodyLimitLayer::new(max_body))
        .layer(TraceLayer::new_for_http())
        .layer(
            CorsLayer::new()
                .allow_origin(Any)
                .allow_methods(Any)
                .allow_headers(Any),
        );

    tracing::info!("监听地址：{bind_addr}");
    let listener = tokio::net::TcpListener::bind(bind_addr)
        .await
        .expect("监听端口失败");

    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await
    .expect("服务器意外退出");
}

async fn panel_handler() -> impl IntoResponse {
    Html(PANEL_HTML)
}

async fn test_handler() -> impl IntoResponse {
    Html(TEST_HTML)
}

async fn llms_txt_handler() -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, "text/plain; charset=utf-8")],
        LLMS_TXT,
    )
}
