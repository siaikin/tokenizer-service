use chrono::Utc;
use serde::{Deserialize, Serialize};
use sqlx::{sqlite::SqliteConnectOptions, FromRow, SqlitePool};
use std::path::Path;
use std::str::FromStr;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::error::AppError;

pub async fn init_db(db_path: &str) -> Result<SqlitePool, AppError> {
    if db_path != ":memory:" {
        if let Some(parent) = Path::new(db_path).parent() {
            if !parent.as_os_str().is_empty() {
                tokio::fs::create_dir_all(parent)
                    .await
                    .map_err(|e| AppError::Internal(format!("DB 目录创建失败：{e}")))?;
            }
        }
    }

    let opts = SqliteConnectOptions::from_str(db_path)
        .map_err(|e| AppError::Internal(format!("DB 路径解析失败：{e}")))?
        .create_if_missing(true)
        .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal)
        .foreign_keys(true);

    let pool = SqlitePool::connect_with(opts)
        .await
        .map_err(|e| AppError::Internal(format!("DB 连接失败：{e}")))?;

    sqlx::query(CREATE_TABLE_SQL)
        .execute(&pool)
        .await
        .map_err(|e| AppError::Internal(format!("建表失败：{e}")))?;

    tracing::info!(db_path, "数据库已就绪");
    Ok(pool)
}

const CREATE_TABLE_SQL: &str = r#"
CREATE TABLE IF NOT EXISTS requests (
    id              TEXT PRIMARY KEY,
    created_at      TEXT NOT NULL,
    started_at      TEXT,
    finished_at     TEXT,
    status          TEXT NOT NULL,
    client_ip       TEXT,
    text_count      INTEGER,
    input_chars     INTEGER,
    output_chars    INTEGER,
    langs           TEXT,
    duration_ms     INTEGER,
    error           TEXT
)
"#;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum RequestStatus {
    Queued,
    Running,
    Succeeded,
    Failed,
    Cancelled,
}

#[derive(Debug, Serialize, Deserialize, ToSchema, FromRow)]
pub struct RequestRow {
    pub id: String,
    pub created_at: String,
    pub started_at: Option<String>,
    pub finished_at: Option<String>,
    pub status: String,
    pub client_ip: Option<String>,
    pub text_count: Option<i64>,
    pub input_chars: Option<i64>,
    pub output_chars: Option<i64>,
    pub langs: Option<String>,
    pub duration_ms: Option<i64>,
    pub error: Option<String>,
}

pub async fn insert_request(
    pool: &SqlitePool,
    id: Uuid,
    client_ip: Option<&str>,
    text_count: usize,
    input_chars: i64,
    langs: &str,
) -> Result<(), AppError> {
    let id_str = id.to_string();
    let now = Utc::now().to_rfc3339();
    sqlx::query(
        r#"INSERT INTO requests
           (id, created_at, status, client_ip, text_count, input_chars, langs)
           VALUES (?, ?, 'queued', ?, ?, ?, ?)"#,
    )
    .bind(&id_str)
    .bind(&now)
    .bind(client_ip)
    .bind(text_count as i64)
    .bind(input_chars)
    .bind(langs)
    .execute(pool)
    .await
    .map_err(AppError::from)?;
    Ok(())
}

pub async fn mark_running(pool: &SqlitePool, id: Uuid) -> Result<(), AppError> {
    let now = Utc::now().to_rfc3339();
    sqlx::query("UPDATE requests SET status = 'running', started_at = ? WHERE id = ?")
        .bind(&now)
        .bind(id.to_string())
        .execute(pool)
        .await
        .map_err(AppError::from)?;
    Ok(())
}

pub async fn mark_succeeded(
    pool: &SqlitePool,
    id: Uuid,
    output_chars: usize,
    duration_ms: i64,
) -> Result<(), AppError> {
    let now = Utc::now().to_rfc3339();
    sqlx::query(
        "UPDATE requests SET status = 'succeeded', finished_at = ?, output_chars = ?, duration_ms = ? WHERE id = ?",
    )
    .bind(&now)
    .bind(output_chars as i64)
    .bind(duration_ms)
    .bind(id.to_string())
    .execute(pool)
    .await
    .map_err(AppError::from)?;
    Ok(())
}

pub async fn mark_failed(pool: &SqlitePool, id: Uuid, error: &str) -> Result<(), AppError> {
    let now = Utc::now().to_rfc3339();
    sqlx::query("UPDATE requests SET status = 'failed', finished_at = ?, error = ? WHERE id = ?")
        .bind(&now)
        .bind(error)
        .bind(id.to_string())
        .execute(pool)
        .await
        .map_err(AppError::from)?;
    Ok(())
}

pub async fn mark_cancelled(pool: &SqlitePool, id: Uuid) -> Result<(), AppError> {
    let now = Utc::now().to_rfc3339();
    sqlx::query("UPDATE requests SET status = 'cancelled', finished_at = ? WHERE id = ?")
        .bind(&now)
        .bind(id.to_string())
        .execute(pool)
        .await
        .map_err(AppError::from)?;
    Ok(())
}

const SELECT: &str = r#"SELECT id, created_at, started_at, finished_at, status, client_ip,
                      text_count, input_chars, output_chars, langs, duration_ms, error
               FROM requests"#;

pub async fn list_requests(
    pool: &SqlitePool,
    status: Option<&str>,
    limit: i64,
    offset: i64,
) -> Result<Vec<RequestRow>, AppError> {
    let rows = if let Some(s) = status {
        sqlx::query_as::<_, RequestRow>(&format!(
            "{SELECT} WHERE status = ? ORDER BY created_at DESC LIMIT ? OFFSET ?"
        ))
        .bind(s)
        .bind(limit)
        .bind(offset)
        .fetch_all(pool)
        .await
    } else {
        sqlx::query_as::<_, RequestRow>(&format!(
            "{SELECT} ORDER BY created_at DESC LIMIT ? OFFSET ?"
        ))
        .bind(limit)
        .bind(offset)
        .fetch_all(pool)
        .await
    }
    .map_err(AppError::from)?;
    Ok(rows)
}

pub async fn get_request(pool: &SqlitePool, id: &str) -> Result<Option<RequestRow>, AppError> {
    sqlx::query_as::<_, RequestRow>(&format!("{SELECT} WHERE id = ?"))
        .bind(id)
        .fetch_optional(pool)
        .await
        .map_err(AppError::from)
}

#[derive(Debug, Serialize, Deserialize, ToSchema, FromRow)]
pub struct StatsRow {
    pub total: i64,
    pub queued: i64,
    pub running: i64,
    pub succeeded: i64,
    pub failed: i64,
    pub cancelled: i64,
    pub avg_duration_ms: Option<f64>,
    pub total_output_chars: Option<i64>,
}

pub async fn get_stats(pool: &SqlitePool) -> Result<StatsRow, AppError> {
    sqlx::query_as::<_, StatsRow>(
        r#"SELECT
            COUNT(*)                                                         AS total,
            SUM(CASE WHEN status='queued'    THEN 1 ELSE 0 END)             AS queued,
            SUM(CASE WHEN status='running'   THEN 1 ELSE 0 END)             AS running,
            SUM(CASE WHEN status='succeeded' THEN 1 ELSE 0 END)             AS succeeded,
            SUM(CASE WHEN status='failed'    THEN 1 ELSE 0 END)             AS failed,
            SUM(CASE WHEN status='cancelled' THEN 1 ELSE 0 END)             AS cancelled,
            AVG(CASE WHEN status='succeeded' THEN CAST(duration_ms AS REAL) END) AS avg_duration_ms,
            SUM(CASE WHEN status='succeeded' THEN output_chars ELSE 0 END)  AS total_output_chars
           FROM requests"#,
    )
    .fetch_one(pool)
    .await
    .map_err(AppError::from)
}

pub async fn count_requests(pool: &SqlitePool, status: Option<&str>) -> Result<i64, AppError> {
    let count: i64 = if let Some(s) = status {
        sqlx::query_scalar("SELECT COUNT(*) FROM requests WHERE status = ?")
            .bind(s)
            .fetch_one(pool)
            .await
    } else {
        sqlx::query_scalar("SELECT COUNT(*) FROM requests")
            .fetch_one(pool)
            .await
    }
    .map_err(AppError::from)?;
    Ok(count)
}
