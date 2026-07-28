use utoipa::{
    openapi::security::{Http, HttpAuthScheme, SecurityScheme},
    Modify, OpenApi,
};
use utoipa_swagger_ui::SwaggerUi;

use crate::routes::admin::{
    __path_cancel_request, __path_get_request, __path_get_stats, __path_list_requests,
};
use crate::routes::health::__path_health;
use crate::routes::tokens::{__path_tokens_batch_handler, __path_tokens_handler};

use crate::{
    db::{RequestRow, RequestStatus, StatsRow},
    routes::admin::{CancelResponse, ListResponse},
    routes::tokens::{
        TokensBatchRequest, TokensBatchResponse, TokensRequest, TokensResponse,
    },
};

struct BearerSecurityAddon;
impl Modify for BearerSecurityAddon {
    fn modify(&self, openapi: &mut utoipa::openapi::OpenApi) {
        if let Some(components) = openapi.components.as_mut() {
            components.add_security_scheme(
                "bearer_auth",
                SecurityScheme::Http(Http::new(HttpAuthScheme::Bearer)),
            );
        }
    }
}

#[derive(OpenApi)]
#[openapi(
    info(
        title = "Tokenizer Service",
        version = "0.1.0",
        description = "CJK 分词服务：ja/ko/zh/latin → Meilisearch tokens 字符串"
    ),
    paths(
        health,
        tokens_handler,
        tokens_batch_handler,
        list_requests,
        get_request,
        cancel_request,
        get_stats,
    ),
    components(schemas(
        TokensRequest,
        TokensResponse,
        TokensBatchRequest,
        TokensBatchResponse,
        RequestRow,
        RequestStatus,
        StatsRow,
        ListResponse,
        CancelResponse,
    )),
    modifiers(&BearerSecurityAddon),
    tags(
        (name = "tokenize", description = "分词接口"),
        (name = "admin", description = "管理与统计接口"),
    )
)]
pub struct ApiDoc;

pub fn swagger_router() -> axum::Router {
    SwaggerUi::new("/swagger-ui")
        .url("/api-docs/openapi.json", ApiDoc::openapi())
        .into()
}
