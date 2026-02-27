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
) -> Result<CallToolResult, McpError> {
    let r = bridge.call("/messages/search", json!({
        "query": query, "folder": folder, "sender": sender,
        "recipient": recipient, "date_from": date_from,
        "date_to": date_to, "max_results": max_results
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
