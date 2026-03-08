# delete_contact — Design

## Goal

Add a `delete_contact` MCP tool that permanently removes a contact from any address book, with a confirmation gate to prevent accidental deletion.

## Architecture

One new Rust wrapper in `src/tools/contacts.rs`, one `#[tool]` method in `src/server.rs`, one JS function + ROUTES entry in `extension/mcp_server/api.js`. Follows the existing thin-wrapper pattern.

## Tool

### `delete_contact`
- Route: `POST /contacts/delete`
- Required: `contact_id` (String — card UID from `search_contacts`)
- Required: `confirm` (bool)

**Behavior:**
- If `confirm != true`: find the contact, return `{ confirm_required: true, contact: { id, displayName, email } }` without deleting — lets AI show the user what would be deleted
- If `confirm == true`: find card by UID across all books, call `book.deleteCards([card])`, return `{ success: true, id }`
- If contact not found (either path): return `{ error: "Contact not found: <id>" }`

Note: deletion is permanent — Thunderbird address books have no Trash concept.

## Rust

One function in `src/tools/contacts.rs`:
```rust
pub async fn delete_contact(bridge: &Bridge, contact_id: String, confirm: bool) -> Result<CallToolResult, McpError>
```

One method in `src/server.rs` with `#[tool(param)]` + `#[schemars(description)]` on both params.

## Tests

Three tests in `src/tools/contacts.rs`:
- `delete_contact_sends_contact_id_and_confirm` — PartialJson matcher verifies both fields sent
- `delete_contact_dry_run_sends_confirm_false` — verifies confirm=false is sent correctly
- `delete_contact_propagates_bridge_error` — `{"error":"..."}` body → `result.is_err()`

## Docs

`docs/system_instructions.md` — add to Contacts section:
- `delete_contact` requires `confirm: true` to execute; omitting or passing `false` returns contact details for user confirmation before deleting
- Deletion is permanent — there is no undo
