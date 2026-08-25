use axum::body::Body;
use axum::http::{Request, StatusCode};
use axum::Router;
use fibonacci_cli::{app, init_pool};
use http_body_util::BodyExt;
use std::sync::Once;
use tower::ServiceExt;

/// 保证全局任务池只初始化一次（跨测试共享）。
static INIT: Once = Once::new();

fn setup() {
    INIT.call_once(init_pool);
}

/// 发送请求并解析响应为 `(状态码, JSON body)`。
async fn send(req: Request<Body>) -> (StatusCode, serde_json::Value) {
    let router: Router = app();
    let response = router.oneshot(req).await.unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let body = serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null);
    (status, body)
}

#[tokio::test]
async fn test_health() {
    setup();
    let req = Request::builder()
        .uri("/health")
        .body(Body::empty())
        .unwrap();
    let (status, body) = send(req).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body, serde_json::json!({"status": "ok"}));
}

#[tokio::test]
async fn test_fibonacci_10() {
    setup();
    let req = Request::builder()
        .uri("/fibonacci/10")
        .body(Body::empty())
        .unwrap();
    let (status, body) = send(req).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body, serde_json::json!({"n": 10, "result": 55}));
}

#[tokio::test]
async fn test_fibonacci_abc() {
    setup();
    let req = Request::builder()
        .uri("/fibonacci/abc")
        .body(Body::empty())
        .unwrap();
    let (status, _) = send(req).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_fibonacci_100() {
    setup();
    let req = Request::builder()
        .uri("/fibonacci/100")
        .body(Body::empty())
        .unwrap();
    let (status, _) = send(req).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_fibonacci_sequence_10() {
    setup();
    let req = Request::builder()
        .uri("/fibonacci/sequence/10")
        .body(Body::empty())
        .unwrap();
    let (status, body) = send(req).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body, serde_json::json!([0, 1, 1, 2, 3, 5, 8, 13, 21, 34]));
}

#[tokio::test]
async fn test_task_create() {
    setup();
    let req = Request::builder()
        .method("POST")
        .uri("/task")
        .header("Content-Type", "application/json")
        .body(Body::from(r#"{"duration_secs":0}"#))
        .unwrap();
    let (status, body) = send(req).await;
    assert_eq!(status, StatusCode::CREATED);
    assert!(
        body["task_id"].is_u64(),
        "响应应包含数值型 task_id，实际: {body}"
    );
}

#[tokio::test]
async fn test_task_get_created() {
    setup();
    // 先创建任务并解析 task_id（全局任务池跨测试共享，id 不固定为 0）
    let create_req = Request::builder()
        .method("POST")
        .uri("/task")
        .header("Content-Type", "application/json")
        .body(Body::from(r#"{"duration_secs":0}"#))
        .unwrap();
    let (status, body) = send(create_req).await;
    assert_eq!(status, StatusCode::CREATED);
    let task_id = body["task_id"].as_u64().expect("响应应包含 task_id");

    let get_req = Request::builder()
        .uri(format!("/task/{task_id}"))
        .body(Body::empty())
        .unwrap();
    let (status, body) = send(get_req).await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        body["status"].is_string(),
        "响应应包含 status 字段，实际: {body}"
    );
}

#[tokio::test]
async fn test_task_get_not_found() {
    setup();
    // 测试进程创建的任务数量远小于 999，该 id 一定不存在
    let req = Request::builder()
        .uri("/task/999")
        .body(Body::empty())
        .unwrap();
    let (status, body) = send(req).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(body, serde_json::json!({"status": "not_found"}));
}
