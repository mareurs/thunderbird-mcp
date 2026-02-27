use rmcp::{model::CallToolResult, Error as McpError};
use serde_json::json;
use crate::bridge::{Bridge, BridgeError};
use super::mail::{bridge_err, result_text};

pub async fn search_contacts(
    bridge: &Bridge,
    query: String,
    limit: Option<u32>,
) -> Result<CallToolResult, McpError> {
    let r = bridge.call("/contacts/search", json!({
        "query": query, "limit": limit
    })).await.map_err(bridge_err)?;
    Ok(result_text(r))
}

pub async fn list_calendars(bridge: &Bridge) -> Result<CallToolResult, McpError> {
    let r = bridge.call("/calendars/list", json!({})).await.map_err(bridge_err)?;
    Ok(result_text(r))
}

pub async fn create_event(
    bridge: &Bridge,
    calendar_id: String,
    title: String,
    start: String,
    end: String,
    description: Option<String>,
    location: Option<String>,
) -> Result<CallToolResult, McpError> {
    let r = bridge.call("/calendar/create-event", json!({
        "calendar_id": calendar_id, "title": title,
        "start": start, "end": end,
        "description": description, "location": location
    })).await.map_err(bridge_err)?;
    Ok(result_text(r))
}
