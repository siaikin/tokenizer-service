use axum::http::StatusCode;
use axum::response::IntoResponse;

#[utoipa::path(
    get,
    path = "/health",
    responses((status = 200, description = "服务正常"))
)]
pub async fn health() -> impl IntoResponse {
    (StatusCode::OK, "ok")
}
