use fibonacci_cli::{app, init_pool};

#[tokio::main]
async fn main() {
    // 初始化 tracing：默认 INFO 级别输出到控制台，可用 RUST_LOG 覆盖
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .init();

    // 初始化全局任务池（只初始化一次）
    init_pool();

    // 监听地址可通过环境变量 ADDR 覆盖（Docker 容器内需监听 0.0.0.0 才能让 -p 端口映射生效）
    let addr = std::env::var("ADDR").unwrap_or_else(|_| "127.0.0.1:3000".to_string());

    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .expect("端口绑定失败");
    tracing::info!(addr = %addr, "fibonacci 服务已启动");

    axum::serve(listener, app()).await.expect("服务运行失败");
}
