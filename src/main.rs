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
use utoipa::{OpenApi, ToSchema};
use utoipa_swagger_ui::SwaggerUi;

mod concurrent_task_pool;
mod utils;

/// 全局任务池：在 `main` 中初始化一次，所有请求处理器共享。
static POOL: OnceCell<Arc<TaskPool>> = OnceCell::const_new();

fn pool() -> Arc<TaskPool> {
    POOL.get().expect("TaskPool 未初始化").clone()
}

/// POST /task 的请求体。
#[derive(Deserialize, ToSchema)]
struct CreateTaskRequest {
    /// 任务模拟运行时长（秒）。
    duration_secs: u64,
}

/// GET /fibonacci/:n —— 计算第 n 个斐波那契数并返回 JSON。
#[utoipa::path(
    get,
    path = "/fibonacci/{n}",
    params(
        ("n" = u32, Path, description = "斐波那契数列下标，范围 0~93")
    ),
    responses(
        (status = 200, description = "第 n 个斐波那契数"),
        (status = 400, description = "n 超出 u64 范围")
    )
)]
async fn fibonacci_handler(Path(n): Path<u32>) -> (StatusCode, Json<serde_json::Value>) {
    // fib(93) 是 u64 能表示的最大值，更大的 n 返回 400
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

/// GET /fibonacci/sequence/:n —— 返回前 n 个斐波那契数组成的数组。
#[utoipa::path(
    get,
    path = "/fibonacci/sequence/{n}",
    params(
        ("n" = u32, Path, description = "序列长度，范围 0~94")
    ),
    responses(
        (status = 200, description = "前 n 个斐波那契数组成的数组"),
        (status = 400, description = "n 超出 u64 范围")
    )
)]
async fn fibonacci_sequence_handler(Path(n): Path<u32>) -> (StatusCode, Json<serde_json::Value>) {
    // 序列最后一个元素是 fib(n - 1)，fib(93) 是 u64 能表示的最大值，因此 n 最大为 94
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

/// POST /task —— 提交一个模拟任务（sleep duration_secs 秒），返回 task_id。
#[utoipa::path(
    post,
    path = "/task",
    request_body = CreateTaskRequest,
    responses(
        (status = 201, description = "任务已提交，返回 task_id"),
        (status = 400, description = "请求体非法")
    )
)]
async fn create_task_handler(
    Json(payload): Json<CreateTaskRequest>,
) -> (StatusCode, Json<serde_json::Value>) {
    let duration = payload.duration_secs;
    let handle = pool().spawn(async move {
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

/// GET /task/:id —— 查询任务状态与结果。
#[utoipa::path(
    get,
    path = "/task/{id}",
    params(
        ("id" = u64, Path, description = "任务 ID")
    ),
    responses(
        (status = 200, description = "任务状态：running 或 completed（含 result）"),
        (status = 404, description = "任务不存在")
    )
)]
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

/// OpenAPI 文档定义，聚合所有带注解的处理器。
#[derive(OpenApi)]
#[openapi(
    paths(
        fibonacci_handler,
        fibonacci_sequence_handler,
        create_task_handler,
        get_task_handler
    ),
    components(schemas(CreateTaskRequest)),
    info(
        title = "fibonacci-cli API",
        version = "0.1.0",
        description = "斐波那契数列与并发任务池 HTTP 服务"
    )
)]
struct ApiDoc;

/// 日志中间件：记录每个请求的方法、路径、状态码与处理耗时。
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

#[tokio::main]
async fn main() {
    // 初始化 tracing：默认 INFO 级别输出到控制台，可用 RUST_LOG 覆盖
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .init();

    // 初始化全局任务池（只初始化一次）
    POOL.set(Arc::new(TaskPool::new()))
        .ok()
        .expect("TaskPool 初始化失败");

    // 监听地址可通过环境变量 ADDR 覆盖（Docker 容器内需监听 0.0.0.0 才能让 -p 端口映射生效）
    let addr = std::env::var("ADDR").unwrap_or_else(|_| "127.0.0.1:3000".to_string());

    // axum 0.8 使用 {n} 语法声明路径参数（0.7 的 :n 语法在启动时会 panic）
    let app = Router::new()
        .merge(SwaggerUi::new("/swagger-ui").url("/api-docs/openapi.json", ApiDoc::openapi()))
        .route("/fibonacci/{n}", get(fibonacci_handler))
        .route("/fibonacci/sequence/{n}", get(fibonacci_sequence_handler))
        .route("/task", post(create_task_handler))
        .route("/task/{id}", get(get_task_handler))
        .layer(middleware::from_fn(logging_middleware));

    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .expect("端口绑定失败");
    info!(addr = %addr, "fibonacci 服务已启动");

    axum::serve(listener, app).await.expect("服务运行失败");
}
