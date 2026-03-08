# delete_contact Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add a `delete_contact` MCP tool that permanently removes a contact by UID, with a `confirm: true` gate to prevent accidental deletion.

**Architecture:** One Rust wrapper in `src/tools/contacts.rs`, one `#[tool]` method in `src/server.rs`, one JS function + ROUTES entry in `extension/mcp_server/api.js`. Follows the exact same thin-wrapper pattern as `update_contact`.

**Tech Stack:** Rust (rmcp, serde_json, mockito for tests), JavaScript (Thunderbird XPCOM: `MailServices.ab`, `book.deleteCards`)

---

## Background: How the codebase is organized

- `src/tools/contacts.rs` — thin Rust async wrappers; each calls `bridge.call("/route", json!({...}))` and returns `result_text(r)`. Tests live in the same file's `#[cfg(test)]` block.
- `src/server.rs` — `ThunderbirdMcp` impl; each tool is an `async fn` with `#[tool(description = "...")]` attribute.
- `extension/mcp_server/api.js` — JS functions + ROUTES table. The `/contacts/update` route is the most recent contact tool to reference.

---

### Task 1: Rust wrapper + tests in `src/tools/contacts.rs`

**Files:**
- Modify: `src/tools/contacts.rs`

**Step 1: Write three failing tests**

Add after the last test (`update_contact_propagates_bridge_error`, currently ending around line 249), inside the existing `#[cfg(test)]` block (before the final closing `}`):

```rust
    #[tokio::test]
    async fn delete_contact_sends_contact_id_and_confirm() {
        let mut server = Server::new_async().await;
        let mock = server.mock("POST", "/contacts/delete")
            .match_body(mockito::Matcher::PartialJson(json!({
                "contact_id": "uid-123",
                "confirm": true
            })))
            .with_status(200).with_body("{}").create_async().await;
        delete_contact(
            &mock_bridge(&server).await,
            "uid-123".into(),
            true,
        ).await.unwrap();
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn delete_contact_dry_run_sends_confirm_false() {
        let mut server = Server::new_async().await;
        let mock = server.mock("POST", "/contacts/delete")
            .match_body(mockito::Matcher::PartialJson(json!({
                "contact_id": "uid-456",
                "confirm": false
            })))
            .with_status(200).with_body("{}").create_async().await;
        delete_contact(
            &mock_bridge(&server).await,
            "uid-456".into(),
            false,
        ).await.unwrap();
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn delete_contact_propagates_bridge_error() {
        let mut server = Server::new_async().await;
        let _mock = server.mock("POST", "/contacts/delete")
            .with_status(200)
            .with_body("{\"error\": \"contact not found\"}")
            .create_async().await;
        let result = delete_contact(
            &mock_bridge(&server).await,
            "uid-123".into(),
            true,
        ).await;
        assert!(result.is_err());
    }
```

**Step 2: Run tests — expect FAIL (function not defined yet)**

```bash
cargo test delete_contact -- --nocapture 2>&1 | head -20
```
Expected: compile error referencing undefined `delete_contact`.

**Step 3: Add the Rust wrapper function**

Add after `update_contact` (around line 50), before the `#[cfg(test)]` block:

```rust
pub async fn delete_contact(
    bridge: &Bridge,
    contact_id: String,
    confirm: bool,
) -> Result<CallToolResult, McpError> {
    let r = bridge.call("/contacts/delete", json!({
        "contact_id": contact_id,
        "confirm": confirm,
    })).await.map_err(bridge_err)?;
    Ok(result_text(r))
}
```

**Step 4: Run tests — expect all 3 PASS**

```bash
cargo test delete_contact -- --nocapture
```
Expected: 3 tests PASS.

**Step 5: Run full suite**

```bash
cargo test
```
Expected: all tests PASS (was 64, now 67).

**Step 6: Commit**

```bash
git add src/tools/contacts.rs
git commit -m "feat: add delete_contact Rust wrapper and tests"
```

---

### Task 2: MCP tool registration in `src/server.rs`

**Files:**
- Modify: `src/server.rs` (after `update_contact` method)

**Step 1: Add the `#[tool]` method after `update_contact`**

```rust
    #[tool(description = "Permanently delete a contact from the address book. Pass confirm=true to execute; omit or pass false to preview what would be deleted.")]
    async fn delete_contact(
        &self,
        #[tool(param)]
        #[schemars(description = "Contact UID from search_contacts results")]
        contact_id: String,
        #[tool(param)]
        #[schemars(description = "Pass true to confirm permanent deletion; pass false to preview the contact without deleting")]
        confirm: bool,
    ) -> Result<CallToolResult, McpError> {
        contacts::delete_contact(&self.bridge, contact_id, confirm).await
    }
```

**Step 2: Verify compile + tests**

```bash
cargo build 2>&1 | head -10
cargo test
```
Expected: clean build, all 67 tests PASS.

**Step 3: Commit**

```bash
git add src/server.rs
git commit -m "feat: register delete_contact MCP tool"
```

---

### Task 3: Extension function + ROUTES in `extension/mcp_server/api.js`

**Files:**
- Modify: `extension/mcp_server/api.js`

**Step 1: Find the insertion point**

Use `search_pattern("function listCalendars", path="extension/mcp_server/api.js", context_lines=2)` to find the exact line. Insert the new function BEFORE it.

**Step 2: Add the JS function**

Insert before `function listCalendars`:

```js
            function deleteContact(contactId, confirm) {
              try {
                if (typeof contactId !== "string" || !contactId) return { error: "contact_id must be a non-empty string" };

                for (const book of MailServices.ab.directories) {
                  for (const card of book.childCards) {
                    if (card.UID === contactId) {
                      if (confirm !== true) {
                        return { confirm_required: true, contact: { id: contactId, displayName: card.displayName, email: card.primaryEmail } };
                      }
                      book.deleteCards([card]);
                      return { success: true, id: contactId };
                    }
                  }
                }
                return { error: `Contact not found: ${contactId}` };
              } catch (e) { return { error: e.toString() }; }
            }

```

**Step 3: Add ROUTES entry**

Find the `/contacts/update` route line:
```js
              "/contacts/update":       async ({ contact_id, email, first_name, last_name, display_name }) => updateContact(contact_id, email, first_name, last_name, display_name),
```

Replace with:
```js
              "/contacts/update":       async ({ contact_id, email, first_name, last_name, display_name }) => updateContact(contact_id, email, first_name, last_name, display_name),
              "/contacts/delete":       async ({ contact_id, confirm }) => deleteContact(contact_id, confirm),
```

**Step 4: Syntax check**

```bash
node --check extension/mcp_server/api.js
```
Expected: no output (clean).

**Step 5: Full test suite**

```bash
cargo test
```
Expected: all 67 tests PASS.

**Step 6: Commit**

```bash
git add extension/mcp_server/api.js
git commit -m "feat: add deleteContact extension endpoint"
```

---

### Task 4: Update docs

**Files:**
- Modify: `docs/system_instructions.md`
- Modify: `README.md`
- Modify: `ROADMAP.md`

**Step 1: Update `docs/system_instructions.md`**

Find:
```
- `update_contact` takes `contact_id` (UID) from `search_contacts` results; omit fields you don't want to change
```

Replace with:
```
- `update_contact` takes `contact_id` (UID) from `search_contacts` results; omit fields you don't want to change
- `delete_contact` requires `confirm: true` to execute; pass `false` (or omit) to preview the contact details before committing to deletion — deletion is permanent
```

**Step 2: Update `README.md`**

Find:
```
| **Contacts** | `search_contacts`, `list_address_books`, `create_contact`, `update_contact` |
```

Replace with:
```
| **Contacts** | `search_contacts`, `list_address_books`, `create_contact`, `update_contact`, `delete_contact` |
```

**Step 3: Update `ROADMAP.md`**

Find:
```
- [ ] **`delete_contact`** — remove contacts from address book
```

Replace with:
```
- [x] **`delete_contact`** — remove contacts from address book (permanent; confirmation gate required)
```

**Step 4: Final test run**

```bash
cargo test
```
Expected: all 67 tests PASS.

**Step 5: Commit**

```bash
git add docs/system_instructions.md README.md ROADMAP.md
git commit -m "docs: add delete_contact to docs"
```
