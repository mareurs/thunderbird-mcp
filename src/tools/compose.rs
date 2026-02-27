use rmcp::{model::CallToolResult, Error as McpError};
use serde_json::json;
use crate::bridge::{Bridge, BridgeError};
use super::mail::{bridge_err, result_text};

pub async fn send_mail(
    bridge: &Bridge,
    to: Vec<String>,
    subject: String,
    body: String,
    cc: Option<Vec<String>>,
    bcc: Option<Vec<String>>,
    from_identity: Option<String>,
) -> Result<CallToolResult, McpError> {
    let r = bridge.call("/mail/send", json!({
        "to": to, "subject": subject, "body": body,
        "cc": cc, "bcc": bcc, "from_identity": from_identity
    })).await.map_err(bridge_err)?;
    Ok(result_text(r))
}

pub async fn reply_to_message(
    bridge: &Bridge,
    message_id: String,
    body: String,
    reply_all: Option<bool>,
) -> Result<CallToolResult, McpError> {
    let r = bridge.call("/mail/reply", json!({
        "message_id": message_id, "body": body, "reply_all": reply_all
    })).await.map_err(bridge_err)?;
    Ok(result_text(r))
}

pub async fn forward_message(
    bridge: &Bridge,
    message_id: String,
    to: Vec<String>,
    body: Option<String>,
) -> Result<CallToolResult, McpError> {
    let r = bridge.call("/mail/forward", json!({
        "message_id": message_id, "to": to, "body": body
    })).await.map_err(bridge_err)?;
    Ok(result_text(r))
}
