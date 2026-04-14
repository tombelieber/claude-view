use axum::body::Body;
use axum::http::{Request, StatusCode};
use axum::routing::get;
use axum::Router;
use tower::ServiceExt;

async fn hello() -> &'static str {
    "ok"
}

#[tokio::test]
async fn generates_request_id_when_absent() {
    let app =
        claude_view_observability::apply_request_id_layers(Router::new().route("/", get(hello)));
    let resp = app
        .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let rid = resp
        .headers()
        .get("x-request-id")
        .expect("x-request-id header must be present");
    let rid_str = rid.to_str().unwrap();
    assert_eq!(
        rid_str.len(),
        26,
        "request id should be 26-char ULID, got len={} val={rid_str}",
        rid_str.len()
    );
}

#[tokio::test]
async fn preserves_incoming_request_id() {
    let app =
        claude_view_observability::apply_request_id_layers(Router::new().route("/", get(hello)));
    let resp = app
        .oneshot(
            Request::builder()
                .uri("/")
                .header("x-request-id", "client-provided-abc123")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let rid = resp
        .headers()
        .get("x-request-id")
        .unwrap()
        .to_str()
        .unwrap();
    assert_eq!(rid, "client-provided-abc123");
}
