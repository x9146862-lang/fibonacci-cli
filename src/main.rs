use axum::{
    extract::Path,
    response::Json,
    routing::get,
    Router,
};
use serde::Serialize;
use std::env;

mod utils;

#[derive(Serialize)]
struct FibonacciResponse {
    n: u32,
    result: u64,
}

#[derive(Serialize)]
struct ErrorResponse {
    error: String,
}

async fn fibonacci_handler(Path(n): Path<u32>) -> Json<serde_json::Value> {
    if n > 93 {
        return Json(serde_json::json!({
            "error": format!("n 过大：第 {} 个斐波那契数超出 u64 范围（最大支持 n = 93）", n)
        }));
    }
    let result = utils::fibonacci(n);
    Json(serde_json::json!({
        "n": n,
        "result": result
    }))
}

#[tokio::main]
async fn main() {
    let addr = env::var("ADDR").unwrap_or_else(|_| "127.0.0.1:3000".to_string());
    let app = Router::new().route("/fibonacci/:n", get(fibonacci_handler));
    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
    println!("fibonacci 服务已启动: http://{}", addr);
    axum::serve(listener, app).await.unwrap();
}
