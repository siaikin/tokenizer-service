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

- 健康检查：`GET /health`
- 控制面板：`http://localhost:8080/`
- 分词测试：`http://localhost:8080/test`
- Swagger：`http://localhost:8080/swagger-ui`

```bash
curl -s http://localhost:8080/api/tokens \
  -H "Authorization: Bearer $TOKENIZER_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{"texts":[{"ja":"ガンダム","zh-Hans":"高达","en-US":"Gundam"}]}'
```

## 环境变量

| 变量 | 默认 | 说明 |
|------|------|------|
| `TOKENIZER_TOKEN` | **必填** | Bearer token |
| `TOKENIZER_BIND` | `0.0.0.0:8080` | 监听地址 |
| `TOKENIZER_DB_PATH` | `/data/tokenizer.db` | SQLite |
| `TOKENIZER_MAX_CONCURRENCY` | CPU 核数 | 并发闸门 |
| `TOKENIZER_MAX_BODY_MB` | `8` | 请求体上限 |

## Docker

```yaml
services:
  tokenizer:
    image: siaikin/tokenizer-service:latest
    ports: ["8080:8080"]
    environment:
      TOKENIZER_TOKEN: your-secret-token
    volumes: [tokenizer_data:/data]
    restart: unless-stopped
volumes:
  tokenizer_data:
```

## API

- `POST /api/tokens` — `{ "texts": [ { "ja": "...", "zh-Hans": "..." } ] }` → `{ "tokens": "..." }`
- `POST /api/tokens/batch` — `{ "items": [ texts[], ... ] }` → `{ "results": [ "...", ... ] }`
- `GET /api/admin/stats` / `requests` — 需 Bearer
