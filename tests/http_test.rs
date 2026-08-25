use axum::body::Body;
use axum::http::{Request, StatusCode};
use fibonacci_cli::{app, init_pool};
use std::sync::Once;
use tower::ServiceExt;

static INIT: Once = Once::new();

fn setup() {
    INIT.call_once(|| {
        init_pool();
    });
}

#[tokio::test]
async fn test_health() {
    setup();
    let app = app();
    let request = Request::builder()
        .uri("/health")
        .body(Body::empty())
        .unwrap();
    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_fibonacci_10() {
    setup();
    let app = app();
    let request = Request::builder()
        .uri("/fibonacci/10")
        .body(Body::empty())
        .unwrap();
    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_fibonacci_abc() {
    setup();
    let app = app();
    let request = Request::builder()
        .uri("/fibonacci/abc")
        .body(Body::empty())
        .unwrap();
    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_fibonacci_100() {
    setup();
    let app = app();
    let request = Request::builder()
        .uri("/fibonacci/100")
        .body(Body::empty())
        .unwrap();
    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_fibonacci_sequence_10() {
    setup();
    let app = app();
    let request = Request::builder()
        .uri("/fibonacci/sequence/10")
        .body(Body::empty())
        .unwrap();
    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_task_create() {
    setup();
    let app = app();
    let request = Request::builder()
        .method("POST")
        .uri("/task")
        .header("Content-Type", "application/json")
        .body(Body::from(r#"{"duration_secs":0}"#))
        .unwrap();
    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);
}

#[tokio::test]
async fn test_task_get_0() {
    setup();
    let app = app();
    let create_req = Request::builder()
        .method("POST")
        .uri("/task")
        .header("Content-Type", "application/json")
        .body(Body::from(r#"{"duration_secs":0}"#))
        .unwrap();
    let create_resp = app.clone().oneshot(create_req).await.unwrap();
    assert_eq!(create_resp.status(), StatusCode::CREATED);

    let get_req = Request::builder()
        .uri("/task/0")
        .body(Body::empty())
        .unwrap();
    let get_resp = app.oneshot(get_req).await.unwrap();
    assert_eq!(get_resp.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_task_get_999() {
    setup();
    let app = app();
    let request = Request::builder()
        .uri("/task/999")
        .body(Body::empty())
        .unwrap();
    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}
