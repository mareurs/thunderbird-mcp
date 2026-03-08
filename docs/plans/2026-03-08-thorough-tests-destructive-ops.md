# Thorough Tests for Destructive Operations Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Fill parameter-marshaling gaps and add error-propagation tests for all destructive/mutating mail operations.

**Architecture:** All tests are inline `#[cfg(test)]` modules in the same file as the code under test, using the existing `mockito` mock HTTP server pattern. No new files. Error-path tests make the mock return `{"error": "..."}` in the body (HTTP 200) — the same way the real extension signals errors — and assert the tool fn returns `Err(_)`.

**Tech Stack:** Rust, tokio (async tests), mockito (mock HTTP server), serde_json `json!` macro.

---

## Background: Test Patterns Used in This Codebase

**Param test pattern** — verifies correct JSON is sent to the extension:
```rust
#[tokio::test]
async fn some_test() {
    let mut server = Server::new_async().await;
    let mock = server.mock("POST", "/endpoint")
        .match_body(mockito::Matcher::PartialJson(json!({ "key": "value" })))
        .with_status(200).with_body("{}").create_async().await;
    some_fn(&mock_bridge(&server).await, "value".into(), ...).await.unwrap();
    mock.assert_async().await;
}
```
`PartialJson` only checks the specified fields — other fields (including nulls) are ignored.

**Error-propagation test pattern** — verifies `BridgeError` becomes `McpError`:
```rust
#[tokio::test]
async fn some_fn_propagates_bridge_error() {
    let mut server = Server::new_async().await;
    let _mock = server.mock("POST", "/endpoint")
        .with_status(200)
        .with_body("{\"error\": \"something went wrong\"}")
        .create_async().await;
    let result = some_fn(&mock_bridge(&server).await, ...).await;
    assert!(result.is_err());
}
```
The extension always returns HTTP 200; errors are `{"error": "..."}` in the body. `Bridge::call` maps this to `BridgeError::ExtensionError`, and `bridge_err` maps that to `McpError`.

---

### Task 1: `update_message` — move_to and trash param tests

**Files:**
- Modify: `src/tools/mail.rs` (add to `tests` module, after line 201)

**Step 1: Write the two failing tests**

Add these two tests inside the `#[cfg(test)]` block in `src/tools/mail.rs`, after `update_message_sends_read_and_flagged`:

```rust
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
```

**Step 2: Run tests to verify they pass**

```bash
cargo test update_message -- --nocapture
```
Expected: all `update_message_*` tests PASS. If any fail, the `move_to`/`trash` fields are not being sent — check the `json!({})` body in `update_message` in `src/tools/mail.rs`.

**Step 3: Commit**

```bash
git add src/tools/mail.rs
git commit -m "test: add update_message move_to and trash param tests"
```

---

### Task 2: `delete_messages_by_filter` — sparse filter test

**Files:**
- Modify: `src/tools/mail.rs` (add to `tests` module, after `delete_messages_by_filter_sends_folder_and_filters`)

**Step 1: Write the failing test**

Add after `delete_messages_by_filter_sends_folder_and_filters` (after line 228):

```rust
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
```

**Step 2: Run test**

```bash
cargo test delete_messages_by_filter -- --nocapture
```
Expected: PASS. This confirms the function works when all optional filters are `None`.

**Step 3: Commit**

```bash
git add src/tools/mail.rs
git commit -m "test: add delete_messages_by_filter sparse-filter test"
```

---

### Task 3: Error propagation tests for `mail.rs` destructive ops

**Files:**
- Modify: `src/tools/mail.rs` (add to `tests` module)

**Step 1: Write three error-propagation tests**

Add after all existing tests in the `mail.rs` `#[cfg(test)]` block:

```rust
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
```

**Step 2: Run tests**

```bash
cargo test propagates_bridge_error -- --nocapture
```
Expected: all three PASS.

**Step 3: Commit**

```bash
git add src/tools/mail.rs
git commit -m "test: add error propagation tests for destructive mail ops"
```

---

### Task 4: `compose.rs` param gap tests (cc/bcc, reply_all)

**Files:**
- Modify: `src/tools/compose.rs` (add to `tests` module)

**Step 1: Write two param tests**

Add after `reply_to_message_sends_message_id_and_body` in `compose.rs`:

```rust
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
```

**Step 2: Run tests**

```bash
cargo test send_mail_sends_optional_cc_and_bcc reply_to_message_sends_reply_all -- --nocapture
```
Expected: both PASS.

**Step 3: Commit**

```bash
git add src/tools/compose.rs
git commit -m "test: add cc/bcc and reply_all param tests for compose"
```

---

### Task 5: Error propagation tests for `compose.rs`

**Files:**
- Modify: `src/tools/compose.rs` (add to `tests` module)

**Step 1: Write three error-propagation tests**

Add after all existing tests in the `compose.rs` `#[cfg(test)]` block:

```rust
    #[tokio::test]
    async fn send_mail_propagates_bridge_error() {
        let mut server = Server::new_async().await;
        let _mock = server.mock("POST", "/mail/send")
            .with_status(200)
            .with_body("{\"error\": \"smtp failure\"}")
            .create_async().await;
        let result = send_mail(
            &mock_bridge(&server).await,
            vec!["alice@example.com".into()],
            "Hi".into(), "Hello".into(), None, None, None,
        ).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn reply_to_message_propagates_bridge_error() {
        let mut server = Server::new_async().await;
        let _mock = server.mock("POST", "/mail/reply")
            .with_status(200)
            .with_body("{\"error\": \"message not found\"}")
            .create_async().await;
        let result = reply_to_message(
            &mock_bridge(&server).await,
            "msg-1".into(), "Thanks!".into(), None,
        ).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn forward_message_propagates_bridge_error() {
        let mut server = Server::new_async().await;
        let _mock = server.mock("POST", "/mail/forward")
            .with_status(200)
            .with_body("{\"error\": \"message not found\"}")
            .create_async().await;
        let result = forward_message(
            &mock_bridge(&server).await,
            "msg-1".into(),
            vec!["bob@example.com".into()],
            None,
        ).await;
        assert!(result.is_err());
    }
```

**Step 2: Run tests**

```bash
cargo test propagates_bridge_error -- --nocapture
```
Expected: all six error propagation tests (3 mail + 3 compose) PASS.

**Step 3: Final full test run**

```bash
cargo test
```
Expected: all tests PASS, no regressions.

**Step 4: Commit**

```bash
git add src/tools/compose.rs
git commit -m "test: add error propagation tests for compose ops"
```
