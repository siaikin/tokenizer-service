use std::{collections::HashMap, sync::Arc, time::Instant};

use axum::{
    extract::{ConnectInfo, State},
    http::HeaderValue,
    response::{IntoResponse, Response},
    Json,
};
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::{
    db,
    error::AppError,
    state::AppState,
    tokenize::summarize_texts,
};

/// 一对一分词请求：`texts` 每项独立分词。
#[derive(Debug, Deserialize, ToSchema)]
#[schema(example = json!({
    "texts": [
        { "ja": "ガンダム", "zh-Hans": "高达" },
        { "en-US": "Mobile Suit" }
    ]
}))]
pub struct TokensRequest {
    /// 一个或多个 localized text 对象。key 为 locale（决定分词器），value 为文本。
    ///
    /// - `ja` / `ja-*` → 日文
    /// - `ko` / `ko-*` → 韩文
    /// - `zh*` / `cmn` → 中文
    /// - 其它 → Latin
    ///
    /// 不能为空；最多 500 条。每项独立分词，与 `results` 一一对应。
    pub texts: Vec<HashMap<String, String>>,
}

/// 一对一分词响应。
#[derive(Debug, Serialize, ToSchema)]
#[schema(example = json!({
    "results": ["ガンダム 高达", "mobile suit"]
}))]
pub struct TokensResponse {
    /// 与 `texts` 等长的 tokens 字符串列表（每项内小写去重、空格拼接）
    pub results: Vec<String>,
}

/// 一对一分词：`texts[i]` → `results[i]`。
///
/// 响应头含 `x-request-id`、`x-duration-ms`。
#[utoipa::path(
    post,
    path = "/api/tokens",
    description = "对 `texts` 中每一项独立分词，返回等长 `results`。key 选择分词器（ja/ko/zh/latin），value 为空则跳过。最多 500 条。",
    request_body = TokensRequest,
    responses(
        (status = 200, description = "分词成功", body = TokensResponse),
        (status = 400, description = "texts 为空或超过 500 条"),
        (status = 401, description = "未授权"),
    ),
    security(("bearer_auth" = [])),
    tag = "tokenize"
)]
pub async fn tokens_handler(
    State(state): State<Arc<AppState>>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    Json(body): Json<TokensRequest>,
) -> Result<Response, AppError> {
    if body.texts.is_empty() {
        return Err(AppError::BadRequest("texts 不能为空".into()));
    }
    if body.texts.len() > 500 {
        return Err(AppError::BadRequest("texts 最多 500 条".into()));
    }

    let id = Uuid::new_v4();
    let client_ip = Some(addr.ip().to_string());
    let (text_count, langs, input_chars) = summarize_texts(&body.texts);

    db::insert_request(
        &state.db,
        id,
        client_ip.as_deref(),
        text_count,
        input_chars,
        &langs,
    )
    .await?;

    let cancel = state.register_inflight(id);
    let _permit = match state.cpu_sem.acquire().await {
        Ok(p) => p,
        Err(_) => {
            let msg = "获取并发许可失败";
            let _ = db::mark_failed(&state.db, id, msg).await;
            state.remove_inflight(&id);
            return Err(AppError::Internal(msg.into()));
        }
    };

    if cancel.is_cancelled() {
        let _ = db::mark_cancelled(&state.db, id).await;
        state.remove_inflight(&id);
        return Err(AppError::Cancelled);
    }

    db::mark_running(&state.db, id).await?;
    let started = Instant::now();

    let engine = Arc::clone(&state.engine);
    let texts = body.texts;
    let results = match tokio::task::spawn_blocking(move || {
        texts
            .iter()
            .map(|m| engine.tokens_from_map(m))
            .collect::<Vec<_>>()
    })
    .await
    {
        Ok(r) => r,
        Err(e) => {
            let msg = format!("分词任务失败：{e}");
            let _ = db::mark_failed(&state.db, id, &msg).await;
            state.remove_inflight(&id);
            return Err(AppError::Internal(msg));
        }
    };

    if cancel.is_cancelled() {
        let _ = db::mark_cancelled(&state.db, id).await;
        state.remove_inflight(&id);
        return Err(AppError::Cancelled);
    }

    let duration_ms = started.elapsed().as_millis() as i64;
    let output_chars: usize = results.iter().map(|s| s.chars().count()).sum();
    db::mark_succeeded(&state.db, id, output_chars, duration_ms).await?;
    state.remove_inflight(&id);

    Ok(with_meta_headers(
        id,
        duration_ms,
        Json(TokensResponse { results }),
    ))
}

fn with_meta_headers(id: Uuid, duration_ms: i64, body: impl IntoResponse) -> Response {
    let mut res = body.into_response();
    let headers = res.headers_mut();
    if let Ok(v) = HeaderValue::from_str(&id.to_string()) {
        headers.insert("x-request-id", v);
    }
    if let Ok(v) = HeaderValue::from_str(&duration_ms.to_string()) {
        headers.insert("x-duration-ms", v);
    }
    res
}
