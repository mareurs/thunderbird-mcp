# apply_filters IMAP Warning Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add a `warning` field to `apply_filters` responses when the target folder is an IMAP folder, and add the missing error-propagation test for this tool.

**Architecture:** Change is entirely in `extension/mcp_server/api.js` — detect `folder.server.type === "imap"` after `applyFiltersToFolders` and spread a `warning` key into the success response. No Rust changes needed; the `warning` field passes through the JSON response transparently. One new Rust test covers the error path.

**Tech Stack:** JavaScript (Thunderbird XPCOM), Rust (mockito for tests)

---

### Task 1: Add error-propagation test for `apply_filters` in `src/tools/filters.rs`

**Files:**
- Modify: `src/tools/filters.rs` (add to `tests` module after `apply_filters_sends_account_and_folder` at line 172)

**Step 1: Write the test**

Add immediately after `apply_filters_sends_account_and_folder` (after line 172):

```rust
    #[tokio::test]
    async fn apply_filters_propagates_bridge_error() {
        let mut server = Server::new_async().await;
        let _mock = server.mock("POST", "/filters/apply")
            .with_status(200)
            .with_body("{\"error\": \"account not found\"}")
            .create_async().await;
        let result = apply_filters(
            &mock_bridge(&server).await,
            "acc".into(),
            "imap://host/INBOX".into(),
        ).await;
        assert!(result.is_err());
    }
```

**Step 2: Run test to verify it passes**

```bash
cargo test apply_filters -- --nocapture
```
Expected: both `apply_filters_*` tests PASS (this tests existing behavior, not new code)

**Step 3: Commit**

```bash
git add src/tools/filters.rs
git commit -m "test: add apply_filters error propagation test"
```

---

### Task 2: Add IMAP warning to `applyFilters` in `extension/mcp_server/api.js`

**Files:**
- Modify: `extension/mcp_server/api.js` (lines 1674–1679 — the success return block)
- Modify: `docs/system_instructions.md` (line 29 — the apply_filters bullet)

**Step 1: Replace the success return in `applyFilters`**

Find this exact block (the success return at the end of `applyFilters`):
```js
                return {
                  success: true,
                  message: "Filters applied (processing may take a moment)",
                  folder: folderUri,
                  enabledFilters: enabledCount,
                };
```

Replace with:
```js
                const isImap = folder.server && folder.server.type === "imap";
                return {
                  success: true,
                  message: "Filters applied (processing may take a moment)",
                  folder: folderUri,
                  enabledFilters: enabledCount,
                  ...(isImap ? { warning: "IMAP folder: filters only apply to locally cached messages; messages not yet downloaded are skipped" } : {}),
                };
```

Use codescout `edit_file("extension/mcp_server/api.js", old_string, new_string)` with the exact text above.

**Step 2: Update `docs/system_instructions.md`**

Find:
```
- `apply_filters` is async — the MCP response returns before messages are actually moved; wait a moment before verifying
```

Replace with:
```
- `apply_filters` is async — the MCP response returns before messages are actually moved; wait a moment before verifying
- `apply_filters` returns `warning` when the folder is IMAP — only locally cached messages are processed; messages not yet downloaded are skipped
```

**Step 3: Verify no syntax errors**

```bash
node --check extension/mcp_server/api.js
```
Expected: no output (clean)

**Step 4: Run full test suite**

```bash
cargo test
```
Expected: all tests PASS (57 Rust tests — no new Rust code changed here)

**Step 5: Commit**

```bash
git add extension/mcp_server/api.js docs/system_instructions.md
git commit -m "feat: add IMAP warning to apply_filters response"
```
