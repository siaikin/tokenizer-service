# Tokenizer Service

CJK 分词 HTTP 服务（ja / ko / zh / latin），供 Jaburo admin Meilisearch 同步使用。

对齐 `hdrify-service`：Bearer 鉴权、状态面板、请求日志、Swagger。

## 快速开始

```bash
cp .env.example .env
# 编辑 TOKENIZER_TOKEN

# Lindera 嵌入词典：build.rs 的 reqwest 拉包常失败，先用脚本预下载
./scripts/fetch-lindera-dicts.sh
export LINDERA_CACHE="$PWD/.lindera-cache"

mkdir -p data
export $(grep -v '^#' .env | xargs)
cargo run
```

启动后可用页面：

| 地址 | 说明 |
|------|------|
| `GET /health` | 健康检查（无需鉴权） |
| `http://localhost:8080/` | 控制面板 |
| `http://localhost:8080/test` | 分词测试页 |
| `http://localhost:8080/swagger-ui` | Swagger UI |
| `http://localhost:8080/api-docs/openapi.json` | OpenAPI JSON |

## Swagger

1. 打开 [http://localhost:8080/swagger-ui](http://localhost:8080/swagger-ui)
2. 点击右上角 **Authorize**，填入 `TOKENIZER_TOKEN` 的值（无需手写 `Bearer ` 前缀，Swagger 会自动加）
3. 在 **tokenize** 分组试 `POST /api/tokens`，或在 **admin** 分组查看请求日志 / 统计

分词与管理接口均需 Bearer；`/health`、面板、`/test`、Swagger 页面本身不需要。

## 鉴权

除公开页面外，请求头：

```http
Authorization: Bearer <TOKENIZER_TOKEN>
```

未带或错误 token 返回 `401`。

## 使用方法

### Locale key（有意义）

`texts` 里每个对象是 locale → 文本。**key 决定用哪种分词器**，不会出现在返回结果中：

| key 规则 | 分词器 | 示例 |
|---------|--------|------|
| `ja` / `ja-*` | 日文（Lindera） | `ja`, `ja-JP` |
| `ko` / `ko-*` | 韩文（Lindera） | `ko`, `ko-KR` |
| `zh*` / `cmn` | 中文（jieba） | `zh`, `zh-Hans`, `zh-Hant` |
| 其它 | Latin（按非字母切分） | `en`, `en-US`, `fr` |

同一对象可写多种语言；所有 value 分词后**小写去重**，再空格拼接成一个字符串。空字符串 value 会被跳过。

### `POST /api/tokens` — 合并分词

将一组（或多个）localized text 合并成一条 tokens 字符串，适合一篇文档的多语言字段。

**请求**

```json
{
  "texts": [
    { "ja": "ガンダム", "zh-Hans": "高达", "en-US": "Gundam" }
  ]
}
```

`texts` 不能为空。也可传多项，例如标题、简介各一个 map，tokens 仍合并到同一结果。

**响应**

```json
{
  "tokens": "ガンダム 高达 gundam"
}
```

**curl**

```bash
curl -s http://localhost:8080/api/tokens \
  -H "Authorization: Bearer $TOKENIZER_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{"texts":[{"ja":"ガンダム","zh-Hans":"高达","en-US":"Gundam"}]}'
```

### `POST /api/tokens/batch` — 批量分词

每项独立分词，返回与 `items` 等长的 `results`，适合一次索引多条文档。

**请求**

```json
{
  "items": [
    [{ "ja": "ガンダム", "zh-Hans": "高达" }],
    [{ "en-US": "Mobile Suit", "zh-Hans": "机动战士" }]
  ]
}
```

每项形态与 `/api/tokens` 的 `texts` 相同（`Vec` of locale maps）。

**响应**

```json
{
  "results": [
    "ガンダム 高达",
    "mobile suit 机动战士"
  ]
}
```

**curl**

```bash
curl -s http://localhost:8080/api/tokens/batch \
  -H "Authorization: Bearer $TOKENIZER_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{"items":[[{"ja":"ガンダム","zh-Hans":"高达"}],[{"en-US":"Mobile Suit","zh-Hans":"机动战士"}]]}'
```

### 管理接口

均需 Bearer。

| 方法 | 路径 | 说明 |
|------|------|------|
| `GET` | `/api/admin/stats` | 聚合统计 |
| `GET` | `/api/admin/requests` | 请求日志列表（`?status=&limit=20&offset=0`，`limit` 最大 100） |
| `GET` | `/api/admin/requests/{id}` | 单条请求详情 |
| `POST` | `/api/admin/requests/{id}/cancel` | 取消进行中的请求 |

```bash
curl -s http://localhost:8080/api/admin/stats \
  -H "Authorization: Bearer $TOKENIZER_TOKEN"

curl -s "http://localhost:8080/api/admin/requests?limit=20" \
  -H "Authorization: Bearer $TOKENIZER_TOKEN"
```

完整 schema、试调与示例以 Swagger UI 为准。

## 环境变量

| 变量 | 默认 | 说明 |
|------|------|------|
| `TOKENIZER_TOKEN` | **必填** | Bearer token |
| `TOKENIZER_BIND` | `0.0.0.0:8080` | 监听地址 |
| `TOKENIZER_DB_PATH` | `/data/tokenizer.db` | SQLite 路径，存请求日志与统计（非分词词典）；本地可用 `./data/tokenizer.db` |
| `TOKENIZER_MAX_CONCURRENCY` | CPU 核数 | 并发闸门 |
| `TOKENIZER_MAX_BODY_MB` | `8` | 请求体上限 |

## Docker

SQLite 默认写在 `/data/tokenizer.db`，需挂载 volume，否则容器重建后请求日志与统计会丢失。

```yaml
services:
  tokenizer:
    image: siaikin/tokenizer-service:latest
    ports: ["8080:8080"]
    environment:
      TOKENIZER_TOKEN: your-secret-token
      TOKENIZER_DB_PATH: /data/tokenizer.db
    volumes:
      - tokenizer_data:/data
    restart: unless-stopped

volumes:
  tokenizer_data:
```

本地目录挂载示例：

```yaml
volumes:
  - ./data:/data
```
