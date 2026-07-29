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

/// 合并分词请求：一组（或多个）locale → 文本 map，合并为一条 tokens。
#[derive(Debug, Deserialize, ToSchema)]
#[schema(example = json!({
    "texts": [
        { "ja": "ガンダム", "zh-Hans": "高达", "en-US": "Gundam" }
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
    /// 不能为空。多项时仍合并到同一条结果。
    pub texts: Vec<HashMap<String, String>>,
}

/// 合并分词响应。
#[derive(Debug, Serialize, ToSchema)]
#[schema(example = json!({ "tokens": "ガンダム 高达 gundam" }))]
pub struct TokensResponse {
    /// 所有输入分词后小写去重、空格拼接的字符串
    pub tokens: String,
}

/// 批量分词请求：每项独立分词，返回等长 `results`。
#[derive(Debug, Deserialize, ToSchema)]
#[schema(example = json!({
    "items": [
        [{ "ja": "ガンダム", "zh-Hans": "高达" }],
        [{ "en-US": "Mobile Suit", "zh-Hans": "机动战士" }]
    ]
}))]
pub struct TokensBatchRequest {
    /// 每项形态与 `/api/tokens` 的 `texts` 相同。不能为空；最多 500 条。
    pub items: Vec<Vec<HashMap<String, String>>>,
}

/// 批量分词响应。
#[derive(Debug, Serialize, ToSchema)]
#[schema(example = json!({
    "results": ["ガンダム 高达", "mobile suit 机动战士"]
}))]
pub struct TokensBatchResponse {
    /// 与 `items` 等长的 tokens 字符串列表
    pub results: Vec<String>,
}

/// 合并分词：一篇文档的多语言字段 → 一条 tokens 字符串。
///
/// 响应头含 `x-request-id`、`x-duration-ms`。
#[utoipa::path(
    post,
    path = "/api/tokens",
    description = "将 `texts` 中所有 locale 文本分词后合并为一条 tokens。key 选择分词器（ja/ko/zh/latin），value 为空则跳过。",
    request_body = TokensRequest,
    responses(
        (status = 200, description = "分词成功", body = TokensResponse),
        (status = 400, description = "texts 为空等参数错误"),
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
    let _permit = state
        .cpu_sem
        .acquire()
        .await
        .map_err(|_| AppError::Internal("获取并发许可失败".into()))?;

    if cancel.is_cancelled() {
        let _ = db::mark_cancelled(&state.db, id).await;
        state.remove_inflight(&id);
        return Err(AppError::Cancelled);
    }

    db::mark_running(&state.db, id).await?;
    let started = Instant::now();

    let engine = Arc::clone(&state.engine);
    let texts = body.texts;
    let tokens = tokio::task::spawn_blocking(move || engine.tokens_from_texts(&texts))
        .await
        .map_err(|e| AppError::Internal(format!("分词任务失败：{e}")))?;

    if cancel.is_cancelled() {
        let _ = db::mark_cancelled(&state.db, id).await;
        state.remove_inflight(&id);
        return Err(AppError::Cancelled);
    }

    let duration_ms = started.elapsed().as_millis() as i64;
    db::mark_succeeded(&state.db, id, tokens.chars().count(), duration_ms).await?;
    state.remove_inflight(&id);

    Ok(with_meta_headers(
        id,
        duration_ms,
        Json(TokensResponse { tokens }),
    ))
}

/// 批量分词：每项独立结果，适合一次索引多条文档。
///
/// 响应头含 `x-request-id`、`x-duration-ms`。
#[utoipa::path(
    post,
    path = "/api/tokens/batch",
    description = "对 `items` 中每一项单独调用与 `/api/tokens` 相同的合并分词逻辑；`results[i]` 对应 `items[i]`。最多 500 条。",
    request_body = TokensBatchRequest,
    responses(
        (status = 200, description = "分词成功", body = TokensBatchResponse),
        (status = 400, description = "items 为空或超过 500 条"),
        (status = 401, description = "未授权"),
    ),
    security(("bearer_auth" = [])),
    tag = "tokenize"
)]
pub async fn tokens_batch_handler(
    State(state): State<Arc<AppState>>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    Json(body): Json<TokensBatchRequest>,
) -> Result<Response, AppError> {
    if body.items.is_empty() {
        return Err(AppError::BadRequest("items 不能为空".into()));
    }
    if body.items.len() > 500 {
        return Err(AppError::BadRequest("items 最多 500 条".into()));
    }

    let id = Uuid::new_v4();
    let client_ip = Some(addr.ip().to_string());

    let mut text_count = 0usize;
    let mut input_chars = 0i64;
    let mut langs_set = std::collections::BTreeSet::new();
    for item in &body.items {
        let (c, langs, chars) = summarize_texts(item);
        text_count += c;
        input_chars += chars;
        for lang in langs.split(',').filter(|s| !s.is_empty()) {
            langs_set.insert(lang.to_string());
        }
    }
    let langs = langs_set.into_iter().collect::<Vec<_>>().join(",");

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
    let _permit = state
        .cpu_sem
        .acquire()
        .await
        .map_err(|_| AppError::Internal("获取并发许可失败".into()))?;

    if cancel.is_cancelled() {
        let _ = db::mark_cancelled(&state.db, id).await;
        state.remove_inflight(&id);
        return Err(AppError::Cancelled);
    }

    db::mark_running(&state.db, id).await?;
    let started = Instant::now();

    let engine = Arc::clone(&state.engine);
    let items = body.items;
    let results = tokio::task::spawn_blocking(move || {
        items
            .iter()
            .map(|texts| engine.tokens_from_texts(texts))
            .collect::<Vec<_>>()
    })
    .await
    .map_err(|e| AppError::Internal(format!("分词任务失败：{e}")))?;

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
        Json(TokensBatchResponse { results }),
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
