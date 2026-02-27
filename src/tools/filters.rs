use rmcp::{model::CallToolResult, Error as McpError};
use serde_json::{json, Value};
use crate::bridge::{Bridge, BridgeError};
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
