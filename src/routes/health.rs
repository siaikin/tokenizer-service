use axum::http::StatusCode;
use axum::response::IntoResponse;

/// 健康检查（无需鉴权）。
#[utoipa::path(
    get,
    path = "/health",
    description = "进程存活探测，返回纯文本 `ok`。",
    responses((status = 200, description = "服务正常，body 为 ok"))
)]
pub async fn health() -> impl IntoResponse {
    (StatusCode::OK, "ok")
}
