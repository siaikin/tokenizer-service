/// 服务配置，全部字段来自环境变量。
use std::num::NonZeroUsize;

#[derive(Debug, Clone)]
pub struct Config {
    /// Bearer 鉴权 Token（必填）
    pub token: String,
    /// 监听地址，默认 0.0.0.0:8080
    pub bind: String,
    /// SQLite 数据库文件路径，默认 /data/tokenizer.db
    pub db_path: String,
    /// 最大同时处理请求数，默认 = CPU 逻辑核数
    pub max_concurrency: usize,
    /// 请求体最大 MB，默认 8
    pub max_body_mb: usize,
}

impl Config {
    pub fn from_env() -> Self {
        let token = std::env::var("TOKENIZER_TOKEN")
            .expect("TOKENIZER_TOKEN 未设置，服务拒绝启动（无 token 无法鉴权）");

        let bind = std::env::var("TOKENIZER_BIND")
            .unwrap_or_else(|_| "0.0.0.0:8080".to_string());

        let db_path = std::env::var("TOKENIZER_DB_PATH")
            .unwrap_or_else(|_| "/data/tokenizer.db".to_string());

        let max_concurrency = std::env::var("TOKENIZER_MAX_CONCURRENCY")
            .ok()
            .and_then(|v| v.parse::<usize>().ok())
            .filter(|&n| n > 0)
            .unwrap_or_else(|| {
                std::thread::available_parallelism()
                    .map(NonZeroUsize::get)
                    .unwrap_or(4)
            });

        let max_body_mb: usize = std::env::var("TOKENIZER_MAX_BODY_MB")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(8);

        tracing::info!(bind, db_path, max_concurrency, max_body_mb, "配置已加载");

        Self {
            token,
            bind,
            db_path,
            max_concurrency,
            max_body_mb,
        }
    }
}
