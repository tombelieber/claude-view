use reqwest::Client;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Provider {
    Omlx,
    Ollama,
    LmStudio,
    Custom,
}

pub const PROBE_ORDER: &[(Provider, u16)] = &[
    (Provider::Omlx, 10710),
    (Provider::Ollama, 11434),
    (Provider::LmStudio, 1234),
];

#[derive(Deserialize)]
struct ModelsResponse {
    data: Vec<ModelEntry>,
}

#[derive(Deserialize)]
struct ModelEntry {
    id: String,
}

/// Probe a URL's `/v1/models` endpoint. Returns model IDs if server is healthy.
pub async fn probe_url(client: &Client, base_url: &str) -> Option<Vec<String>> {
    let resp = client
        .get(format!("{base_url}/v1/models"))
        .send()
        .await
        .ok()?;
    let body: ModelsResponse = resp.json().await.ok()?;
    let models: Vec<String> = body.data.into_iter().map(|m| m.id).collect();
    if models.is_empty() {
        None
    } else {
        Some(models)
    }
}

/// Probe known provider ports in priority order. Returns first success.
pub async fn probe_providers(
    client: &Client,
    order: &[(Provider, u16)],
) -> Option<(Provider, String, Vec<String>)> {
    for (provider, port) in order {
        let url = format!("http://localhost:{port}");
        if let Some(models) = probe_url(client, &url).await {
            return Some((*provider, url, models));
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn probe_url_returns_models_on_success() {
        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("GET", "/v1/models")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{"object":"list","data":[{"id":"test-model","object":"model","created":0,"owned_by":"test"}]}"#)
            .create_async()
            .await;

        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(2))
            .build()
            .unwrap();
        let result = probe_url(&client, &server.url()).await;

        mock.assert_async().await;
        assert_eq!(result, Some(vec!["test-model".to_string()]));
    }

    #[tokio::test]
    async fn probe_url_returns_none_on_connection_refused() {
        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(1))
            .build()
            .unwrap();
        let result = probe_url(&client, "http://127.0.0.1:39999").await;
        assert_eq!(result, None);
    }

    #[tokio::test]
    async fn probe_url_returns_none_on_empty_models() {
        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("GET", "/v1/models")
            .with_status(200)
            .with_body(r#"{"object":"list","data":[]}"#)
            .create_async()
            .await;

        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(2))
            .build()
            .unwrap();
        let result = probe_url(&client, &server.url()).await;

        mock.assert_async().await;
        assert_eq!(result, None);
    }
}
