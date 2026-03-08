use rmcp::{model::CallToolResult, Error as McpError};
use serde_json::{json, Value};
use crate::bridge::Bridge;
use super::mail::{bridge_err, result_text};

pub async fn list_filters(
    bridge: &Bridge,
    account_id: Option<String>,
) -> Result<CallToolResult, McpError> {
    let r = bridge.call("/filters/list", json!({"account_id": account_id}))
        .await.map_err(bridge_err)?;
    Ok(result_text(r))
}

pub async fn create_filter(
    bridge: &Bridge,
    account_id: String,
    name: String,
    conditions: Value,
    actions: Value,
    enabled: Option<bool>,
) -> Result<CallToolResult, McpError> {
    let r = bridge.call("/filters/create", json!({
        "account_id": account_id, "name": name,
        "conditions": conditions, "actions": actions,
        "enabled": enabled.unwrap_or(true)
    })).await.map_err(bridge_err)?;
    Ok(result_text(r))
}

pub async fn update_filter(
    bridge: &Bridge,
    account_id: String,
    filter_index: u32,
    name: Option<String>,
    enabled: Option<bool>,
    conditions: Option<Value>,
    actions: Option<Value>,
) -> Result<CallToolResult, McpError> {
    let r = bridge.call("/filters/update", json!({
        "account_id": account_id, "filter_index": filter_index,
        "name": name, "enabled": enabled,
        "conditions": conditions, "actions": actions
    })).await.map_err(bridge_err)?;
    Ok(result_text(r))
}

pub async fn delete_filter(
    bridge: &Bridge,
    account_id: String,
    filter_index: u32,
) -> Result<CallToolResult, McpError> {
    let r = bridge.call("/filters/delete", json!({
        "account_id": account_id, "filter_index": filter_index
    })).await.map_err(bridge_err)?;
    Ok(result_text(r))
}

pub async fn reorder_filters(
    bridge: &Bridge,
    account_id: String,
    from_index: u32,
    to_index: u32,
) -> Result<CallToolResult, McpError> {
    let r = bridge.call("/filters/reorder", json!({
        "account_id": account_id,
        "from_index": from_index,
        "to_index": to_index
    })).await.map_err(bridge_err)?;
    Ok(result_text(r))
}

pub async fn apply_filters(
    bridge: &Bridge,
    account_id: String,
    folder_uri: String,
) -> Result<CallToolResult, McpError> {
    let r = bridge.call("/filters/apply", json!({
        "account_id": account_id, "folder_uri": folder_uri
    })).await.map_err(bridge_err)?;
    Ok(result_text(r))
}

#[cfg(test)]
mod tests {
    use super::*;
    use mockito::Server;
    use serde_json::json;

    async fn mock_bridge(server: &Server) -> Bridge {
        Bridge::with_base_url("tok".to_string(), server.url())
    }

    #[tokio::test]
    async fn list_filters_hits_correct_endpoint() {
        let mut server = Server::new_async().await;
        let mock = server.mock("POST", "/filters/list")
            .with_status(200).with_body("{}").create_async().await;
        list_filters(&mock_bridge(&server).await, None).await.unwrap();
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn create_filter_defaults_enabled_to_true() {
        let mut server = Server::new_async().await;
        let mock = server.mock("POST", "/filters/create")
            .match_body(mockito::Matcher::PartialJson(json!({"enabled": true})))
            .with_status(200).with_body("{}").create_async().await;
        create_filter(
            &mock_bridge(&server).await,
            "acc".into(), "My Filter".into(),
            json!([]), json!([]), None,
        ).await.unwrap();
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn create_filter_respects_explicit_enabled_false() {
        let mut server = Server::new_async().await;
        let mock = server.mock("POST", "/filters/create")
            .match_body(mockito::Matcher::PartialJson(json!({"enabled": false})))
            .with_status(200).with_body("{}").create_async().await;
        create_filter(
            &mock_bridge(&server).await,
            "acc".into(), "Disabled".into(),
            json!([]), json!([]), Some(false),
        ).await.unwrap();
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn update_filter_sends_filter_index() {
        let mut server = Server::new_async().await;
        let mock = server.mock("POST", "/filters/update")
            .match_body(mockito::Matcher::PartialJson(json!({"account_id": "acc", "filter_index": 2})))
            .with_status(200).with_body("{}").create_async().await;
        update_filter(&mock_bridge(&server).await, "acc".into(), 2, None, None, None, None).await.unwrap();
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn delete_filter_sends_account_and_index() {
        let mut server = Server::new_async().await;
        let mock = server.mock("POST", "/filters/delete")
            .match_body(mockito::Matcher::PartialJson(json!({"account_id": "acc", "filter_index": 3})))
            .with_status(200).with_body("{}").create_async().await;
        delete_filter(&mock_bridge(&server).await, "acc".into(), 3).await.unwrap();
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn reorder_filters_sends_from_and_to_index() {
        let mut server = Server::new_async().await;
        let mock = server.mock("POST", "/filters/reorder")
            .match_body(mockito::Matcher::PartialJson(json!({"from_index": 1, "to_index": 4})))
            .with_status(200).with_body("{}").create_async().await;
        reorder_filters(&mock_bridge(&server).await, "acc".into(), 1, 4).await.unwrap();
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn apply_filters_sends_account_and_folder() {
        let mut server = Server::new_async().await;
        let mock = server.mock("POST", "/filters/apply")
            .match_body(mockito::Matcher::PartialJson(json!({
                "account_id": "acc",
                "folder_uri": "imap://host/INBOX"
            })))
            .with_status(200).with_body("{}").create_async().await;
        apply_filters(&mock_bridge(&server).await, "acc".into(), "imap://host/INBOX".into()).await.unwrap();
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn apply_filters_propagates_bridge_error() {
        let mut server = Server::new_async().await;
        let _mock = server.mock("POST", "/filters/apply")
            .with_status(200)
            .with_body("{\"error\": \"account not found\"}")
            .create_async().await;
        let result = apply_filters(
            &mock_bridge(&server).await,
            "acc".into(),
            "imap://host/INBOX".into(),
        ).await;
        assert!(result.is_err());
    }

}
