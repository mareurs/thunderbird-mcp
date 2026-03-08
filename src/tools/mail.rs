use rmcp::{model::{CallToolResult, Content}, Error as McpError};
use serde_json::json;
use crate::bridge::{Bridge, BridgeError};

pub fn bridge_err(e: BridgeError) -> McpError {
    McpError::internal_error(e.to_string(), None)
}

pub fn result_text(v: serde_json::Value) -> CallToolResult {
    CallToolResult::success(vec![Content::text(
        serde_json::to_string_pretty(&v).unwrap_or_default()
    )])
}

pub async fn list_accounts(bridge: &Bridge) -> Result<CallToolResult, McpError> {
    let r = bridge.call("/accounts/list", json!({})).await.map_err(bridge_err)?;
    Ok(result_text(r))
}

pub async fn list_folders(
    bridge: &Bridge,
    account_id: Option<String>,
    folder_uri: Option<String>,
) -> Result<CallToolResult, McpError> {
    let r = bridge.call("/folders/list", json!({
        "account_id": account_id,
        "folder_uri": folder_uri
    })).await.map_err(bridge_err)?;
    Ok(result_text(r))
}

pub async fn search_messages(
    bridge: &Bridge,
    query: Option<String>,
    folder: Option<String>,
    sender: Option<String>,
    recipient: Option<String>,
    date_from: Option<String>,
    date_to: Option<String>,
    max_results: Option<u32>,
    search_body: Option<bool>,
) -> Result<CallToolResult, McpError> {
    let r = bridge.call("/messages/search", json!({
        "query": query, "folder": folder, "sender": sender,
        "recipient": recipient, "date_from": date_from,
        "date_to": date_to, "max_results": max_results,
        "search_body": search_body,
    })).await.map_err(bridge_err)?;
    Ok(result_text(r))
}

pub async fn get_message(
    bridge: &Bridge,
    message_id: String,
    save_attachments: Option<bool>,
) -> Result<CallToolResult, McpError> {
    let r = bridge.call("/messages/get", json!({
        "message_id": message_id,
        "save_attachments": save_attachments
    })).await.map_err(bridge_err)?;
    Ok(result_text(r))
}

pub async fn get_recent_messages(
    bridge: &Bridge,
    folder: Option<String>,
    limit: Option<u32>,
    unread_only: Option<bool>,
    since_date: Option<String>,
) -> Result<CallToolResult, McpError> {
    let r = bridge.call("/messages/recent", json!({
        "folder": folder, "limit": limit,
        "unread_only": unread_only, "since_date": since_date
    })).await.map_err(bridge_err)?;
    Ok(result_text(r))
}

pub async fn update_message(
    bridge: &Bridge,
    message_id: String,
    read: Option<bool>,
    flagged: Option<bool>,
    move_to: Option<String>,
    trash: Option<bool>,
) -> Result<CallToolResult, McpError> {
    let r = bridge.call("/messages/update", json!({
        "message_id": message_id, "read": read,
        "flagged": flagged, "move_to": move_to, "trash": trash
    })).await.map_err(bridge_err)?;
    Ok(result_text(r))
}

pub async fn delete_messages(
    bridge: &Bridge,
    message_ids: Vec<String>,
) -> Result<CallToolResult, McpError> {
    let r = bridge.call("/messages/delete", json!({
        "message_ids": message_ids
    })).await.map_err(bridge_err)?;
    Ok(result_text(r))
}

pub async fn delete_messages_by_filter(
    bridge: &Bridge,
    folder_uri: String,
    date_from: Option<String>,
    date_to: Option<String>,
    sender: Option<String>,
    query: Option<String>,
) -> Result<CallToolResult, McpError> {
    let r = bridge.call("/messages/delete-by-filter", serde_json::json!({
        "folder_uri": folder_uri,
        "date_from": date_from,
        "date_to": date_to,
        "sender": sender,
        "query": query,
    })).await.map_err(bridge_err)?;
    Ok(result_text(r))
}

pub async fn create_folder(
    bridge: &Bridge,
    parent_uri: String,
    name: String,
) -> Result<CallToolResult, McpError> {
    let r = bridge.call("/folders/create", json!({
        "parent_uri": parent_uri, "name": name
    })).await.map_err(bridge_err)?;
    Ok(result_text(r))
}

pub async fn delete_folder(
    bridge: &Bridge,
    folder_uri: String,
) -> Result<CallToolResult, McpError> {
    let r = bridge.call("/folders/delete", json!({
        "folder_uri": folder_uri
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
    async fn list_accounts_hits_correct_endpoint() {
        let mut server = Server::new_async().await;
        let mock = server.mock("POST", "/accounts/list")
            .with_status(200).with_body("{}").create_async().await;
        list_accounts(&mock_bridge(&server).await).await.unwrap();
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn list_folders_sends_account_id() {
        let mut server = Server::new_async().await;
        let mock = server.mock("POST", "/folders/list")
            .match_body(mockito::Matcher::PartialJson(json!({"account_id": "acc1"})))
            .with_status(200).with_body("{}").create_async().await;
        list_folders(&mock_bridge(&server).await, Some("acc1".into()), None).await.unwrap();
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn search_messages_sends_query_and_sender() {
        let mut server = Server::new_async().await;
        let mock = server.mock("POST", "/messages/search")
            .match_body(mockito::Matcher::PartialJson(json!({"query": "invoice", "sender": "boss@corp.com"})))
            .with_status(200).with_body("{}").create_async().await;
        search_messages(
            &mock_bridge(&server).await,
            Some("invoice".into()), None, Some("boss@corp.com".into()),
            None, None, None, None, None,
        ).await.unwrap();
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn search_messages_sends_search_body_true() {
        let mut server = Server::new_async().await;
        let mock = server.mock("POST", "/messages/search")
            .match_body(mockito::Matcher::PartialJson(json!({
                "query": "invoice",
                "search_body": true
            })))
            .with_status(200).with_body("{}").create_async().await;
        search_messages(
            &mock_bridge(&server).await,
            Some("invoice".into()), None, None,
            None, None, None, None, Some(true),
        ).await.unwrap();
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn get_message_sends_message_id() {
        let mut server = Server::new_async().await;
        let mock = server.mock("POST", "/messages/get")
            .match_body(mockito::Matcher::PartialJson(json!({"message_id": "msg-42"})))
            .with_status(200).with_body("{}").create_async().await;
        get_message(&mock_bridge(&server).await, "msg-42".into(), None).await.unwrap();
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn get_recent_messages_sends_limit_and_unread_only() {
        let mut server = Server::new_async().await;
        let mock = server.mock("POST", "/messages/recent")
            .match_body(mockito::Matcher::PartialJson(json!({"limit": 5, "unread_only": true})))
            .with_status(200).with_body("{}").create_async().await;
        get_recent_messages(&mock_bridge(&server).await, None, Some(5), Some(true), None).await.unwrap();
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn update_message_sends_read_and_flagged() {
        let mut server = Server::new_async().await;
        let mock = server.mock("POST", "/messages/update")
            .match_body(mockito::Matcher::PartialJson(json!({"message_id": "m1", "read": true})))
            .with_status(200).with_body("{}").create_async().await;
        update_message(&mock_bridge(&server).await, "m1".into(), Some(true), None, None, None).await.unwrap();
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn update_message_sends_move_to() {
        let mut server = Server::new_async().await;
        let mock = server.mock("POST", "/messages/update")
            .match_body(mockito::Matcher::PartialJson(json!({
                "message_id": "m1",
                "move_to": "imap://user@host/Archive"
            })))
            .with_status(200).with_body("{}").create_async().await;
        update_message(
            &mock_bridge(&server).await,
            "m1".into(), None, None,
            Some("imap://user@host/Archive".into()), None,
        ).await.unwrap();
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn update_message_sends_trash() {
        let mut server = Server::new_async().await;
        let mock = server.mock("POST", "/messages/update")
            .match_body(mockito::Matcher::PartialJson(json!({
                "message_id": "m1",
                "trash": true
            })))
            .with_status(200).with_body("{}").create_async().await;
        update_message(
            &mock_bridge(&server).await,
            "m1".into(), None, None, None, Some(true),
        ).await.unwrap();
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn delete_messages_sends_ids() {
        let mut server = Server::new_async().await;
        let mock = server.mock("POST", "/messages/delete")
            .match_body(mockito::Matcher::PartialJson(json!({"message_ids": ["a", "b"]})))
            .with_status(200).with_body("{}").create_async().await;
        delete_messages(&mock_bridge(&server).await, vec!["a".into(), "b".into()]).await.unwrap();
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn delete_messages_by_filter_sends_folder_and_filters() {
        let mut server = Server::new_async().await;
        let mock = server.mock("POST", "/messages/delete-by-filter")
            .match_body(mockito::Matcher::PartialJson(json!({
                "folder_uri": "imap://user@host/INBOX",
                "sender": "spam@bad.com"
            })))
            .with_status(200).with_body("{}").create_async().await;
        delete_messages_by_filter(
            &mock_bridge(&server).await,
            "imap://user@host/INBOX".into(),
            None, None, Some("spam@bad.com".into()), None,
        ).await.unwrap();
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn delete_messages_by_filter_with_only_folder_uri() {
        let mut server = Server::new_async().await;
        let mock = server.mock("POST", "/messages/delete-by-filter")
            .match_body(mockito::Matcher::PartialJson(json!({
                "folder_uri": "imap://user@host/INBOX"
            })))
            .with_status(200).with_body("{}").create_async().await;
        delete_messages_by_filter(
            &mock_bridge(&server).await,
            "imap://user@host/INBOX".into(),
            None, None, None, None,
        ).await.unwrap();
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn create_folder_sends_parent_and_name() {
        let mut server = Server::new_async().await;
        let mock = server.mock("POST", "/folders/create")
            .match_body(mockito::Matcher::PartialJson(json!({"parent_uri": "imap://host/", "name": "Archive"})))
            .with_status(200).with_body("{}").create_async().await;
        create_folder(&mock_bridge(&server).await, "imap://host/".into(), "Archive".into()).await.unwrap();
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn delete_folder_sends_folder_uri() {
        let mut server = Server::new_async().await;
        let mock = server.mock("POST", "/folders/delete")
            .match_body(mockito::Matcher::PartialJson(json!({
                "folder_uri": "imap://user@host/Archive"
            })))
            .with_status(200).with_body("{}").create_async().await;
        delete_folder(&mock_bridge(&server).await, "imap://user@host/Archive".into()).await.unwrap();
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn delete_folder_propagates_bridge_error() {
        let mut server = Server::new_async().await;
        let _mock = server.mock("POST", "/folders/delete")
            .with_status(200)
            .with_body("{\"error\": \"folder not found\"}")
            .create_async().await;
        let result = delete_folder(&mock_bridge(&server).await, "imap://user@host/Archive".into()).await;
        assert!(result.is_err());
    }

    #[test]
    fn result_text_produces_success_with_pretty_json() {
        let v = json!({"key": "value"});
        let result = result_text(v);
        assert!(!result.content.is_empty());
        let text = result.content[0].as_text().unwrap();
        assert!(text.text.contains("\"key\""));
        assert!(text.text.contains("\"value\""));
    }

    #[tokio::test]
    async fn update_message_propagates_bridge_error() {
        let mut server = Server::new_async().await;
        let _mock = server.mock("POST", "/messages/update")
            .with_status(200)
            .with_body("{\"error\": \"permission denied\"}")
            .create_async().await;
        let result = update_message(
            &mock_bridge(&server).await,
            "m1".into(), None, None, None, None,
        ).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn delete_messages_propagates_bridge_error() {
        let mut server = Server::new_async().await;
        let _mock = server.mock("POST", "/messages/delete")
            .with_status(200)
            .with_body("{\"error\": \"not found\"}")
            .create_async().await;
        let result = delete_messages(
            &mock_bridge(&server).await,
            vec!["m1".into()],
        ).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn delete_messages_by_filter_propagates_bridge_error() {
        let mut server = Server::new_async().await;
        let _mock = server.mock("POST", "/messages/delete-by-filter")
            .with_status(200)
            .with_body("{\"error\": \"folder not found\"}")
            .create_async().await;
        let result = delete_messages_by_filter(
            &mock_bridge(&server).await,
            "imap://user@host/INBOX".into(),
            None, None, None, None,
        ).await;
        assert!(result.is_err());
    }

    #[test]
    fn bridge_err_maps_to_internal_error() {
        use crate::bridge::BridgeError;
        let e = BridgeError::ExtensionError("oops".into());
        let mcp_err = bridge_err(e);
        assert!(mcp_err.to_string().contains("oops"));
    }
}
