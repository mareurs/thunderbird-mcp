use rmcp::{model::{CallToolResult, Content}, Error as McpError};
use serde_json::json;
use std::path::Path;
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
    get_message_in(bridge, message_id, save_attachments, &body_dir()).await
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

/// Longest body (in chars) that `get_message` returns inline. Longer bodies go to a file.
const MAX_INLINE_BODY_CHARS: usize = 4000;
/// Chars of the body start returned as `bodyPreview` when the body goes to a file.
const BODY_PREVIEW_CHARS: usize = 500;
/// Number of body files kept in the body directory; older ones are deleted.
const MAX_BODY_FILES: usize = 50;

/// Moves a long `body` out of the response into a private file, so the caller reads it
/// with `grep`/`awk`/`sed` instead of paying for it in context.
///
/// A body of at most `max_inline` chars stays inline. The response then only gains
/// `bodyLength`. Otherwise `body` is replaced by `bodyPreview` (`BODY_PREVIEW_CHARS` chars),
/// `bodyLength`, `bodyFile` (absolute path) and `bodyHint`. `r` changes only if the write
/// succeeds, so on `Err` the caller still has the full body.
fn offload_body(r: &mut serde_json::Value, dir: &Path, max_inline: usize) -> std::io::Result<()> {
    let Some(body) = r.get("body").and_then(|b| b.as_str()) else {
        return Ok(());
    };
    let length = body.chars().count();
    if length <= max_inline {
        r["bodyLength"] = json!(length);
        return Ok(());
    }

    let preview: String = body.chars().take(BODY_PREVIEW_CHARS).collect();
    let path = write_body_file(dir, body)?;
    // Best-effort cleanup: failing to delete an old file must not lose this body.
    prune_body_files(dir, MAX_BODY_FILES).ok();

    if let Some(obj) = r.as_object_mut() {
        obj.remove("body");
        obj.insert("bodyPreview".into(), json!(preview));
        obj.insert("bodyLength".into(), json!(length));
        obj.insert("bodyFile".into(), json!(path.to_string_lossy()));
        obj.insert("bodyHint".into(), json!(
            "Full body is in bodyFile. Search it with grep, awk or sed on that path; do not read the whole file."
        ));
    }
    Ok(())
}

/// Deletes all but the newest `keep` body files in `dir`. File names sort in time order
/// (`body-<ms>-<n>.txt`), so no mtimes are needed. Other files in `dir` are left alone.
fn prune_body_files(dir: &Path, keep: usize) -> std::io::Result<()> {
    let mut files = std::fs::read_dir(dir)?
        .map(|entry| entry.map(|e| e.path()))
        .collect::<std::io::Result<Vec<_>>>()?;
    files.retain(|p| {
        p.file_name()
            .and_then(|n| n.to_str())
            .is_some_and(|n| n.starts_with("body-") && n.ends_with(".txt"))
    });
    files.sort();
    let excess = files.len().saturating_sub(keep);
    files.iter().take(excess).try_for_each(std::fs::remove_file)
}

/// Writes `body` to a new `0600` file in `dir` (created `0700`) and returns its absolute path.
/// The binary is built for Linux only, so Unix permission APIs are used directly.
fn write_body_file(dir: &Path, body: &str) -> std::io::Result<std::path::PathBuf> {
    use std::io::Write;
    use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt, PermissionsExt};
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    // Distinguishes two files written in the same millisecond.
    static COUNTER: AtomicU64 = AtomicU64::new(0);

    std::fs::DirBuilder::new().recursive(true).mode(0o700).create(dir)?;
    // The directory may already exist with looser permissions.
    std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700))?;

    let ms = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_millis());
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let path = dir.canonicalize()?.join(format!("body-{ms:013}-{n:06}.txt"));

    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&path)?;
    file.write_all(body.as_bytes())?;
    Ok(path)
}

/// Where `get_message` puts long bodies: per-user runtime dir, else the cache dir.
fn body_dir() -> std::path::PathBuf {
    dirs::runtime_dir()
        .or_else(dirs::cache_dir)
        .unwrap_or_else(std::env::temp_dir)
        .join("thunderbird-mcp")
        .join("bodies")
}

pub async fn get_message_in(
    bridge: &Bridge,
    message_id: String,
    save_attachments: Option<bool>,
    body_dir: &Path,
) -> Result<CallToolResult, McpError> {
    let mut r = bridge.call("/messages/get", json!({
        "message_id": message_id,
        "save_attachments": save_attachments
    })).await.map_err(bridge_err)?;
    if let Err(e) = offload_body(&mut r, body_dir, MAX_INLINE_BODY_CHARS) {
        // `r` is unchanged on error, so the caller still gets the full body.
        r["bodyFileError"] = json!(e.to_string());
    }
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

    fn long_body() -> String {
        // Multi-byte chars: the limit counts chars, not bytes.
        "é".repeat(MAX_INLINE_BODY_CHARS + 1)
    }

    #[test]
    fn short_body_stays_inline_and_reports_length() {
        let dir = tempfile::tempdir().unwrap();
        let bodies = dir.path().join("bodies");
        let mut r = json!({"body": "hello"});
        offload_body(&mut r, &bodies, MAX_INLINE_BODY_CHARS).unwrap();
        assert_eq!(r["body"], "hello");
        assert_eq!(r["bodyLength"], 5);
        assert!(r.get("bodyFile").is_none());
        assert!(!bodies.exists(), "no directory is created for an inline body");
    }

    #[test]
    fn body_of_exactly_max_chars_stays_inline() {
        let dir = tempfile::tempdir().unwrap();
        let mut r = json!({"body": "é".repeat(MAX_INLINE_BODY_CHARS)});
        offload_body(&mut r, &dir.path().join("bodies"), MAX_INLINE_BODY_CHARS).unwrap();
        assert!(r.get("bodyFile").is_none());
        assert_eq!(r["bodyLength"], MAX_INLINE_BODY_CHARS);
    }

    #[test]
    fn response_without_body_is_untouched() {
        let dir = tempfile::tempdir().unwrap();
        for original in [json!({}), json!({"body": null}), json!({"body": ""})] {
            let mut r = original.clone();
            offload_body(&mut r, &dir.path().join("bodies"), MAX_INLINE_BODY_CHARS).unwrap();
            assert!(r.get("bodyFile").is_none());
            assert_eq!(r["body"], original["body"]);
        }
    }

    #[test]
    fn long_body_goes_to_file_with_preview() {
        let dir = tempfile::tempdir().unwrap();
        let bodies = dir.path().join("bodies");
        let body = long_body();
        let mut r = json!({"subject": "s", "body": body});
        offload_body(&mut r, &bodies, MAX_INLINE_BODY_CHARS).unwrap();

        assert!(r.get("body").is_none());
        assert_eq!(r["subject"], "s");
        assert_eq!(r["bodyLength"], MAX_INLINE_BODY_CHARS + 1);
        assert_eq!(r["bodyPreview"].as_str().unwrap().chars().count(), BODY_PREVIEW_CHARS);
        let path = std::path::PathBuf::from(r["bodyFile"].as_str().unwrap());
        assert!(path.is_absolute());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), body);
        assert!(r["bodyHint"].as_str().unwrap().contains("grep"));
    }

    #[test]
    fn body_file_and_directory_are_private() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let bodies = dir.path().join("bodies");
        let mut r = json!({"body": long_body()});
        offload_body(&mut r, &bodies, MAX_INLINE_BODY_CHARS).unwrap();

        let file_mode = std::fs::metadata(r["bodyFile"].as_str().unwrap()).unwrap().permissions().mode();
        let dir_mode = std::fs::metadata(&bodies).unwrap().permissions().mode();
        assert_eq!(file_mode & 0o777, 0o600);
        assert_eq!(dir_mode & 0o777, 0o700);
    }

    #[test]
    fn two_long_bodies_get_distinct_files() {
        let dir = tempfile::tempdir().unwrap();
        let bodies = dir.path().join("bodies");
        let mut a = json!({"body": long_body()});
        let mut b = json!({"body": long_body()});
        offload_body(&mut a, &bodies, MAX_INLINE_BODY_CHARS).unwrap();
        offload_body(&mut b, &bodies, MAX_INLINE_BODY_CHARS).unwrap();
        assert_ne!(a["bodyFile"], b["bodyFile"]);
    }

    #[test]
    fn prune_keeps_newest_body_files_only() {
        let dir = tempfile::tempdir().unwrap();
        for i in 1..=5 {
            std::fs::write(dir.path().join(format!("body-{i:013}-000000.txt")), "x").unwrap();
        }
        std::fs::write(dir.path().join("notes.txt"), "keep me").unwrap();
        prune_body_files(dir.path(), 2).unwrap();

        let mut left: Vec<String> = std::fs::read_dir(dir.path()).unwrap()
            .map(|e| e.unwrap().file_name().into_string().unwrap())
            .collect();
        left.sort();
        assert_eq!(left, vec![
            "body-0000000000004-000000.txt",
            "body-0000000000005-000000.txt",
            "notes.txt",
        ]);
    }

    #[tokio::test]
    async fn get_message_offloads_long_body_through_bridge() {
        let mut server = Server::new_async().await;
        let body = long_body();
        server.mock("POST", "/messages/get")
            .with_status(200)
            .with_body(json!({"id": "m1", "body": body}).to_string())
            .create_async().await;
        let dir = tempfile::tempdir().unwrap();

        let res = get_message_in(&mock_bridge(&server).await, "m1".into(), None, dir.path()).await.unwrap();
        let v = serde_json::to_value(&res).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(v["content"][0]["text"].as_str().unwrap()).unwrap();

        assert_eq!(parsed["id"], "m1");
        assert!(parsed.get("body").is_none());
        assert_eq!(std::fs::read_to_string(parsed["bodyFile"].as_str().unwrap()).unwrap(), body);
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
