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
    let mut props = props.as_object().cloned().unwrap_or_default();
    props.extend(
        serde_json::from_value::<serde_json::Map<String, serde_json::Value>>(serde_json::json!({
            "product": "claude_view",
            "schema_version": 1,
            "source": "relay",
            "deployment": "relay",
            "environment": "production",
            "$process_person_profile": false,
            "$geoip_disable": true,
            "$ip": null,
        }))
        .expect("static telemetry properties are an object"),
    );
    let _ = client
        .post("https://us.i.posthog.com/capture/")
        .json(&serde_json::json!({
            "api_key": api_key,
            "event": event,
            "distinct_id": format!("claude_view:relay:{user_id}"),
            "properties": props,
        }))
        .send()
        .await;
}
