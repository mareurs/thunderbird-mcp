# delete_folder Tool Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add a `delete_folder` MCP tool that moves a Thunderbird folder to Trash via `propagateDelete(folder, false, null)`.

**Architecture:** Follows the identical pattern as `create_folder` — Rust `#[tool]` method in `src/server.rs` → thin async wrapper in `src/tools/mail.rs` → `Bridge::call("/folders/delete", ...)` → `deleteFolder()` in `extension/mcp_server/api.js`. Tests live inline in `src/tools/mail.rs` using the existing mockito pattern.

**Tech Stack:** Rust (rmcp, serde_json, mockito for tests), JavaScript (Thunderbird XPCOM `MailServices.folderLookup`, `nsIMsgFolder.propagateDelete`)

---

### Task 1: Tests + Rust wrapper in `src/tools/mail.rs`

**Files:**
- Modify: `src/tools/mail.rs`

**Step 1: Write the two failing tests**

Add these tests inside the `#[cfg(test)]` block in `src/tools/mail.rs`, after `create_folder_sends_parent_and_name` (currently the last test):

```rust
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
```

**Step 2: Run tests to verify they fail**

```bash
cargo test delete_folder -- --nocapture
```
Expected: FAIL with `cannot find function delete_folder`

**Step 3: Add the `delete_folder` function**

Add this function to `src/tools/mail.rs` after `create_folder` (after line 128):

```rust
pub async fn delete_folder(
    bridge: &Bridge,
    folder_uri: String,
) -> Result<CallToolResult, McpError> {
    let r = bridge.call("/folders/delete", json!({
        "folder_uri": folder_uri
    })).await.map_err(bridge_err)?;
    Ok(result_text(r))
}
```

**Step 4: Run tests to verify they pass**

```bash
cargo test delete_folder -- --nocapture
```
Expected: 2 tests PASS

**Step 5: Commit**

```bash
git add src/tools/mail.rs
git commit -m "feat: add delete_folder Rust wrapper and tests"
```

---

### Task 2: MCP tool registration in `src/server.rs`

**Files:**
- Modify: `src/server.rs`

**Step 1: Add the tool method**

In `src/server.rs`, add after the `create_folder` method (after line 161):

```rust
    #[tool(description = "Delete a folder by moving it to the account Trash")]
    async fn delete_folder(
        &self,
        #[tool(param)]
        #[schemars(description = "URI of the folder to delete (moves to Trash)")]
        folder_uri: String,
    ) -> Result<CallToolResult, McpError> {
        mail::delete_folder(&self.bridge, folder_uri).await
    }
```

**Step 2: Verify it compiles**

```bash
cargo build 2>&1 | head -20
```
Expected: no errors

**Step 3: Run full test suite**

```bash
cargo test
```
Expected: all tests PASS

**Step 4: Commit**

```bash
git add src/server.rs
git commit -m "feat: register delete_folder MCP tool in server"
```

---

### Task 3: Extension endpoint in `extension/mcp_server/api.js`

**Files:**
- Modify: `extension/mcp_server/api.js`

**Step 1: Add `deleteFolder` function**

In `api.js`, add the following function after `createFolder` (after line 1302, before the `// ── Filter constants` comment at line 1305):

```js
            function deleteFolder(folderUri) {
              try {
                if (typeof folderUri !== "string" || !folderUri) return { error: "folder_uri must be a non-empty string" };
                const folder = MailServices.folderLookup.getFolderForURL(folderUri);
                if (!folder) return { error: `Folder not found: ${folderUri}` };
                if (!folder.parent) return { error: "Cannot delete root folder" };
                folder.parent.propagateDelete(folder, false, null);
                return { success: true, message: "Folder moved to Trash" };
              } catch (e) {
                return { error: e.toString() };
              }
            }
```

**Step 2: Add the ROUTES entry**

In `api.js`, find the line:
```js
              "/folders/create":        async ({ parent_uri, name }) => createFolder(parent_uri, name),
```
(currently line 1685 — may shift after Step 1)

Add immediately after it:
```js
              "/folders/delete":        async ({ folder_uri }) => deleteFolder(folder_uri),
```

**Step 3: Verify no syntax errors**

```bash
node --check extension/mcp_server/api.js
```
Expected: no output (clean)

**Step 4: Commit**

```bash
git add extension/mcp_server/api.js
git commit -m "feat: add deleteFolder extension endpoint"
```

---

### Task 4: Update docs

**Files:**
- Modify: `docs/system_instructions.md`
- Modify: `README.md`
- Modify: `ROADMAP.md`

**Step 1: Update `docs/system_instructions.md`**

In the `## Folders` section (around line 22), add after the `create_folder` entries:
```
- `delete_folder` moves the folder to the account Trash — it does not permanently delete; to permanently remove, empty Trash afterward
```

**Step 2: Update `README.md`**

Find the Folders row in the features table (line 19):
```
| **Folders** | `list_accounts`, `list_folders`, `create_folder`, `delete_messages` |
```
Change to:
```
| **Folders** | `list_accounts`, `list_folders`, `create_folder`, `delete_folder`, `delete_messages` |
```

**Step 3: Update `ROADMAP.md`**

Find:
```
- [ ] **`delete_folder`** — remove a folder by URI
```
Change to:
```
- [x] **`delete_folder`** — remove a folder by URI (moves to Trash)
```

**Step 4: Run full test suite one final time**

```bash
cargo test
```
Expected: all tests PASS

**Step 5: Commit**

```bash
git add docs/system_instructions.md README.md ROADMAP.md
git commit -m "docs: add delete_folder to system instructions, README, and roadmap"
```
