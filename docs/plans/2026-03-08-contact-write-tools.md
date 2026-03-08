# Contact Write Tools Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add three new MCP tools — `list_address_books`, `create_contact`, and `update_contact` — giving AI clients full write access to Thunderbird's address books.

**Architecture:** Three new Rust wrapper functions in `src/tools/contacts.rs`, three `#[tool]` methods in `src/server.rs`, three JS functions + ROUTES entries in `extension/mcp_server/api.js`. All follow the existing thin-wrapper pattern used by `search_contacts`.

**Tech Stack:** Rust (rmcp, serde_json, mockito for tests), JavaScript (Thunderbird XPCOM: `MailServices.ab`, `nsIAbCard`)

---

## Background: How the codebase is organized

- `src/tools/contacts.rs` — thin Rust async wrappers; each calls `bridge.call("/route", json!({...}))` and returns `result_text(r)`. Tests live in the same file's `#[cfg(test)]` block.
- `src/server.rs` — `ThunderbirdMcp` impl; each tool is an `async fn` with `#[tool(description = "...")]` attribute. The `search_contacts` method is at lines 330–340.
- `extension/mcp_server/api.js` — JS functions + ROUTES table around line 1714. The `/contacts/search` route is at line 1714.

---

### Task 1: Rust wrappers + tests in `src/tools/contacts.rs`

**Files:**
- Modify: `src/tools/contacts.rs`

**Step 1: Write six failing tests**

Add after the last test (`list_events_sends_date_range_and_limit`, currently ending at line 121), inside the existing `#[cfg(test)]` block:

```rust
    #[tokio::test]
    async fn list_address_books_hits_correct_endpoint() {
        let mut server = Server::new_async().await;
        let mock = server.mock("POST", "/contacts/list-books")
            .with_status(200).with_body("[]").create_async().await;
        list_address_books(&mock_bridge(&server).await).await.unwrap();
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn list_address_books_propagates_bridge_error() {
        let mut server = Server::new_async().await;
        let _mock = server.mock("POST", "/contacts/list-books")
            .with_status(200)
            .with_body("{\"error\": \"unavailable\"}")
            .create_async().await;
        let result = list_address_books(&mock_bridge(&server).await).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn create_contact_sends_required_fields() {
        let mut server = Server::new_async().await;
        let mock = server.mock("POST", "/contacts/create")
            .match_body(mockito::Matcher::PartialJson(json!({
                "address_book_uri": "moz-abmdbdirectory://abook.mab",
                "email": "alice@example.com"
            })))
            .with_status(200).with_body("{}").create_async().await;
        create_contact(
            &mock_bridge(&server).await,
            "moz-abmdbdirectory://abook.mab".into(),
            "alice@example.com".into(),
            None, None, None,
        ).await.unwrap();
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn create_contact_propagates_bridge_error() {
        let mut server = Server::new_async().await;
        let _mock = server.mock("POST", "/contacts/create")
            .with_status(200)
            .with_body("{\"error\": \"address book not found\"}")
            .create_async().await;
        let result = create_contact(
            &mock_bridge(&server).await,
            "moz-abmdbdirectory://abook.mab".into(),
            "alice@example.com".into(),
            None, None, None,
        ).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn update_contact_sends_contact_id_and_fields() {
        let mut server = Server::new_async().await;
        let mock = server.mock("POST", "/contacts/update")
            .match_body(mockito::Matcher::PartialJson(json!({
                "contact_id": "uid-123",
                "email": "new@example.com"
            })))
            .with_status(200).with_body("{}").create_async().await;
        update_contact(
            &mock_bridge(&server).await,
            "uid-123".into(),
            Some("new@example.com".into()),
            None, None, None,
        ).await.unwrap();
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn update_contact_propagates_bridge_error() {
        let mut server = Server::new_async().await;
        let _mock = server.mock("POST", "/contacts/update")
            .with_status(200)
            .with_body("{\"error\": \"contact not found\"}")
            .create_async().await;
        let result = update_contact(
            &mock_bridge(&server).await,
            "uid-123".into(),
            None, None, None, None,
        ).await;
        assert!(result.is_err());
    }
```

**Step 2: Run tests — expect FAIL (functions not defined yet)**

```bash
cargo test list_address_books create_contact update_contact -- --nocapture 2>&1 | head -20
```
Expected: compile errors referencing undefined functions.

**Step 3: Add the three Rust wrapper functions**

Add after `search_contacts` (after line 15), before `list_calendars`:

```rust
pub async fn list_address_books(bridge: &Bridge) -> Result<CallToolResult, McpError> {
    let r = bridge.call("/contacts/list-books", json!({})).await.map_err(bridge_err)?;
    Ok(result_text(r))
}

pub async fn create_contact(
    bridge: &Bridge,
    address_book_uri: String,
    email: String,
    first_name: Option<String>,
    last_name: Option<String>,
    display_name: Option<String>,
) -> Result<CallToolResult, McpError> {
    let r = bridge.call("/contacts/create", json!({
        "address_book_uri": address_book_uri,
        "email": email,
        "first_name": first_name,
        "last_name": last_name,
        "display_name": display_name,
    })).await.map_err(bridge_err)?;
    Ok(result_text(r))
}

pub async fn update_contact(
    bridge: &Bridge,
    contact_id: String,
    email: Option<String>,
    first_name: Option<String>,
    last_name: Option<String>,
    display_name: Option<String>,
) -> Result<CallToolResult, McpError> {
    let r = bridge.call("/contacts/update", json!({
        "contact_id": contact_id,
        "email": email,
        "first_name": first_name,
        "last_name": last_name,
        "display_name": display_name,
    })).await.map_err(bridge_err)?;
    Ok(result_text(r))
}
```

**Step 4: Run tests — expect all 6 PASS**

```bash
cargo test list_address_books create_contact update_contact -- --nocapture
```
Expected: 6 tests PASS

**Step 5: Run full suite**

```bash
cargo test
```
Expected: all tests PASS (was 58, now 64)

**Step 6: Commit**

```bash
git add src/tools/contacts.rs
git commit -m "feat: add list_address_books, create_contact, update_contact Rust wrappers"
```

---

### Task 2: MCP tool registration in `src/server.rs`

**Files:**
- Modify: `src/server.rs` (after `search_contacts` method at line 340)

**Step 1: Add three `#[tool]` methods after `search_contacts`**

```rust
    #[tool(description = "List all address books with their URIs, names, and read-only status")]
    async fn list_address_books(&self) -> Result<CallToolResult, McpError> {
        contacts::list_address_books(&self.bridge).await
    }

    #[tool(description = "Create a new contact in an address book")]
    async fn create_contact(
        &self,
        #[tool(param)]
        #[schemars(description = "URI of the address book to add the contact to (from list_address_books)")]
        address_book_uri: String,
        #[tool(param)]
        #[schemars(description = "Contact email address")]
        email: String,
        #[tool(param)]
        #[schemars(description = "First name")]
        first_name: Option<String>,
        #[tool(param)]
        #[schemars(description = "Last name")]
        last_name: Option<String>,
        #[tool(param)]
        #[schemars(description = "Display name (auto-derived from first/last name or email if omitted)")]
        display_name: Option<String>,
    ) -> Result<CallToolResult, McpError> {
        contacts::create_contact(&self.bridge, address_book_uri, email, first_name, last_name, display_name).await
    }

    #[tool(description = "Update fields on an existing contact")]
    async fn update_contact(
        &self,
        #[tool(param)]
        #[schemars(description = "Contact UID from search_contacts results")]
        contact_id: String,
        #[tool(param)]
        #[schemars(description = "New email address")]
        email: Option<String>,
        #[tool(param)]
        #[schemars(description = "New first name")]
        first_name: Option<String>,
        #[tool(param)]
        #[schemars(description = "New last name")]
        last_name: Option<String>,
        #[tool(param)]
        #[schemars(description = "New display name")]
        display_name: Option<String>,
    ) -> Result<CallToolResult, McpError> {
        contacts::update_contact(&self.bridge, contact_id, email, first_name, last_name, display_name).await
    }
```

**Step 2: Verify compile + tests**

```bash
cargo build 2>&1 | head -10
cargo test
```
Expected: clean build, all tests PASS

**Step 3: Commit**

```bash
git add src/server.rs
git commit -m "feat: register list_address_books, create_contact, update_contact MCP tools"
```

---

### Task 3: Extension functions + ROUTES in `extension/mcp_server/api.js`

**Files:**
- Modify: `extension/mcp_server/api.js`

**Step 1: Add three JS functions**

Add after `searchContacts` (around line 495, before `listCalendars`). Use `search_pattern("function listCalendars", path="extension/mcp_server/api.js", context_lines=2)` to find the exact insertion point, then `edit_file` to insert before it:

```js
            function listAddressBooks() {
              try {
                const books = [];
                for (const book of MailServices.ab.directories) {
                  books.push({ uri: book.URI, name: book.dirName, readOnly: book.readOnly });
                }
                return books;
              } catch (e) { return { error: e.toString() }; }
            }

            function createContact(addressBookUri, email, firstName, lastName, displayName) {
              try {
                if (typeof addressBookUri !== "string" || !addressBookUri) return { error: "address_book_uri must be a non-empty string" };
                if (typeof email !== "string" || !email) return { error: "email must be a non-empty string" };

                let book = null;
                for (const b of MailServices.ab.directories) {
                  if (b.URI === addressBookUri) { book = b; break; }
                }
                if (!book) return { error: `Address book not found: ${addressBookUri}` };

                const card = Cc["@mozilla.org/addressbook/cardproperty;1"].createInstance(Ci.nsIAbCard);
                card.primaryEmail = email;
                if (firstName) card.firstName = firstName;
                if (lastName) card.lastName = lastName;
                card.displayName = displayName || [firstName, lastName].filter(Boolean).join(" ") || email;

                const newCard = book.addCard(card);
                return { success: true, id: newCard.UID, displayName: newCard.displayName };
              } catch (e) { return { error: e.toString() }; }
            }

            function updateContact(contactId, email, firstName, lastName, displayName) {
              try {
                if (typeof contactId !== "string" || !contactId) return { error: "contact_id must be a non-empty string" };

                for (const book of MailServices.ab.directories) {
                  for (const card of book.childCards) {
                    if (card.UID === contactId) {
                      if (email !== undefined && email !== null) card.primaryEmail = email;
                      if (firstName !== undefined && firstName !== null) card.firstName = firstName;
                      if (lastName !== undefined && lastName !== null) card.lastName = lastName;
                      if (displayName !== undefined && displayName !== null) card.displayName = displayName;
                      book.modifyCard(card);
                      return { success: true, id: contactId };
                    }
                  }
                }
                return { error: `Contact not found: ${contactId}` };
              } catch (e) { return { error: e.toString() }; }
            }

```

**Step 2: Add ROUTES entries**

Find the line:
```js
              "/contacts/search":       async ({ query, limit }) => searchContacts(query, limit),
```

Replace with:
```js
              "/contacts/search":       async ({ query, limit }) => searchContacts(query, limit),
              "/contacts/list-books":   async () => listAddressBooks(),
              "/contacts/create":       async ({ address_book_uri, email, first_name, last_name, display_name }) => createContact(address_book_uri, email, first_name, last_name, display_name),
              "/contacts/update":       async ({ contact_id, email, first_name, last_name, display_name }) => updateContact(contact_id, email, first_name, last_name, display_name),
```

**Step 3: Syntax check**

```bash
node --check extension/mcp_server/api.js
```
Expected: no output (clean)

**Step 4: Full test suite**

```bash
cargo test
```
Expected: all tests PASS

**Step 5: Commit**

```bash
git add extension/mcp_server/api.js
git commit -m "feat: add listAddressBooks, createContact, updateContact extension endpoints"
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
- Some contacts have no `primaryEmail` set — these return `email: null`, not an empty string; this is a data quality issue
```

Replace with:
```
- Some contacts have no `primaryEmail` set — these return `email: null`, not an empty string; this is a data quality issue
- `list_address_books` returns `[{ uri, name, readOnly }]` — use `uri` as `address_book_uri` in `create_contact`
- `create_contact` requires `address_book_uri` — call `list_address_books` first to find the right book; returns `{ id, displayName }` for the new contact
- `update_contact` takes `contact_id` (UID) from `search_contacts` results; omit fields you don't want to change
```

**Step 2: Update `README.md`**

Find:
```
| **Contacts** | `search_contacts` |
```

Replace with:
```
| **Contacts** | `search_contacts`, `list_address_books`, `create_contact`, `update_contact` |
```

**Step 3: Update `ROADMAP.md`**

Find:
```
- [ ] **`create_contact` / `update_contact`** — write-side contact management
```

Replace with:
```
- [x] **`create_contact` / `update_contact`** — write-side contact management
- [x] **`list_address_books`** — list address books with URIs for use with create_contact
```

**Step 4: Final test run**

```bash
cargo test
```
Expected: all tests PASS

**Step 5: Commit**

```bash
git add docs/system_instructions.md README.md ROADMAP.md
git commit -m "docs: add list_address_books, create_contact, update_contact to docs"
```
