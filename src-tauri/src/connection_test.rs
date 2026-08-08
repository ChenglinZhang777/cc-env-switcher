use reqwest::{Client, StatusCode};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::time::Duration;

#[derive(Deserialize)]
pub struct ConnectionTestInput {
    pub base_url: String,
    pub auth_token: String,
    pub model: String,
}

#[derive(Serialize)]
pub struct ConnectionTestResult {
    pub kind: String,
}

fn endpoint(base_url: &str) -> Result<String, String> {
    let base = base_url.trim().trim_end_matches('/');
    if base.is_empty() { return Err("缺少 API 地址".into()); }
    let parsed = reqwest::Url::parse(base).map_err(|_| "API 地址格式无效".to_string())?;
    if !matches!(parsed.scheme(), "https" | "http") { return Err("API 地址必须使用 HTTP 或 HTTPS".into()); }
    Ok(format!("{base}/v1/messages"))
}

fn payload(model: &str) -> Value {
    json!({"model": model, "max_tokens": 1, "messages": [{"role": "user", "content": "ping"}]})
}

fn classify_status(status: StatusCode) -> &'static str {
    if status.is_success() { "success" }
    else if matches!(status.as_u16(), 401 | 403) { "authentication" }
    else if matches!(status.as_u16(), 400 | 404) { "request" }
    else { "unavailable" }
}

pub async fn test(input: ConnectionTestInput) -> Result<ConnectionTestResult, String> {
    let url = endpoint(&input.base_url)?;
    if input.auth_token.trim().is_empty() { return Err("缺少 API Key".into()); }
    if input.model.trim().is_empty() { return Err("缺少主模型".into()); }
    let response = Client::builder()
        .timeout(Duration::from_secs(20))
        .build()
        .map_err(|_| "无法建立测试连接".to_string())?
        .post(url)
        .header("content-type", "application/json")
        .header("x-api-key", &input.auth_token)
        .header("authorization", format!("Bearer {}", input.auth_token))
        .header("anthropic-version", "2023-06-01")
        .json(&payload(&input.model))
        .send()
        .await;
    match response {
        Ok(response) => Ok(ConnectionTestResult { kind: classify_status(response.status()).into() }),
        Err(_) => Ok(ConnectionTestResult { kind: "network".into() }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_endpoint_and_builds_minimal_payload() {
        assert_eq!(endpoint("https://api.example.com/anthropic/").unwrap(), "https://api.example.com/anthropic/v1/messages");
        assert_eq!(payload("deepseek-v4-flash[1m]"), json!({"model": "deepseek-v4-flash[1m]", "max_tokens": 1, "messages": [{"role": "user", "content": "ping"}]}));
    }

    #[test]
    fn classifies_http_status_without_body() {
        assert_eq!(classify_status(StatusCode::UNAUTHORIZED), "authentication");
        assert_eq!(classify_status(StatusCode::BAD_REQUEST), "request");
        assert_eq!(classify_status(StatusCode::TOO_MANY_REQUESTS), "unavailable");
        assert_eq!(classify_status(StatusCode::OK), "success");
    }
}
