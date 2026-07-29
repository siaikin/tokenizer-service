use std::sync::Arc;

use axum::{
    extract::{Path, Query, State},
    Json,
};
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

use crate::{
    db::{self, RequestRow, StatsRow},
    error::AppError,
    state::AppState,
};

#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct ListQuery {
    /// 按状态过滤：`queued` / `running` / `succeeded` / `failed` / `cancelled`
    pub status: Option<String>,
    /// 每页条数，默认 20，范围 1–100
    #[serde(default = "default_limit")]
    pub limit: i64,
    /// 偏移量，默认 0
    #[serde(default)]
    pub offset: i64,
}

fn default_limit() -> i64 {
    20
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ListResponse {
    pub total: i64,
    pub limit: i64,
    pub offset: i64,
    pub items: Vec<RequestRow>,
}

/// 分页列出分词请求日志。
#[utoipa::path(
    get,
    path = "/api/admin/requests",
    description = "按创建时间倒序列出历史请求；可用 `status` 过滤，`limit` 最大 100。",
    params(ListQuery),
    responses((status = 200, description = "列表", body = ListResponse), (status = 401, description = "未授权")),
    security(("bearer_auth" = [])),
    tag = "admin"
)]
pub async fn list_requests(
    State(state): State<Arc<AppState>>,
    Query(q): Query<ListQuery>,
) -> Result<Json<ListResponse>, AppError> {
    let limit = q.limit.min(100).max(1);
    let offset = q.offset.max(0);
    let total = db::count_requests(&state.db, q.status.as_deref()).await?;
    let items = db::list_requests(&state.db, q.status.as_deref(), limit, offset).await?;
    Ok(Json(ListResponse {
        total,
        limit,
        offset,
        items,
    }))
}

/// 按 UUID 查询单条请求详情。
#[utoipa::path(
    get,
    path = "/api/admin/requests/{id}",
    description = "返回单条请求的状态、语种、输入输出字符数、耗时等。",
    params(("id" = String, Path, description = "请求 UUID（响应头 x-request-id）")),
    responses(
        (status = 200, description = "请求详情", body = RequestRow),
        (status = 401, description = "未授权"),
        (status = 404, description = "请求不存在"),
    ),
    security(("bearer_auth" = [])),
    tag = "admin"
)]
pub async fn get_request(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<RequestRow>, AppError> {
    let row = db::get_request(&state.db, &id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("请求 {id} 不存在")))?;
    Ok(Json(row))
}

#[derive(Debug, Serialize, ToSchema)]
pub struct CancelResponse {
    pub cancelled: bool,
    pub message: String,
}

/// 取消进行中的分词请求。
#[utoipa::path(
    post,
    path = "/api/admin/requests/{id}/cancel",
    description = "向 inflight 请求发送取消信号。若请求已结束，`cancelled` 为 false。",
    params(("id" = String, Path, description = "请求 UUID")),
    responses(
        (status = 200, description = "取消结果", body = CancelResponse),
        (status = 400, description = "id 不是有效 UUID"),
        (status = 401, description = "未授权"),
        (status = 404, description = "请求不存在"),
    ),
    security(("bearer_auth" = [])),
    tag = "admin"
)]
pub async fn cancel_request(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<CancelResponse>, AppError> {
    let _row = db::get_request(&state.db, &id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("请求 {id} 不存在")))?;

    let id_uuid = id
        .parse::<uuid::Uuid>()
        .map_err(|_| AppError::BadRequest("id 格式不是有效 UUID".into()))?;

    let cancelled = state.cancel_inflight(&id_uuid);
    let message = if cancelled {
        "取消信号已发送"
    } else {
        "该请求不在 inflight 中（可能已完成）"
    }
    .to_string();

    Ok(Json(CancelResponse { cancelled, message }))
}

/// 聚合统计（各状态计数等）。
#[utoipa::path(
    get,
    path = "/api/admin/stats",
    description = "返回请求总量及按状态聚合的统计信息。",
    responses((status = 200, description = "统计", body = StatsRow), (status = 401, description = "未授权")),
    security(("bearer_auth" = [])),
    tag = "admin"
)]
pub async fn get_stats(State(state): State<Arc<AppState>>) -> Result<Json<StatsRow>, AppError> {
    Ok(Json(db::get_stats(&state.db).await?))
}
