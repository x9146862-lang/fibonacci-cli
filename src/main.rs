use axum::{
    extract::Path,
    http::StatusCode,
    response::Json,
    routing::get,
    Router,
};
use std::env;

mod utils;

/// GET /fibonacci/:n —— 计算第 n 个斐波那契数并返回 JSON
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

/// GET /fibonacci/sequence/:n —— 返回前 n 个斐波那契数组成的数组
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

#[tokio::main]
async fn main() {
    // 监听地址可通过环境变量 ADDR 覆盖（Docker 容器内需监听 0.0.0.0 才能让 -p 端口映射生效）
    let addr = env::var("ADDR").unwrap_or_else(|_| "127.0.0.1:3000".to_string());

    // 注意：axum 0.8 使用 {n} 语法声明路径参数（0.7 的 :n 语法在启动时会 panic）
    let app = Router::new()
        .route("/fibonacci/{n}", get(fibonacci_handler))
        .route("/fibonacci/sequence/{n}", get(fibonacci_sequence_handler));

    let listener = tokio::net::TcpListener::bind(&addr).await.expect("端口绑定失败");
    println!("fibonacci 服务已启动: http://{addr}");

    axum::serve(listener, app).await.expect("服务运行失败");
}
