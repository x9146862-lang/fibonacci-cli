pub mod concurrent_task_pool;
pub mod utils;

use axum::{
    extract::{rejection::PathRejection, Path, Request},
    http::StatusCode,
    middleware::{self, Next},
    response::{IntoResponse, Json, Response},
    routing::{get, post},
    Router,
};
use concurrent_task_pool::{TaskPool, TaskStatus};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::OnceCell;
use tower_http::cors::CorsLayer;
use tracing::info;
use utoipa::{OpenApi, ToSchema};
use utoipa_swagger_ui::SwaggerUi;

/// 全局任务池：通过 [`init_pool`] 初始化一次，所有请求处理器共享。
static POOL: OnceCell<Arc<TaskPool>> = OnceCell::const_new();

fn pool() -> Arc<TaskPool> {
    POOL.get().expect("TaskPool 未初始化").clone()
}

/// 初始化全局任务池（只应调用一次；重复调用会 panic）。
pub fn init_pool() {
    POOL.set(Arc::new(TaskPool::new()))
        .ok()
        .expect("TaskPool 初始化失败");
}

/// 统一错误响应结构：所有错误接口都返回 `{"error": "..."}` 格式。
#[derive(Debug, Serialize, ToSchema)]
struct ApiError {
    /// 错误描述
    error: String,
}

impl ApiError {
    /// 构造 400 Bad Request 错误
    fn bad_request(msg: impl Into<String>) -> Self {
        Self { error: msg.into() }
    }
}

/// 让 ApiError 可以直接作为处理器返回值：以 400 状态码返回 JSON。
impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (StatusCode::BAD_REQUEST, Json(self)).into_response()
    }
}

/// 将 axum 路径参数解析失败统一转换为 ApiError（400 + JSON）
fn path_param_error<T>(result: Result<Path<T>, PathRejection>) -> Result<T, ApiError> {
    result
        .map(|Path(value)| value)
        .map_err(|_| ApiError::bad_request("路径参数 n 必须是有效的非负整数"))
}

/// `GET /fibonacci/{n}` 的成功响应体。
#[derive(Debug, Serialize, ToSchema)]
struct FibResponse {
    /// 请求的下标
    n: u32,
    /// 第 n 个斐波那契数
    result: u64,
}

/// `GET /health` 的响应体。
#[derive(Debug, Serialize, ToSchema)]
struct HealthResponse {
    /// 服务状态
    status: String,
}

/// POST /task 的请求体。
#[derive(Deserialize, ToSchema)]
struct CreateTaskRequest {
    /// 任务模拟运行时长（秒）。
    duration_secs: u64,
}

/// GET /fibonacci/{n} —— 计算第 n 个斐波那契数并返回 JSON。
#[utoipa::path(
    get,
    path = "/fibonacci/{n}",
    params(
        ("n" = u32, Path, description = "斐波那契数列下标，范围 0~93")
    ),
    responses(
        (status = 200, description = "成功返回第 n 个斐波那契数", body = FibResponse),
        (status = 400, description = "参数非法或超出 u64 范围", body = ApiError)
    )
)]
async fn fibonacci_handler(
    path: Result<Path<u32>, PathRejection>,
) -> Result<Json<FibResponse>, ApiError> {
    let n = path_param_error(path)?;
    info!(path = %format!("/fibonacci/{n}"), "计算斐波那契数");
    let result = utils::fibonacci_checked(n).ok_or_else(|| {
        ApiError::bad_request(format!(
            "n 过大：第 {n} 个斐波那契数超出 u64 范围（最大支持 n = 93）"
        ))
    })?;
    Ok(Json(FibResponse { n, result }))
}

/// GET /fibonacci/sequence/{n} —— 返回前 n 个斐波那契数组成的数组。
#[utoipa::path(
    get,
    path = "/fibonacci/sequence/{n}",
    params(
        ("n" = u32, Path, description = "序列长度，范围 0~94")
    ),
    responses(
        (status = 200, description = "成功返回前 n 个斐波那契数数组", body = Vec<u64>),
        (status = 400, description = "参数非法或超出 u64 范围", body = ApiError)
    )
)]
async fn fibonacci_sequence_handler(
    path: Result<Path<u32>, PathRejection>,
) -> Result<Json<Vec<u64>>, ApiError> {
    let n = path_param_error(path)?;
    info!(path = %format!("/fibonacci/sequence/{n}"), "计算斐波那契序列");
    let sequence = utils::fibonacci_sequence_checked(n).ok_or_else(|| {
        ApiError::bad_request(format!(
            "n 过大：前 {n} 个斐波那契数超出 u64 范围（最大支持 n = 94）"
        ))
    })?;
    Ok(Json(sequence))
}

/// GET /health —— 健康检查。
#[utoipa::path(
    get,
    path = "/health",
    responses(
        (status = 200, description = "服务健康", body = HealthResponse)
    )
)]
async fn health_handler() -> Json<HealthResponse> {
    info!(path = "/health", "健康检查");
    Json(HealthResponse {
        status: "ok".to_string(),
    })
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
    let handle = pool()
        .spawn(async move {
            tokio::time::sleep(Duration::from_secs(duration)).await;
            duration
        })
        .await;
    let task_id = handle.id();
    info!(task_id, duration_secs = duration, "提交模拟任务");
    (
        StatusCode::CREATED,
        Json(serde_json::json!({ "task_id": task_id })),
    )
}

/// GET /task/{id} —— 查询任务状态与结果。
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
        health_handler,
        create_task_handler,
        get_task_handler
    ),
    components(schemas(CreateTaskRequest, FibResponse, HealthResponse, ApiError)),
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

/// 构建应用路由（含 Swagger UI、CORS 与日志中间件），供 `main` 与集成测试复用。
pub fn app() -> Router {
    Router::new()
        .merge(SwaggerUi::new("/swagger-ui").url("/api-docs/openapi.json", ApiDoc::openapi()))
        .route("/fibonacci/{n}", get(fibonacci_handler))
        .route("/fibonacci/sequence/{n}", get(fibonacci_sequence_handler))
        .route("/health", get(health_handler))
        .route("/task", post(create_task_handler))
        .route("/task/{id}", get(get_task_handler))
        // 开发环境允许所有来源的跨域请求（详见 README）
        .layer(CorsLayer::permissive())
        .layer(middleware::from_fn(logging_middleware))
}
