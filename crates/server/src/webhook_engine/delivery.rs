//! HMAC-SHA256 signing and HTTP delivery for webhooks.
//!
//! Follows the Standard Webhooks spec for signing:
//! signed_content = "{webhook_id}.{timestamp}.{body}"
//! signature = HMAC-SHA256(base64_decode(secret), signed_content)

use base64::Engine;
use hmac::{Hmac, Mac};
use sha2::Sha256;

type HmacSha256 = Hmac<Sha256>;

/// Result of a webhook delivery attempt.
#[derive(Debug)]
pub struct DeliveryResult {
    pub success: bool,
    pub status_code: Option<u16>,
    pub attempts: u32,
    pub error: Option<String>,
}

/// Sign a payload body using HMAC-SHA256 (Standard Webhooks spec).
///
/// `secret` is the full `whsec_...` string — this function strips the prefix
/// and base64-decodes the key material.
pub fn sign_payload(webhook_id: &str, timestamp: i64, body: &str, secret: &str) -> String {
    let key_b64 = secret.strip_prefix("whsec_").unwrap_or(secret);
    let key_bytes = base64::engine::general_purpose::STANDARD
        .decode(key_b64)
        .expect("invalid base64 in signing secret");

    let signed_content = format!("{webhook_id}.{timestamp}.{body}");
    let mut mac = HmacSha256::new_from_slice(&key_bytes).expect("HMAC accepts any key length");
    mac.update(signed_content.as_bytes());
    let result = mac.finalize().into_bytes();

    format!(
        "v1,{}",
        base64::engine::general_purpose::STANDARD.encode(result)
    )
}

/// Deliver a webhook with retry.
///
/// Sends HTTP POST with Standard Webhooks headers. Retries up to `max_attempts`
/// on 5xx responses or network errors. Does NOT retry on 4xx (client error).
pub async fn deliver(
    client: &reqwest::Client,
    url: &str,
    webhook_id: &str,
    body: String,
    secret: &str,
    max_attempts: u32,
) -> DeliveryResult {
    let timestamp = chrono::Utc::now().timestamp();
    let signature = sign_payload(webhook_id, timestamp, &body, secret);

    let mut last_error = None;
    let mut last_status = None;

    for attempt in 0..max_attempts {
        if attempt > 0 {
            // Simple backoff: 100ms, 500ms
            let delay = if attempt == 1 { 100 } else { 500 };
            tokio::time::sleep(std::time::Duration::from_millis(delay)).await;
        }

        let result = client
            .post(url)
            .header("content-type", "application/json")
            .header("user-agent", "claude-view/1.0")
            .header("webhook-id", webhook_id)
            .header("webhook-timestamp", timestamp.to_string())
            .header("webhook-signature", &signature)
            .body(body.clone())
            .send()
            .await;

        match result {
            Ok(resp) => {
                let status = resp.status().as_u16();
                last_status = Some(status);

                if status < 300 {
                    return DeliveryResult {
                        success: true,
                        status_code: Some(status),
                        attempts: attempt + 1,
                        error: None,
                    };
                }

                // Don't retry client errors (4xx)
                if (400..500).contains(&status) {
                    return DeliveryResult {
                        success: false,
                        status_code: Some(status),
                        attempts: attempt + 1,
                        error: Some(format!("Client error: {status}")),
                    };
                }

                // 5xx → retry
                last_error = Some(format!("Server error: {status}"));
            }
            Err(e) => {
                last_error = Some(format!("Network error: {e}"));
            }
        }
    }

    DeliveryResult {
        success: false,
        status_code: last_status,
        attempts: max_attempts,
        error: last_error,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_secret() -> String {
        // Generate a deterministic test secret
        let key = b"test-webhook-signing-key-32bytes!";
        format!(
            "whsec_{}",
            base64::engine::general_purpose::STANDARD.encode(key)
        )
    }

    #[test]
    fn sign_payload_is_deterministic() {
        let secret = test_secret();
        let sig1 = sign_payload("evt_123", 1000000, r#"{"test":true}"#, &secret);
        let sig2 = sign_payload("evt_123", 1000000, r#"{"test":true}"#, &secret);
        assert_eq!(sig1, sig2);
    }

    #[test]
    fn sign_payload_differs_for_different_body() {
        let secret = test_secret();
        let sig1 = sign_payload("evt_123", 1000000, r#"{"a":1}"#, &secret);
        let sig2 = sign_payload("evt_123", 1000000, r#"{"a":2}"#, &secret);
        assert_ne!(sig1, sig2);
    }

    #[test]
    fn sign_payload_starts_with_v1() {
        let secret = test_secret();
        let sig = sign_payload("evt_123", 1000000, "body", &secret);
        assert!(sig.starts_with("v1,"), "got: {sig}");
    }

    #[test]
    fn sign_payload_base64_after_prefix() {
        let secret = test_secret();
        let sig = sign_payload("evt_123", 1000000, "body", &secret);
        let b64_part = sig.strip_prefix("v1,").unwrap();
        assert!(base64::engine::general_purpose::STANDARD
            .decode(b64_part)
            .is_ok());
    }

    #[test]
    fn sign_payload_can_be_verified_independently() {
        let secret = test_secret();
        let webhook_id = "evt_verify";
        let timestamp = 1234567890i64;
        let body = r#"{"event":"test"}"#;

        let sig = sign_payload(webhook_id, timestamp, body, &secret);
        let b64_sig = sig.strip_prefix("v1,").unwrap();
        let sig_bytes = base64::engine::general_purpose::STANDARD
            .decode(b64_sig)
            .unwrap();

        // Independently verify
        let key_b64 = secret.strip_prefix("whsec_").unwrap();
        let key_bytes = base64::engine::general_purpose::STANDARD
            .decode(key_b64)
            .unwrap();
        let signed_content = format!("{webhook_id}.{timestamp}.{body}");
        let mut mac = HmacSha256::new_from_slice(&key_bytes).unwrap();
        mac.update(signed_content.as_bytes());
        mac.verify_slice(&sig_bytes)
            .expect("signature should verify");
    }

    #[tokio::test]
    async fn deliver_success_on_200() {
        // Spin up a mock server
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let url = format!("http://{addr}/webhook");

        // Mock server that returns 200
        tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            let io = hyper_util::rt::TokioIo::new(stream);
            hyper::server::conn::http1::Builder::new()
                .serve_connection(
                    io,
                    hyper::service::service_fn(|_req| async {
                        Ok::<_, std::convert::Infallible>(hyper::Response::new(
                            http_body_util::Full::new(hyper::body::Bytes::from("ok")),
                        ))
                    }),
                )
                .await
                .ok();
        });

        let client = reqwest::Client::new();
        let secret = test_secret();
        let result = deliver(
            &client,
            &url,
            "evt_test",
            r#"{"test":true}"#.into(),
            &secret,
            3,
        )
        .await;

        assert!(result.success);
        assert_eq!(result.status_code, Some(200));
        assert_eq!(result.attempts, 1);
        assert!(result.error.is_none());
    }

    #[tokio::test]
    async fn deliver_fails_on_4xx_no_retry() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let url = format!("http://{addr}/webhook");

        tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            let io = hyper_util::rt::TokioIo::new(stream);
            hyper::server::conn::http1::Builder::new()
                .serve_connection(
                    io,
                    hyper::service::service_fn(|_req| async {
                        Ok::<_, std::convert::Infallible>(
                            hyper::Response::builder()
                                .status(400)
                                .body(http_body_util::Full::new(hyper::body::Bytes::from(
                                    "bad request",
                                )))
                                .unwrap(),
                        )
                    }),
                )
                .await
                .ok();
        });

        let client = reqwest::Client::new();
        let secret = test_secret();
        let result = deliver(&client, &url, "evt_test", "{}".into(), &secret, 3).await;

        assert!(!result.success);
        assert_eq!(result.status_code, Some(400));
        assert_eq!(result.attempts, 1); // no retry on 4xx
    }

    #[tokio::test]
    async fn deliver_fails_on_bad_url() {
        let client = reqwest::Client::new();
        let secret = test_secret();
        let result = deliver(
            &client,
            "http://127.0.0.1:1/nonexistent",
            "evt_test",
            "{}".into(),
            &secret,
            1,
        )
        .await;

        assert!(!result.success);
        assert!(result.error.is_some());
    }
}
