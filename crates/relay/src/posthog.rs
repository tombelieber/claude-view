use reqwest::Client;

pub async fn track(
    client: &Client,
    api_key: &str,
    event: &str,
    user_id: &str,
    props: serde_json::Value,
) {
    if api_key.is_empty() {
        return;
    }
    let _ = client
        .post("https://us.i.posthog.com/capture/")
        .json(&serde_json::json!({
            "api_key": api_key,
            "event": event,
            "distinct_id": user_id,
            "properties": props,
        }))
        .send()
        .await;
}
