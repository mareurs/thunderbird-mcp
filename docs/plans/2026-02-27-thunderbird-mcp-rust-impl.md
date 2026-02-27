# Thunderbird MCP Rust Rewrite — Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Replace the Node.js `mcp-bridge.cjs` with a Rust binary that owns the entire MCP surface, while slimming the Thunderbird extension to a ~400-line pure XPCOM adapter.

**Architecture:** Rust binary (`thunderbird-mcp`) reads JSON-RPC from stdio via `rmcp`, translates each MCP tool call into a `POST /domain/operation` HTTP request to the Thunderbird extension, and returns the result. The Thunderbird extension (JS) exposes one endpoint per XPCOM operation and has zero MCP awareness.

**Tech Stack:** Rust 1.75+, `rmcp` (MCP SDK), `tokio` (async), `reqwest` (HTTP client), `mockito` (test HTTP server), `thiserror`/`anyhow` (errors), `dirs` (home dir), `tempfile` (test fixtures). Extension: JavaScript (WebExtension experiment_apis), Mozilla `httpd.sys.mjs`.

**Reference:** Design doc at `docs/plans/2026-02-27-thunderbird-mcp-rust-design.md`

---

## Task 1: Rust project scaffold

**Files:**
- Create: `Cargo.toml`
- Create: `src/main.rs`
- Create: `src/server.rs`
- Create: `src/bridge.rs`
- Create: `src/auth.rs`
- Create: `src/sanitize.rs`
- Create: `src/tools/mod.rs`
- Create: `src/tools/mail.rs`
- Create: `src/tools/compose.rs`
- Create: `src/tools/filters.rs`
- Create: `src/tools/contacts.rs`

**Step 1: Create the Rust project**

```bash
cd /home/marius/work/claude/thunderbird-mcp
cargo init --name thunderbird-mcp
```

Expected: creates `Cargo.toml` and `src/main.rs`.

**Step 2: Replace Cargo.toml**

```toml
[package]
name = "thunderbird-mcp"
version = "0.1.0"
edition = "2021"
description = "MCP server for Thunderbird — email, contacts, calendar, filters"
license = "MIT"

[[bin]]
name = "thunderbird-mcp"
path = "src/main.rs"

[dependencies]
rmcp       = { version = "0.1", features = ["server", "transport-io"] }
tokio      = { version = "1",   features = ["full"] }
reqwest    = { version = "0.12", features = ["json"] }
serde      = { version = "1",   features = ["derive"] }
serde_json = "1"
thiserror  = "2"
anyhow     = "1"
dirs       = "5"

[dev-dependencies]
mockito    = "1"
tempfile   = "3"
tokio-test = "0.4"
```

**Step 3: Create module stub files**

`src/auth.rs`:
```rust
// Auth token discovery
pub fn find_token() -> anyhow::Result<String> {
    todo!()
}
```

`src/sanitize.rs`:
```rust
// UTF-8 / control char sanitization
pub fn sanitize_str(s: &str) -> String {
    todo!()
}
```

`src/bridge.rs`:
```rust
// HTTP client to Thunderbird extension
pub struct Bridge;
```

`src/server.rs`:
```rust
// MCP server — ThunderbirdMcp struct + tool registrations
pub struct ThunderbirdMcp;
```

`src/tools/mod.rs`:
```rust
pub mod mail;
pub mod compose;
pub mod filters;
pub mod contacts;
```

`src/tools/mail.rs`, `src/tools/compose.rs`, `src/tools/filters.rs`, `src/tools/contacts.rs`: empty files for now.

`src/main.rs`:
```rust
mod auth;
mod bridge;
mod sanitize;
mod server;
mod tools;

fn main() {
    println!("thunderbird-mcp");
}
```

**Step 4: Verify it compiles**

```bash
cargo build 2>&1 | head -20
```

Expected: compiles with warnings about `todo!()` and unused code, no errors.

**Step 5: Commit**

```bash
git add Cargo.toml Cargo.lock src/
git commit -m "feat: scaffold Rust project structure"
```

---

## Task 2: auth.rs — token discovery with snap fallback

**Files:**
- Modify: `src/auth.rs`

The Thunderbird extension writes its auth token to `~/.thunderbird-mcp-auth`. When Thunderbird is installed as a snap package, `$HOME` inside the snap is remapped to `~/snap/thunderbird/common/`, so the file ends up at `~/snap/thunderbird/common/.thunderbird-mcp-auth`. We must check both paths.

**Step 1: Write the failing tests**

Add to `src/auth.rs`:

```rust
use std::path::PathBuf;

#[derive(thiserror::Error, Debug)]
pub enum AuthError {
    #[error("Cannot determine home directory")]
    NoHome,
    #[error("Auth token not found at {paths:?}. Is Thunderbird running with the MCP extension?")]
    NotFound { paths: Vec<PathBuf> },
}

pub fn find_token() -> Result<String, AuthError> {
    todo!()
}

// Testable inner function — accepts home dir as parameter
pub fn find_token_in(home: &std::path::Path) -> Result<String, AuthError> {
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn write_token(dir: &std::path::Path, rel: &str, token: &str) {
        let path = dir.join(rel);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, token).unwrap();
    }

    #[test]
    fn finds_token_at_home_path() {
        let tmp = TempDir::new().unwrap();
        write_token(tmp.path(), ".thunderbird-mcp-auth", "token-abc");
        let result = find_token_in(tmp.path()).unwrap();
        assert_eq!(result, "token-abc");
    }

    #[test]
    fn finds_token_at_snap_path() {
        let tmp = TempDir::new().unwrap();
        write_token(tmp.path(), "snap/thunderbird/common/.thunderbird-mcp-auth", "token-snap");
        let result = find_token_in(tmp.path()).unwrap();
        assert_eq!(result, "token-snap");
    }

    #[test]
    fn prefers_home_over_snap() {
        let tmp = TempDir::new().unwrap();
        write_token(tmp.path(), ".thunderbird-mcp-auth", "token-home");
        write_token(tmp.path(), "snap/thunderbird/common/.thunderbird-mcp-auth", "token-snap");
        let result = find_token_in(tmp.path()).unwrap();
        assert_eq!(result, "token-home");
    }

    #[test]
    fn trims_whitespace() {
        let tmp = TempDir::new().unwrap();
        write_token(tmp.path(), ".thunderbird-mcp-auth", "  token-xyz\n");
        let result = find_token_in(tmp.path()).unwrap();
        assert_eq!(result, "token-xyz");
    }

    #[test]
    fn returns_error_when_not_found() {
        let tmp = TempDir::new().unwrap();
        let result = find_token_in(tmp.path());
        assert!(matches!(result, Err(AuthError::NotFound { .. })));
    }
}
```

**Step 2: Run tests to verify they fail**

```bash
cargo test auth 2>&1
```

Expected: compile error on `todo!()` panics, or FAIL on all 5 tests.

**Step 3: Implement `find_token_in()`**

```rust
pub fn find_token_in(home: &std::path::Path) -> Result<String, AuthError> {
    let candidates = [
        home.join(".thunderbird-mcp-auth"),
        home.join("snap/thunderbird/common/.thunderbird-mcp-auth"),
    ];
    for path in &candidates {
        if let Ok(content) = std::fs::read_to_string(path) {
            return Ok(content.trim().to_string());
        }
    }
    Err(AuthError::NotFound { paths: candidates.to_vec() })
}

pub fn find_token() -> Result<String, AuthError> {
    let home = dirs::home_dir().ok_or(AuthError::NoHome)?;
    find_token_in(&home)
}
```

**Step 4: Run tests to verify they pass**

```bash
cargo test auth 2>&1
```

Expected: `test auth::tests::finds_token_at_home_path ... ok` × 5

**Step 5: Commit**

```bash
git add src/auth.rs
git commit -m "feat: auth token discovery with snap path fallback"
```

---

## Task 3: sanitize.rs — control char and newline sanitization

**Files:**
- Modify: `src/sanitize.rs`

Email bodies frequently contain raw control characters that break JSON parsing. This module strips control chars (except `\t`, `\n`, `\r`) and escapes raw (unescaped) newlines/carriage returns. Moved from the old JS bridge and api.js into Rust.

**Step 1: Write the failing tests**

```rust
pub fn sanitize_str(s: &str) -> String {
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_control_chars() {
        // 0x00-0x08, 0x0b, 0x0c, 0x0e-0x1f, 0x7f should be removed
        let input = "hello\x00world\x07foo\x7fbar";
        assert_eq!(sanitize_str(input), "helloworldfoobar");
    }

    #[test]
    fn preserves_tab_and_newline() {
        let input = "line1\nline2\ttabbed\r\n";
        // These are allowed — they appear in normal email text
        assert!(sanitize_str(input).contains('\n'));
        assert!(sanitize_str(input).contains('\t'));
    }

    #[test]
    fn handles_empty_string() {
        assert_eq!(sanitize_str(""), "");
    }

    #[test]
    fn preserves_unicode() {
        let input = "Héllo wörld 日本語";
        assert_eq!(sanitize_str(input), input);
    }

    #[test]
    fn handles_only_control_chars() {
        let input = "\x00\x01\x02\x03";
        assert_eq!(sanitize_str(input), "");
    }
}
```

**Step 2: Run tests to verify they fail**

```bash
cargo test sanitize 2>&1
```

Expected: FAIL (todo! panic).

**Step 3: Implement**

```rust
pub fn sanitize_str(s: &str) -> String {
    s.chars()
        .filter(|&c| {
            let n = c as u32;
            // Allow: \t (0x09), \n (0x0a), \r (0x0d), and everything >= 0x20 except DEL (0x7f)
            matches!(n, 0x09 | 0x0a | 0x0d) || (n >= 0x20 && n != 0x7f)
        })
        .collect()
}
```

**Step 4: Run tests to verify they pass**

```bash
cargo test sanitize 2>&1
```

Expected: all 5 tests pass.

**Step 5: Commit**

```bash
git add src/sanitize.rs
git commit -m "feat: sanitize control characters from email content"
```

---

## Task 4: bridge.rs — HTTP client to Thunderbird extension

**Files:**
- Modify: `src/bridge.rs`

The `Bridge` struct wraps a `reqwest` client, sends `POST /domain/operation` requests to `localhost:8765` with a Bearer auth token, and returns the parsed JSON response. Handles connection errors, auth errors, and extension-side errors (JSON `{ "error": "..." }`).

**Step 1: Write the failing tests**

```rust
use reqwest::Client;
use serde_json::{json, Value};
use std::sync::Arc;

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
        todo!()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mockito::{Server, Mock};

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
```

**Step 2: Run tests to verify they fail**

```bash
cargo test bridge 2>&1
```

Expected: compile errors or FAIL.

**Step 3: Implement `call()`**

```rust
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
```

**Step 4: Run tests to verify they pass**

```bash
cargo test bridge 2>&1
```

Expected: all 4 tests pass.

**Step 5: Commit**

```bash
git add src/bridge.rs src/sanitize.rs
git commit -m "feat: bridge HTTP client with auth, error handling, sanitization"
```

---

## Task 5: tools/mail.rs — 8 mail tools

**Files:**
- Modify: `src/tools/mail.rs`

Implement the 8 mail tools as async functions that call `bridge.call()`. Each function maps to one extension endpoint. Input types are Rust-native (Rust's type system enforces what was previously hand-written JSON Schema). Return `rmcp::CallToolResult`.

> **Note:** Check the `rmcp` crate documentation for the exact `#[tool]` and `#[arg]` macro syntax. The illustrative code below is correct in intent — adjust attribute names if the crate's API differs. Run `cargo doc --open -p rmcp` to browse the API locally.

**Step 1: Add tool implementations**

```rust
// src/tools/mail.rs
use rmcp::{handler::server::tool::Parameters, model::CallToolResult, Error as McpError};
use serde::Deserialize;
use serde_json::json;
use std::sync::Arc;
use crate::bridge::{Bridge, BridgeError};

fn bridge_err(e: BridgeError) -> McpError {
    McpError::internal_error(e.to_string(), None)
}

fn result_text(v: serde_json::Value) -> CallToolResult {
    CallToolResult::success(vec![rmcp::model::Content::text(
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
```

**Step 2: Verify it compiles**

```bash
cargo build 2>&1 | grep -E "^error"
```

Expected: no errors (warnings about unused imports are fine for now).

**Step 3: Commit**

```bash
git add src/tools/mail.rs
git commit -m "feat: implement 8 mail tool handlers"
```

---

## Task 6: tools/compose.rs — 3 compose tools

**Files:**
- Modify: `src/tools/compose.rs`

**Step 1: Add implementations**

```rust
// src/tools/compose.rs
use rmcp::{model::CallToolResult, Error as McpError};
use serde_json::json;
use crate::bridge::{Bridge, BridgeError};
use super::mail::result_text;  // reuse helper — if not pub, make it pub in mail.rs

fn bridge_err(e: BridgeError) -> McpError {
    McpError::internal_error(e.to_string(), None)
}

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
```

> **Note:** Move `result_text` and `bridge_err` to a shared `src/tools/util.rs` module if you prefer to avoid duplication. Add `pub mod util;` to `tools/mod.rs`.

**Step 2: Verify compile**

```bash
cargo build 2>&1 | grep "^error"
```

**Step 3: Commit**

```bash
git add src/tools/compose.rs src/tools/mail.rs
git commit -m "feat: implement 3 compose tool handlers"
```

---

## Task 7: tools/filters.rs — 6 filter tools

**Files:**
- Modify: `src/tools/filters.rs`

**Step 1: Add implementations**

```rust
// src/tools/filters.rs
use rmcp::{model::CallToolResult, Error as McpError};
use serde_json::{json, Value};
use crate::bridge::{Bridge, BridgeError};
use super::mail::result_text;

fn bridge_err(e: BridgeError) -> McpError {
    McpError::internal_error(e.to_string(), None)
}

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
    conditions: Value,   // structured JSON — validated by extension
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
```

**Step 2: Verify compile**

```bash
cargo build 2>&1 | grep "^error"
```

**Step 3: Commit**

```bash
git add src/tools/filters.rs
git commit -m "feat: implement 6 filter tool handlers"
```

---

## Task 8: tools/contacts.rs — 3 contact/calendar tools

**Files:**
- Modify: `src/tools/contacts.rs`

**Step 1: Add implementations**

```rust
// src/tools/contacts.rs
use rmcp::{model::CallToolResult, Error as McpError};
use serde_json::json;
use crate::bridge::{Bridge, BridgeError};
use super::mail::result_text;

fn bridge_err(e: BridgeError) -> McpError {
    McpError::internal_error(e.to_string(), None)
}

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
    start: String,     // ISO 8601
    end: String,       // ISO 8601
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
```

**Step 2: Verify compile**

```bash
cargo build 2>&1 | grep "^error"
```

**Step 3: Commit**

```bash
git add src/tools/contacts.rs
git commit -m "feat: implement 3 contact/calendar tool handlers"
```

---

## Task 9: server.rs + main.rs — wire rmcp

**Files:**
- Modify: `src/server.rs`
- Modify: `src/main.rs`

This registers all 20 tools with `rmcp`, wires up the stdio transport, and is the entry point of the binary. Check `rmcp` docs for the exact macro API: `cargo doc --open -p rmcp`.

**Step 1: Implement server.rs**

```rust
// src/server.rs
use std::sync::Arc;
use rmcp::{
    ServerHandler,
    model::{CallToolResult, ServerInfo, ServerCapabilities, Tool},
    tool, Error as McpError,
};
use serde_json::Value;
use crate::bridge::Bridge;
use crate::tools::{mail, compose, filters, contacts};

#[derive(Clone)]
pub struct ThunderbirdMcp {
    pub bridge: Arc<Bridge>,
}

// NOTE: Adjust the macro syntax below to match the rmcp version in Cargo.lock.
// Run: cargo doc --open -p rmcp  — look for ServerHandler / tool_box examples.
#[tool(tool_box)]
impl ThunderbirdMcp {
    #[tool(description = "List all email accounts and their identities")]
    async fn list_accounts(&self) -> Result<CallToolResult, McpError> {
        mail::list_accounts(&self.bridge).await
    }

    #[tool(description = "Browse folder tree. Optionally filter by account or a specific subtree.")]
    async fn list_folders(
        &self,
        #[arg(description = "Account ID to filter by")] account_id: Option<String>,
        #[arg(description = "Folder URI to list subtree from")] folder_uri: Option<String>,
    ) -> Result<CallToolResult, McpError> {
        mail::list_folders(&self.bridge, account_id, folder_uri).await
    }

    #[tool(description = "Search messages by subject, sender, recipient, date range or folder")]
    async fn search_messages(
        &self,
        #[arg(description = "Text to search in subject/body")] query: Option<String>,
        #[arg(description = "Folder URI to scope search")] folder: Option<String>,
        #[arg(description = "Filter by sender address")] sender: Option<String>,
        #[arg(description = "Filter by recipient address")] recipient: Option<String>,
        #[arg(description = "Start date (ISO 8601)")] date_from: Option<String>,
        #[arg(description = "End date (ISO 8601)")] date_to: Option<String>,
        #[arg(description = "Max results, default 20, max 100")] max_results: Option<u32>,
    ) -> Result<CallToolResult, McpError> {
        mail::search_messages(&self.bridge, query, folder, sender, recipient, date_from, date_to, max_results).await
    }

    #[tool(description = "Read full email content, optionally save attachments to disk")]
    async fn get_message(
        &self,
        #[arg(description = "Message ID")] message_id: String,
        #[arg(description = "Save attachments to ~/thunderbird-mcp-attachments/")] save_attachments: Option<bool>,
    ) -> Result<CallToolResult, McpError> {
        mail::get_message(&self.bridge, message_id, save_attachments).await
    }

    #[tool(description = "Get recent messages with optional date and unread filtering")]
    async fn get_recent_messages(
        &self,
        #[arg(description = "Folder URI")] folder: Option<String>,
        #[arg(description = "Number of messages, default 20")] limit: Option<u32>,
        #[arg(description = "Return only unread messages")] unread_only: Option<bool>,
        #[arg(description = "Return messages newer than this date (ISO 8601)")] since_date: Option<String>,
    ) -> Result<CallToolResult, McpError> {
        mail::get_recent_messages(&self.bridge, folder, limit, unread_only, since_date).await
    }

    #[tool(description = "Mark read/unread, flag/unflag, move between folders, or trash a message")]
    async fn update_message(
        &self,
        #[arg(description = "Message ID")] message_id: String,
        #[arg(description = "Mark as read (true) or unread (false)")] read: Option<bool>,
        #[arg(description = "Flag or unflag")] flagged: Option<bool>,
        #[arg(description = "Folder URI to move message to")] move_to: Option<String>,
        #[arg(description = "Move to trash")] trash: Option<bool>,
    ) -> Result<CallToolResult, McpError> {
        mail::update_message(&self.bridge, message_id, read, flagged, move_to, trash).await
    }

    #[tool(description = "Delete messages — drafts are moved to Trash")]
    async fn delete_messages(
        &self,
        #[arg(description = "Array of message IDs to delete")] message_ids: Vec<String>,
    ) -> Result<CallToolResult, McpError> {
        mail::delete_messages(&self.bridge, message_ids).await
    }

    #[tool(description = "Create a new subfolder under a parent folder")]
    async fn create_folder(
        &self,
        #[arg(description = "Parent folder URI")] parent_uri: String,
        #[arg(description = "New folder name")] name: String,
    ) -> Result<CallToolResult, McpError> {
        mail::create_folder(&self.bridge, parent_uri, name).await
    }

    #[tool(description = "Open a compose window with pre-filled recipients, subject, and body. Nothing sends without your review.")]
    async fn send_mail(
        &self,
        #[arg(description = "Recipient addresses")] to: Vec<String>,
        #[arg(description = "Email subject")] subject: String,
        #[arg(description = "Email body (plain text)")] body: String,
        #[arg(description = "CC addresses")] cc: Option<Vec<String>>,
        #[arg(description = "BCC addresses")] bcc: Option<Vec<String>>,
        #[arg(description = "From identity (email address)")] from_identity: Option<String>,
    ) -> Result<CallToolResult, McpError> {
        compose::send_mail(&self.bridge, to, subject, body, cc, bcc, from_identity).await
    }

    #[tool(description = "Reply to a message with quoted original. Opens compose window for review.")]
    async fn reply_to_message(
        &self,
        #[arg(description = "Message ID to reply to")] message_id: String,
        #[arg(description = "Reply body text")] body: String,
        #[arg(description = "Reply to all recipients")] reply_all: Option<bool>,
    ) -> Result<CallToolResult, McpError> {
        compose::reply_to_message(&self.bridge, message_id, body, reply_all).await
    }

    #[tool(description = "Forward a message with all attachments. Opens compose window for review.")]
    async fn forward_message(
        &self,
        #[arg(description = "Message ID to forward")] message_id: String,
        #[arg(description = "Recipient addresses")] to: Vec<String>,
        #[arg(description = "Optional forwarding note")] body: Option<String>,
    ) -> Result<CallToolResult, McpError> {
        compose::forward_message(&self.bridge, message_id, to, body).await
    }

    #[tool(description = "List all message filter rules with human-readable conditions and actions")]
    async fn list_filters(
        &self,
        #[arg(description = "Account ID to list filters for")] account_id: Option<String>,
    ) -> Result<CallToolResult, McpError> {
        filters::list_filters(&self.bridge, account_id).await
    }

    #[tool(description = "Create a message filter with structured conditions and actions")]
    async fn create_filter(
        &self,
        #[arg(description = "Account ID")] account_id: String,
        #[arg(description = "Filter name")] name: String,
        #[arg(description = "Array of condition objects with field/op/value")] conditions: Value,
        #[arg(description = "Array of action objects with type/value")] actions: Value,
        #[arg(description = "Enable filter immediately (default true)")] enabled: Option<bool>,
    ) -> Result<CallToolResult, McpError> {
        filters::create_filter(&self.bridge, account_id, name, conditions, actions, enabled).await
    }

    #[tool(description = "Modify a filter's name, enabled state, conditions, or actions")]
    async fn update_filter(
        &self,
        #[arg(description = "Account ID")] account_id: String,
        #[arg(description = "Filter index (from listFilters)")] filter_index: u32,
        #[arg(description = "New name")] name: Option<String>,
        #[arg(description = "Enable or disable")] enabled: Option<bool>,
        #[arg(description = "New conditions array")] conditions: Option<Value>,
        #[arg(description = "New actions array")] actions: Option<Value>,
    ) -> Result<CallToolResult, McpError> {
        filters::update_filter(&self.bridge, account_id, filter_index, name, enabled, conditions, actions).await
    }

    #[tool(description = "Remove a filter by its index")]
    async fn delete_filter(
        &self,
        #[arg(description = "Account ID")] account_id: String,
        #[arg(description = "Filter index (from listFilters)")] filter_index: u32,
    ) -> Result<CallToolResult, McpError> {
        filters::delete_filter(&self.bridge, account_id, filter_index).await
    }

    #[tool(description = "Change filter execution priority by moving a filter to a new index")]
    async fn reorder_filters(
        &self,
        #[arg(description = "Account ID")] account_id: String,
        #[arg(description = "Current filter index")] from_index: u32,
        #[arg(description = "Target filter index")] to_index: u32,
    ) -> Result<CallToolResult, McpError> {
        filters::reorder_filters(&self.bridge, account_id, from_index, to_index).await
    }

    #[tool(description = "Run all filters on a folder on demand")]
    async fn apply_filters(
        &self,
        #[arg(description = "Account ID")] account_id: String,
        #[arg(description = "Folder URI to run filters on")] folder_uri: String,
    ) -> Result<CallToolResult, McpError> {
        filters::apply_filters(&self.bridge, account_id, folder_uri).await
    }

    #[tool(description = "Search contacts across all address books")]
    async fn search_contacts(
        &self,
        #[arg(description = "Name, email, or any contact field to search")] query: String,
        #[arg(description = "Max results (default 20)")] limit: Option<u32>,
    ) -> Result<CallToolResult, McpError> {
        contacts::search_contacts(&self.bridge, query, limit).await
    }

    #[tool(description = "List all calendars (local and CalDAV)")]
    async fn list_calendars(&self) -> Result<CallToolResult, McpError> {
        contacts::list_calendars(&self.bridge).await
    }

    #[tool(description = "Open a pre-filled calendar event dialog for review before saving")]
    async fn create_event(
        &self,
        #[arg(description = "Calendar ID")] calendar_id: String,
        #[arg(description = "Event title")] title: String,
        #[arg(description = "Start time (ISO 8601)")] start: String,
        #[arg(description = "End time (ISO 8601)")] end: String,
        #[arg(description = "Event description")] description: Option<String>,
        #[arg(description = "Event location")] location: Option<String>,
    ) -> Result<CallToolResult, McpError> {
        contacts::create_event(&self.bridge, calendar_id, title, start, end, description, location).await
    }
}

impl ServerHandler for ThunderbirdMcp {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            name: "thunderbird-mcp".into(),
            version: env!("CARGO_PKG_VERSION").into(),
            ..Default::default()
        }
    }
}
```

**Step 2: Implement main.rs**

```rust
// src/main.rs
mod auth;
mod bridge;
mod sanitize;
mod server;
mod tools;

use anyhow::Context;
use bridge::Bridge;
use server::ThunderbirdMcp;
use std::sync::Arc;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Read auth token written by the Thunderbird extension on startup
    let token = auth::find_token()
        .context("Is Thunderbird running with the MCP extension installed?")?;

    let bridge = Arc::new(Bridge::new(token));
    let handler = ThunderbirdMcp { bridge };

    // Start MCP server on stdio (Claude connects via stdin/stdout)
    // NOTE: Adjust the serve call to match the rmcp API in your Cargo.lock.
    // Likely: rmcp::serve_server(handler, tokio::io::stdin(), tokio::io::stdout()).await
    //   or:   handler.serve(rmcp::transport::stdio()).await
    // Run: cargo doc --open -p rmcp  for the exact API.
    rmcp::serve_server(handler, tokio::io::stdin(), tokio::io::stdout())
        .await
        .context("MCP server error")?;

    Ok(())
}
```

**Step 3: Build in release mode**

```bash
cargo build --release 2>&1 | tail -5
```

Expected: `Finished release [optimized] target(s)`. Fix any compile errors before continuing (most likely macro name mismatches — consult `cargo doc --open -p rmcp`).

**Step 4: Smoke test — tools/list**

```bash
echo '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"test","version":"0"}}}' \
  | ./target/release/thunderbird-mcp
```

Expected: JSON response with `serverInfo.name = "thunderbird-mcp"` and `capabilities.tools`.

```bash
printf '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"test","version":"0"}}}\n{"jsonrpc":"2.0","id":2,"method":"tools/list"}\n' \
  | ./target/release/thunderbird-mcp
```

Expected: second response lists all 20 tool names.

**Step 5: Commit**

```bash
git add src/server.rs src/main.rs
git commit -m "feat: wire rmcp server with all 20 tools registered"
```

---

## Task 10: Rewrite Thunderbird extension as thin XPCOM shim

**Files:**
- Modify: `extension/mcp_server/api.js`
- Keep unchanged: `extension/manifest.json`, `extension/background.js`, `extension/httpd.sys.mjs`, `extension/mcp_server/schema.json`

Replace the ~2500-line `api.js` with a ~400-line path-based XPCOM adapter. The structure below preserves every XPCOM call from the old file — only the surrounding MCP/JSON-RPC plumbing is removed.

**Step 1: Read the old api.js XPCOM calls**

The old file's XPCOM bodies (lines 464–2394) are the only thing being kept. The skeleton below references them. Extract each function body from the old file:

```bash
# Quick reference: what functions exist
grep -n "const [a-z]" /home/marius/work/claude/thunderbird-mcp-old/extension/mcp_server/api.js | head -40
```

**Step 2: Write the new api.js skeleton**

The full replacement follows. XPCOM bodies marked `[COPY FROM OLD api.js lines N-M]` — copy the function body verbatim from the old file. The old file is at `/home/marius/work/claude/thunderbird-mcp-old/extension/mcp_server/api.js`.

```javascript
// extension/mcp_server/api.js — thin XPCOM adapter
// All MCP logic lives in the Rust binary. This file: XPCOM glue only.

/* globals ChromeUtils, Components, Services */

const { MailServices } = ChromeUtils.import("resource:///modules/MailServices.jsm");
const { cal } = ChromeUtils.importESModule("resource:///modules/calendar/calUtils.sys.mjs");
const { NetUtil } = ChromeUtils.import("resource://gre/modules/NetUtil.jsm");

const Cc = Components.classes;
const Ci = Components.interfaces;
const Cu = Components.utils;

const MCP_PORT = 8765;
const AUTH_TOKEN_FILENAME = ".thunderbird-mcp-auth";
const DEFAULT_MAX_RESULTS = 20;
const MAX_SEARCH_RESULTS_CAP = 100;
const SEARCH_COLLECTION_CAP = 500;

// [COPY resProto and extensionRoot setup from old api.js lines 16-30]

var mcpServer = {
  getAPI(context) {
    let authToken = "";
    let server = null;

    // --- Auth token generation (nsIRandomGenerator, NOT Web Crypto) ---
    function generateToken() {
      const rng = Cc["@mozilla.org/security/random-generator;1"]
        .getService(Ci.nsIRandomGenerator);
      const bytes = rng.generateRandomBytes(32);
      return Array.from(bytes).map(b => b.toString(16).padStart(2, "0")).join("");
    }

    function writeTokenToFile(token) {
      // [COPY writeTokenToFile body from old api.js — uses nsIFile + NetUtil]
    }

    // --- HTTP utilities (unchanged from old api.js) ---
    function readRequestBody(request) {
      // [COPY readRequestBody from old api.js — must use charset: "UTF-8"]
    }

    function respond(response, status, obj) {
      response.setStatusLine(null, status, status === 200 ? "OK" : "Error");
      response.setHeader("Content-Type", "application/json; charset=utf-8", false);
      response.setHeader("Access-Control-Allow-Origin", "null", false);
      const body = JSON.stringify(obj);
      response.bodyOutputStream.write(body, body.length);
    }

    // --- XPCOM helpers (copy from old api.js verbatim) ---
    function findTrashFolder(folder) {
      // [COPY from old api.js]
    }

    function stripHtml(html) {
      // [COPY from old api.js]
    }

    function openFolder(folder) {
      // [COPY from old api.js — returns { isImap }]
    }

    // --- Route handlers — one per operation, pure XPCOM ---
    // NOTE: No sanitizeForJson() calls — sanitization moved to Rust.
    // NOTE: No MCP envelope — return raw result objects.

    async function listAccounts() {
      // [COPY body from old api.js listAccounts handler]
      // Remove: sanitizeForJson() calls
      // Keep: all MailServices.accounts XPCOM calls
      // Return: { accounts: [...] }
    }

    async function listFolders({ account_id, folder_uri }) {
      // [COPY body from old api.js listFolders handler]
    }

    async function searchMessages(params) {
      // [COPY body from old api.js searchMessages handler]
      // Return: { messages: [...], imap_sync_pending: bool }
    }

    async function getMessage({ message_id, save_attachments }) {
      // [COPY body from old api.js getMessage handler]
    }

    async function getRecentMessages(params) {
      // [COPY body from old api.js getRecentMessages handler]
    }

    async function updateMessage(params) {
      // [COPY body from old api.js updateMessage handler]
    }

    async function deleteMessages({ message_ids }) {
      // [COPY body from old api.js deleteMessages handler]
    }

    async function createFolder({ parent_uri, name }) {
      // [COPY body from old api.js createFolder handler]
    }

    async function sendMail(params) {
      // [COPY body from old api.js sendMail handler]
    }

    async function replyToMessage(params) {
      // [COPY body from old api.js replyToMessage handler]
    }

    async function forwardMessage(params) {
      // [COPY body from old api.js forwardMessage handler]
    }

    async function listFilters({ account_id }) {
      // [COPY body from old api.js listFilters handler]
    }

    async function createFilter(params) {
      // [COPY body from old api.js createFilter handler]
    }

    async function updateFilter(params) {
      // [COPY body from old api.js updateFilter handler]
    }

    async function deleteFilter(params) {
      // [COPY body from old api.js deleteFilter handler]
    }

    async function reorderFilters(params) {
      // [COPY body from old api.js reorderFilters handler]
    }

    async function applyFilters(params) {
      // [COPY body from old api.js applyFilters handler]
    }

    async function searchContacts({ query, limit }) {
      // [COPY body from old api.js searchContacts handler]
      // Remove: sanitizeForJson() calls (Rust sanitizes now)
    }

    async function listCalendars() {
      // [COPY body from old api.js listCalendars handler]
    }

    async function createEvent(params) {
      // [COPY body from old api.js createEvent handler]
    }

    // --- Path router ---
    const ROUTES = {
      "/accounts/list":         listAccounts,
      "/folders/list":          listFolders,
      "/messages/search":       searchMessages,
      "/messages/get":          getMessage,
      "/messages/recent":       getRecentMessages,
      "/messages/update":       updateMessage,
      "/messages/delete":       deleteMessages,
      "/folders/create":        createFolder,
      "/mail/send":             sendMail,
      "/mail/reply":            replyToMessage,
      "/mail/forward":          forwardMessage,
      "/filters/list":          listFilters,
      "/filters/create":        createFilter,
      "/filters/update":        updateFilter,
      "/filters/delete":        deleteFilter,
      "/filters/reorder":       reorderFilters,
      "/filters/apply":         applyFilters,
      "/contacts/search":       searchContacts,
      "/calendars/list":        listCalendars,
      "/calendar/create-event": createEvent,
    };

    // --- HTTP handler ---
    async function handleRequest(request, response) {
      // Auth check
      const authHeader = request.getHeader("Authorization") || "";
      const token = authHeader.startsWith("Bearer ") ? authHeader.slice(7) : "";
      if (!authToken || token !== authToken) {
        respond(response, 401, { error: "Unauthorized" });
        return;
      }

      const handler = ROUTES[request.path];
      if (!handler) {
        respond(response, 404, { error: `Unknown operation: ${request.path}` });
        return;
      }

      let params = {};
      try {
        const bodyStr = await readRequestBody(request);
        if (bodyStr.trim()) params = JSON.parse(bodyStr);
      } catch (e) {
        respond(response, 400, { error: `Invalid JSON body: ${e.message}` });
        return;
      }

      try {
        const result = await handler(params);
        respond(response, 200, result);
      } catch (e) {
        console.error(`[thunderbird-mcp] ${request.path} error:`, e);
        respond(response, 500, { error: e.message || "Internal error" });
      }
    }

    return {
      mcpServer: {
        async start() {
          if (globalThis.__tbMcpServer) {
            return { success: true, port: MCP_PORT };
          }
          try {
            authToken = generateToken();
            writeTokenToFile(authToken);

            // [COPY httpd.sys.mjs import and server start from old api.js lines 363-462]
            // Register handleRequest as the HTTP handler
            // Store server reference in globalThis.__tbMcpServer

            return { success: true, port: MCP_PORT };
          } catch (e) {
            return { success: false, error: e.message };
          }
        }
      }
    };
  },

  onShutdown(isAppShutdown) {
    // Do NOT delete the auth token file — see gotchas.md (extension update race)
    if (globalThis.__tbMcpServer) {
      try { globalThis.__tbMcpServer.stop(); } catch (_) {}
      globalThis.__tbMcpServer = null;
    }
  }
};
```

**Step 3: Test each endpoint with curl**

Thunderbird must be running with the new extension installed. Read the token:

```bash
TOKEN=$(cat ~/.thunderbird-mcp-auth 2>/dev/null || cat ~/snap/thunderbird/common/.thunderbird-mcp-auth)

# Test listAccounts
curl -s -X POST http://localhost:8765/accounts/list \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer $TOKEN" \
  -d '{}' | python3 -m json.tool

# Test searchMessages
curl -s -X POST http://localhost:8765/messages/search \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer $TOKEN" \
  -d '{"query": "test", "max_results": 5}' | python3 -m json.tool

# Test auth rejection
curl -s -X POST http://localhost:8765/accounts/list \
  -H "Authorization: Bearer wrong-token" \
  -d '{}' | python3 -m json.tool
# Expected: {"error":"Unauthorized"}
```

**Step 4: Test all 20 endpoints** — repeat the curl pattern for each route in `ROUTES`. Verify each returns JSON (not an error).

**Step 5: Commit**

```bash
git add extension/mcp_server/api.js
git commit -m "refactor: slim extension to pure XPCOM adapter (~400 lines)"
```

---

## Task 11: End-to-end integration test

**Files:**
- Create: `scripts/test-e2e.sh`

**Step 1: Write the test script**

```bash
#!/usr/bin/env bash
# scripts/test-e2e.sh — end-to-end test of the Rust MCP binary against Thunderbird
set -euo pipefail

BINARY=./target/release/thunderbird-mcp

if [[ ! -f "$BINARY" ]]; then
  echo "Build first: cargo build --release"
  exit 1
fi

send_request() {
  local json="$1"
  echo "$json"
}

run_session() {
  (
    send_request '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"test","version":"0"}}}'
    send_request '{"jsonrpc":"2.0","id":2,"method":"notifications/initialized"}'
    send_request '{"jsonrpc":"2.0","id":3,"method":"tools/list"}'
    send_request '{"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"list_accounts","arguments":{}}}'
    send_request '{"jsonrpc":"2.0","id":5,"method":"tools/call","params":{"name":"list_calendars","arguments":{}}}'
  ) | "$BINARY"
}

echo "=== Running end-to-end test ==="
OUTPUT=$(run_session)
echo "$OUTPUT"

# Verify tools/list returns 20 tools
TOOL_COUNT=$(echo "$OUTPUT" | python3 -c "
import sys, json
lines = [l for l in sys.stdin.read().splitlines() if l.strip()]
for line in lines:
    obj = json.loads(line)
    if obj.get('id') == 3:
        print(len(obj['result']['tools']))
        break
" 2>/dev/null || echo "0")

echo ""
echo "Tool count: $TOOL_COUNT (expected 20)"
if [[ "$TOOL_COUNT" == "20" ]]; then
  echo "PASS: all 20 tools registered"
else
  echo "FAIL: expected 20 tools, got $TOOL_COUNT"
  exit 1
fi

# Verify list_accounts returned content (not error)
if echo "$OUTPUT" | grep -q '"isError":true'; then
  echo "FAIL: list_accounts returned an error"
  exit 1
fi

echo "PASS: end-to-end test complete"
```

**Step 2: Make it executable and run it**

```bash
chmod +x scripts/test-e2e.sh
./scripts/test-e2e.sh
```

Expected: `PASS: all 20 tools registered` + `PASS: end-to-end test complete`

**Step 3: Update MCP client config**

In your Claude Code config (replace the old Node.js entry):

```json
{
  "mcpServers": {
    "thunderbird-mail": {
      "command": "/home/marius/work/claude/thunderbird-mcp/target/release/thunderbird-mcp"
    }
  }
}
```

Reconnect: `/mcp` in Claude Code.

**Step 4: Commit**

```bash
git add scripts/test-e2e.sh
git commit -m "test: end-to-end integration test script"
```

---

## Completion Checklist

- [ ] `cargo build --release` succeeds
- [ ] `cargo test` — all unit tests pass (auth, sanitize, bridge)
- [ ] `tools/list` response contains all 20 tools
- [ ] All 20 extension endpoints respond correctly to curl
- [ ] End-to-end test script passes
- [ ] MCP client config updated to point at Rust binary
- [ ] `/mcp` reconnect in Claude Code — tools visible

---

## Key Reference

- Old project: `/home/marius/work/claude/thunderbird-mcp-old/extension/mcp_server/api.js`
- Design doc: `docs/plans/2026-02-27-thunderbird-mcp-rust-design.md`
- Gotchas: see design doc section "Key Gotchas Carried Forward"
- rmcp docs: `cargo doc --open -p rmcp` after `cargo build`
