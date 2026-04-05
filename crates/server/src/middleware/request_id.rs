//! V1-hardening M1.2 — Request ID middleware.
//!
//! Every request gets an `X-Request-Id` header. If the client provides one,
//! we honor it; otherwise we generate a UUIDv4. The ID is echoed on the
//! response and inserted into the request's extensions so handlers can
//! retrieve it via `Extension<RequestId>` for structured logging.
//!
//! Why: correlating a user bug report to server logs requires a stable
//! identifier per request. tracing's `TraceLayer` already creates a span
//! per request; this middleware threads a stable ID through that span so
//! one bug report can be grepped out of minutes of logs.

use axum::{extract::Request, http::HeaderValue, middleware::Next, response::Response};

const HEADER: &str = "x-request-id";

/// Request ID wrapper stored in request extensions.
#[derive(Clone, Debug)]
pub struct RequestId(pub String);

impl RequestId {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Axum middleware that ensures every request has an `X-Request-Id`.
///
/// Applied via `Router::layer(middleware::from_fn(set_request_id))`.
pub async fn set_request_id(mut req: Request, next: Next) -> Response {
    // Prefer client-supplied ID (for distributed tracing). Fall back to uuid.
    let request_id = req
        .headers()
        .get(HEADER)
        .and_then(|v| v.to_str().ok())
        .filter(|s| !s.is_empty() && s.len() <= 128)
        .map(String::from)
        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());

    // Stash in extensions so handlers + tracing spans can read it.
    req.extensions_mut().insert(RequestId(request_id.clone()));

    let mut resp = next.run(req).await;

    // Echo on the response.
    if let Ok(header) = HeaderValue::from_str(&request_id) {
        resp.headers_mut().insert(HEADER, header);
    }
    resp
}

/// Backwards-compat alias — some codebases prefer the `Layer` type signature.
pub struct SetRequestIdLayer;

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{body::Body, http::Request as HttpRequest, routing::get, Router};
    use http_body_util::BodyExt;
    use tower::ServiceExt;

    async fn echo_id(axum::Extension(id): axum::Extension<RequestId>) -> String {
        id.0
    }

    fn app() -> Router {
        Router::new()
            .route("/echo", get(echo_id))
            .layer(axum::middleware::from_fn(set_request_id))
    }

    #[tokio::test]
    async fn generates_uuid_when_header_absent() {
        let resp = app()
            .oneshot(
                HttpRequest::builder()
                    .uri("/echo")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let header = resp
            .headers()
            .get(HEADER)
            .unwrap()
            .to_str()
            .unwrap()
            .to_string();
        assert_eq!(header.len(), 36, "UUIDv4 length"); // 8-4-4-4-12

        let body = resp.into_body().collect().await.unwrap().to_bytes();
        let body_str = std::str::from_utf8(&body).unwrap();
        assert_eq!(body_str, header, "body echoes the same ID as header");
    }

    #[tokio::test]
    async fn preserves_client_supplied_id() {
        let client_id = "trace-abc-123";
        let resp = app()
            .oneshot(
                HttpRequest::builder()
                    .uri("/echo")
                    .header(HEADER, client_id)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let header = resp.headers().get(HEADER).unwrap().to_str().unwrap();
        assert_eq!(header, client_id);
    }

    #[tokio::test]
    async fn rejects_overlong_id_and_generates_new() {
        let overlong = "a".repeat(129);
        let resp = app()
            .oneshot(
                HttpRequest::builder()
                    .uri("/echo")
                    .header(HEADER, overlong)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let header = resp.headers().get(HEADER).unwrap().to_str().unwrap();
        assert_eq!(header.len(), 36, "overlong header replaced by uuid");
    }
}
