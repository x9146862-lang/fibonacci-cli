pub mod concurrent_task_pool;
pub mod utils;

use axum::{
    extract::{Path, Request},
    http::StatusCode,
    middleware::{self, Next},
    response::{Json, Response},
    routing::{get, post},
    Router,
};
use concurrent_task_pool::{TaskPool, TaskStatus};
use serde::Deserialize;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::OnceCell;
use tracing::info;

static POOL: OnceCell<Arc<TaskPool>> = OnceCell::const_new();

fn pool() -> Arc<TaskPool> {
    POOL.get().expect("TaskPool 未初始化").clone()
}

pub fn init_pool() {
    POOL.set(Arc::new(TaskPool::new()))
        .ok()
        .expect("TaskPool 初始化失败");
}

#[derive(Deserialize)]
pub struct CreateTaskRequest {
    pub duration_secs: u64,
}

async fn fibonacci_handler(Path(n): Path<u32>) -> (StatusCode, Json<serde_json::Value>) {
    if n > 93 {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "error": format!("n 过大：第 {n} 个斐波那契数超出 u64 范围（最大支持 n = 93）")
            })),
        );
    }
    (
        StatusCode::OK,
        Json(serde_json::json!({
            "n": n,
            "result": utils::fibonacci(n)
        })),
    )
}

async fn fibonacci_sequence_handler(Path(n): Path<u32>) -> (StatusCode, Json<serde_json::Value>) {
    if n > 94 {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "error": format!("n 过大：前 {n} 个斐波那契数超出 u64 范围（最大支持 n = 94）")
            })),
        );
    }
    (
        StatusCode::OK,
        Json(serde_json::json!(utils::fibonacci_sequence(n))),
    )
}

async fn create_task_handler(
    Json(payload): Json<CreateTaskRequest>,
) -> (StatusCode, Json<serde_json::Value>) {
    let pool = pool();
    let duration = payload.duration_secs;
    let handle = pool.spawn(async move {
        tokio::time::sleep(Duration::from_secs(duration)).await;
        duration
    });
    let task_id = handle.id();
    info!(task_id, duration_secs = duration, "提交模拟任务");
    (
        StatusCode::CREATED,
        Json(serde_json::json!({ "task_id": task_id })),
    )
}

async fn get_task_handler(Path(id): Path<u64>) -> (StatusCode, Json<serde_json::Value>) {
    let pool = pool();
    match pool.status(id) {
        None => {
            info!(task_id = id, "查询任务：不存在");
            (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({ "status": "not_found" })),
            )
        }
        Some(TaskStatus::Running) => {
            info!(task_id = id, "查询任务：运行中");
            (
                StatusCode::OK,
                Json(serde_json::json!({ "status": "running" })),
            )
        }
        Some(TaskStatus::Completed) => {
            let result = pool.result::<u64>(id).map(|r| *r);
            info!(task_id = id, ?result, "查询任务：已完成");
            (
                StatusCode::OK,
                Json(serde_json::json!({ "status": "completed", "result": result })),
            )
        }
    }
}

async fn logging_middleware(req: Request, next: Next) -> Response {
    let method = req.method().clone();
    let path = req.uri().path().to_string();
    let start = Instant::now();
    let response = next.run(req).await;
    let elapsed = start.elapsed();
    info!(
        method = %method,
        path = %path,
        status = response.status().as_u16(),
        elapsed_ms = elapsed.as_millis() as u64,
        "HTTP 请求处理完成"
    );
    response
}

pub fn app() -> Router {
    Router::new()
        .route("/fibonacci/{n}", get(fibonacci_handler))
        .route("/fibonacci/sequence/{n}", get(fibonacci_sequence_handler))
        .route("/task", post(create_task_handler))
        .route("/task/{id}", get(get_task_handler))
        .route("/health", get(|| async { (StatusCode::OK, Json(serde_json::json!({"status":"ok"}))) }))
        .layer(middleware::from_fn(logging_middleware))
}
