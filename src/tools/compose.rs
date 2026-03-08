use rmcp::{model::CallToolResult, Error as McpError};
use serde_json::json;
use crate::bridge::Bridge;
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

#[cfg(test)]
mod tests {
    use super::*;
    use mockito::Server;
    use serde_json::json;

    async fn mock_bridge(server: &Server) -> Bridge {
        Bridge::with_base_url("tok".to_string(), server.url())
    }

    #[tokio::test]
    async fn send_mail_hits_correct_endpoint_with_recipients() {
        let mut server = Server::new_async().await;
        let mock = server.mock("POST", "/mail/send")
            .match_body(mockito::Matcher::PartialJson(json!({
                "to": ["alice@example.com"],
                "subject": "Hi",
                "body": "Hello"
            })))
            .with_status(200).with_body("{}").create_async().await;
        send_mail(
            &mock_bridge(&server).await,
            vec!["alice@example.com".into()],
            "Hi".into(), "Hello".into(), None, None, None,
        ).await.unwrap();
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn reply_to_message_sends_message_id_and_body() {
        let mut server = Server::new_async().await;
        let mock = server.mock("POST", "/mail/reply")
            .match_body(mockito::Matcher::PartialJson(json!({
                "message_id": "msg-1",
                "body": "Thanks!"
            })))
            .with_status(200).with_body("{}").create_async().await;
        reply_to_message(&mock_bridge(&server).await, "msg-1".into(), "Thanks!".into(), None).await.unwrap();
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn send_mail_sends_optional_cc_and_bcc() {
        let mut server = Server::new_async().await;
        let mock = server.mock("POST", "/mail/send")
            .match_body(mockito::Matcher::PartialJson(json!({
                "to": ["alice@example.com"],
                "subject": "Hi",
                "body": "Hello",
                "cc": ["cc@example.com"],
                "bcc": ["bcc@example.com"]
            })))
            .with_status(200).with_body("{}").create_async().await;
        send_mail(
            &mock_bridge(&server).await,
            vec!["alice@example.com".into()],
            "Hi".into(), "Hello".into(),
            Some(vec!["cc@example.com".into()]),
            Some(vec!["bcc@example.com".into()]),
            None,
        ).await.unwrap();
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn reply_to_message_sends_reply_all() {
        let mut server = Server::new_async().await;
        let mock = server.mock("POST", "/mail/reply")
            .match_body(mockito::Matcher::PartialJson(json!({
                "message_id": "msg-1",
                "body": "Thanks!",
                "reply_all": true
            })))
            .with_status(200).with_body("{}").create_async().await;
        reply_to_message(
            &mock_bridge(&server).await,
            "msg-1".into(), "Thanks!".into(), Some(true),
        ).await.unwrap();
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn forward_message_sends_message_id_and_recipients() {
        let mut server = Server::new_async().await;
        let mock = server.mock("POST", "/mail/forward")
            .match_body(mockito::Matcher::PartialJson(json!({
                "message_id": "msg-2",
                "to": ["bob@example.com"]
            })))
            .with_status(200).with_body("{}").create_async().await;
        forward_message(
            &mock_bridge(&server).await,
            "msg-2".into(), vec!["bob@example.com".into()], None,
        ).await.unwrap();
        mock.assert_async().await;
    }
}
