use utoipa::{
    openapi::security::{HttpAuthScheme, HttpBuilder, SecurityScheme},
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

const API_DESCRIPTION: &str = r#"
CJK 分词 HTTP 服务（ja / ko / zh / latin），输出适合 Meilisearch 索引的 tokens 字符串。

## 鉴权

点击右上角 **Authorize**，填入环境变量 `TOKENIZER_TOKEN` 的值（无需手写 `Bearer ` 前缀）。

除 `/health` 外，分词与管理接口均需：

```
Authorization: Bearer <TOKENIZER_TOKEN>
```

## Locale key（有意义）

请求体中 localized text 对象的 **key 决定分词器**，不会出现在返回结果中：

| key 规则 | 分词器 | 示例 |
|---------|--------|------|
| `ja` / `ja-*` | 日文（Lindera） | `ja`, `ja-JP` |
| `ko` / `ko-*` | 韩文（Lindera） | `ko`, `ko-KR` |
| `zh*` / `cmn` | 中文（jieba） | `zh`, `zh-Hans`, `zh-Hant` |
| 其它 | Latin（按非字母切分） | `en`, `en-US` |

同一对象可写多种语言；所有 value 分词后**小写去重**，再空格拼接。空字符串 value 会被跳过。

## 接口概览

- `POST /api/tokens` — 合并分词（一篇文档的多语言字段 → 一条 tokens）
- `POST /api/tokens/batch` — 批量分词（每项独立结果，最多 500 条）
- `GET /api/admin/*` — 请求日志与统计
"#;

struct BearerSecurityAddon;
impl Modify for BearerSecurityAddon {
    fn modify(&self, openapi: &mut utoipa::openapi::OpenApi) {
        if let Some(components) = openapi.components.as_mut() {
            components.add_security_scheme(
                "bearer_auth",
                SecurityScheme::Http(
                    HttpBuilder::new()
                        .scheme(HttpAuthScheme::Bearer)
                        .bearer_format("TOKENIZER_TOKEN")
                        .description(Some(
                            "填入环境变量 TOKENIZER_TOKEN 的值（Authorize 时不要加 Bearer 前缀）"
                                .to_string(),
                        ))
                        .build(),
                ),
            );
        }
    }
}

#[derive(OpenApi)]
#[openapi(
    info(
        title = "Tokenizer Service",
        version = "0.1.0",
        description = API_DESCRIPTION
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
        (name = "tokenize", description = "分词接口。Locale key 选择分词器，返回小写去重后的空格拼接 tokens。"),
        (name = "admin", description = "管理与统计接口（请求日志、取消进行中任务、聚合统计）。"),
    )
)]
pub struct ApiDoc;

pub fn swagger_router() -> axum::Router {
    SwaggerUi::new("/swagger-ui")
        .url("/api-docs/openapi.json", ApiDoc::openapi())
        .into()
}
