# Contact Write Tools — Design

## Goal

Add three new MCP tools: `list_address_books`, `create_contact`, and `update_contact`.

## Architecture

Three new tool functions in `src/tools/contacts.rs`, three `#[tool]` methods in `src/server.rs`, three new JS functions + ROUTES entries in `extension/mcp_server/api.js`. All follow the existing thin-wrapper pattern.

## Tools

### `list_address_books`
- Route: `/contacts/list-books`
- Extension: iterate `MailServices.ab.directories`, return `[{ uri, name, readOnly }]`

### `create_contact`
- Route: `/contacts/create`
- Required params: `address_book_uri` (String), `email` (String)
- Optional params: `first_name`, `last_name`, `display_name`
- Extension: `MailServices.ab.getDirectory(uri)` → create `nsIAbCard` → set fields → `book.addCard(card)` → return `{ id, displayName }`
- `display_name` auto-derived from name fields if not provided; falls back to email

### `update_contact`
- Route: `/contacts/update`
- Required: `contact_id` (String — card UID from `search_contacts` results)
- Optional: `email`, `first_name`, `last_name`, `display_name`
- Extension: iterate `book.childCards` across all address books to find by UID → modify fields → `book.modifyCard(card)` → return `{ success: true, id }`

## Rust

Three functions added to `src/tools/contacts.rs`, three methods added to `src/server.rs`.

## Tests

Six tests in `src/tools/contacts.rs`:
- `list_address_books_hits_correct_endpoint` (param)
- `list_address_books_propagates_bridge_error` (error)
- `create_contact_sends_required_fields` (param)
- `create_contact_propagates_bridge_error` (error)
- `update_contact_sends_contact_id_and_fields` (param)
- `update_contact_propagates_bridge_error` (error)

## Docs

`docs/system_instructions.md` — add Contacts section notes:
- `list_address_books` returns URIs needed by `create_contact`
- `create_contact` requires `address_book_uri` — call `list_address_books` first
- `update_contact` takes `contact_id` from `search_contacts` results
