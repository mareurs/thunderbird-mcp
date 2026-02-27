use reqwest::Client;
use serde_json::Value;

#[derive(thiserror::Error, Debug)]
pub enum BridgeError {
    #[error("Thunderbird not reachable — is it running with the MCP extension?")]
    ConnectionFailed(#[from] reqwest::Error),
    #[error("Extension error: {0}")]
    ExtensionError(String),
    #[error("Invalid JSON from extension: {0}")]
    InvalidJson(#[from] serde_json::Error),
    #[error("Unauthorized — auth token mismatch")]
    Unauthorized,
}

#[derive(Clone)]
pub struct Bridge {
    client: Client,
    base_url: String,
    token: String,
}

impl Bridge {
    pub fn new(token: String) -> Self {
        Self::with_base_url(token, "http://localhost:8765".to_string())
    }

    pub fn with_base_url(token: String, base_url: String) -> Self {
        Self { client: Client::new(), base_url, token }
    }

pub async fn call(&self, path: &str, params: Value) -> Result<Value, BridgeError> {
        let url = format!("{}{}", self.base_url, path);
        let resp = self.client
            .post(&url)
            .bearer_auth(&self.token)
            .json(&params)
            .send()
            .await?;

        if resp.status() == reqwest::StatusCode::UNAUTHORIZED {
            return Err(BridgeError::Unauthorized);
        }

        let text = resp.text().await?;
        let value: Value = serde_json::from_str(&crate::sanitize::sanitize_str(&text))?;

        if let Some(err) = value.get("error").and_then(|e| e.as_str()) {
            return Err(BridgeError::ExtensionError(err.to_string()));
        }

        Ok(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mockito::Server;
    use serde_json::json;

    async fn mock_bridge(server: &Server) -> Bridge {
        Bridge::with_base_url("test-token".to_string(), server.url())
    }

    #[tokio::test]
    async fn returns_json_on_success() {
        let mut server = Server::new_async().await;
        let mock = server.mock("POST", "/accounts/list")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{"accounts": []}"#)
            .create_async().await;

        let bridge = mock_bridge(&server).await;
        let result = bridge.call("/accounts/list", json!({})).await.unwrap();
        assert_eq!(result["accounts"], json!([]));
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn returns_extension_error_on_error_field() {
        let mut server = Server::new_async().await;
        server.mock("POST", "/messages/search")
            .with_status(200)
            .with_body(r#"{"error": "folder not found"}"#)
            .create_async().await;

        let bridge = mock_bridge(&server).await;
        let err = bridge.call("/messages/search", json!({})).await.unwrap_err();
        assert!(matches!(err, BridgeError::ExtensionError(ref s) if s == "folder not found"));
    }

    #[tokio::test]
    async fn returns_unauthorized_on_401() {
        let mut server = Server::new_async().await;
        server.mock("POST", "/accounts/list")
            .with_status(401)
            .create_async().await;

        let bridge = mock_bridge(&server).await;
        let err = bridge.call("/accounts/list", json!({})).await.unwrap_err();
        assert!(matches!(err, BridgeError::Unauthorized));
    }

    #[tokio::test]
    async fn sends_bearer_token() {
        let mut server = Server::new_async().await;
        let mock = server.mock("POST", "/accounts/list")
            .match_header("authorization", "Bearer test-token")
            .with_status(200)
            .with_body(r#"{}"#)
            .create_async().await;

        let bridge = mock_bridge(&server).await;
        let _ = bridge.call("/accounts/list", json!({})).await;
        mock.assert_async().await;
    }
}
