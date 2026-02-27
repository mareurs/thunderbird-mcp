# Thunderbird MCP — Rust Rewrite Design

**Date:** 2026-02-27
**Status:** Approved

---

## Overview

Rewrite `thunderbird-mcp` in Rust. The Thunderbird extension (JS, mandatory — runs
inside Thunderbird's runtime and calls XPCOM APIs) is slimmed to a ~400-line pure
XPCOM adapter. All MCP protocol logic, schemas, validation, sanitization, and routing
move into a Rust binary that replaces the Node.js bridge.

---

## Architecture

```
MCP Client (Claude, etc.)
       │  stdio — JSON-RPC 2.0, one line per message
       ▼
thunderbird-mcp  (Rust binary)
  ├── auth.rs        reads ~/.thunderbird-mcp-auth (+ snap fallback)
  ├── server.rs      registers 20 tools via rmcp, implements ServerHandler
  ├── tools/         one module per domain: mail, compose, filters, contacts
  │     each tool:   validate input types → call bridge → format MCP response
  ├── bridge.rs      reqwest HTTP client → localhost:8765, Bearer token on every call
  └── sanitize.rs    UTF-8 + control char cleaning
       │  HTTP — POST /domain/operation + JSON body + Authorization header
       ▼
Thunderbird Extension  (~400 lines JS, down from 2500)
  ├── manifest.json  unchanged
  ├── background.js  unchanged
  └── mcp_server/api.js  path-based routing, one handler per XPCOM operation
        startup:  nsIRandomGenerator → auth token → ~/.thunderbird-mcp-auth
        routing:  POST /accounts/list, /messages/search, /filters/create …
        handlers: call MailServices / cal / NetUtil → return raw JSON result
```

**Key boundary:** the extension has zero MCP awareness. It exposes raw XPCOM
results as plain JSON. Rust owns the entire MCP surface.

---

## Rust Crate

### Cargo.toml dependencies

```toml
[dependencies]
rmcp       = { version = "0.1", features = ["server", "transport-io"] }
tokio      = { version = "1",   features = ["full"] }
reqwest    = { version = "0.12", features = ["json"] }
serde      = { version = "1",   features = ["derive"] }
serde_json = "1"
thiserror  = "2"
anyhow     = "1"
dirs       = "5"
```

### Module layout

```
src/
├── main.rs         tokio entry: read auth token, start rmcp server on stdio
├── server.rs       ThunderbirdMcp struct — implements rmcp ServerHandler
├── bridge.rs       Bridge: reqwest client + call(resource, operation, params)
├── auth.rs         find_token() — checks home path then snap path
├── sanitize.rs     sanitize_str() — strips control chars, escapes raw newlines
└── tools/
    ├── mod.rs
    ├── mail.rs     listAccounts, listFolders, searchMessages, getMessage,
    │               getRecentMessages, updateMessage, deleteMessages, createFolder
    ├── compose.rs  sendMail, replyToMessage, forwardMessage
    ├── filters.rs  listFilters, createFilter, updateFilter, deleteFilter,
    │               reorderFilters, applyFilters
    └── contacts.rs searchContacts, listCalendars, createEvent
```

### Tool registration pattern

```rust
#[tool(tool_box)]
impl ThunderbirdMcp {
    #[tool(description = "List all email accounts and identities")]
    async fn list_accounts(&self) -> Result<CallToolResult, McpError> {
        self.bridge.call("accounts", "list", json!({})).await
    }

    #[tool(description = "Search messages by subject, sender, date range or folder")]
    async fn search_messages(
        &self,
        #[arg(description = "Text to search for")] query: Option<String>,
        #[arg(description = "Folder URI to search within")] folder: Option<String>,
        #[arg(description = "Filter by sender address")] sender: Option<String>,
        #[arg(description = "Max results (default 20, max 100)")] max_results: Option<u32>,
    ) -> Result<CallToolResult, McpError> {
        self.bridge.call("messages", "search", json!({
            "query": query, "folder": folder,
            "sender": sender, "max_results": max_results
        })).await
    }
    // ... 18 more tools
}
```

Tool schemas are derived from Rust function signatures at compile time — no
hand-written JSON Schema, no drift between schema and implementation.

---

## Extension Thin Shim

### Routing

Path-based dispatch replaces the old JSON-RPC `callTool()` switch:

```javascript
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
```

### Handler contract

Each handler receives a plain JSON object (parsed by Rust, forwarded as body),
returns a plain JSON object (no MCP envelope, no sanitization — Rust handles both):

```javascript
async function searchMessages({ query, folder, sender, max_results }) {
  // pure XPCOM calls
  return { messages: results, imap_sync_pending: isImap };
}
```

### Auth token generation

Uses `nsIRandomGenerator` — Web Crypto API is unavailable in `experiment_apis`
chrome scope. Same approach as the old project.

### What is removed from the extension

- `tools[]` schema array (~320 lines) → Rust structs
- JSON-RPC envelope handling → rmcp
- `callTool()` / `toolHandlers` dispatch → Rust bridge routing
- `sanitizeForJson()` → `sanitize.rs`
- `initialize` / `resources/list` / `prompts/list` responses → rmcp built-ins

---

## Security

| Concern | Approach |
|---|---|
| Auth | 32-byte random token via `nsIRandomGenerator`, written to `~/.thunderbird-mcp-auth` on startup |
| Transport | HTTP on localhost:8765 only (httpd.sys.mjs binds to "localhost") |
| Snap compatibility | `auth.rs` checks real home first, `~/snap/thunderbird/common/` second |
| Path traversal (attachments) | Block sensitive path prefixes — same logic as old project, now in Rust |
| Token cleanup | Do NOT delete token file on shutdown (race condition with extension update lifecycle) |

---

## Error Handling

```rust
#[derive(thiserror::Error, Debug)]
pub enum BridgeError {
    #[error("Thunderbird not reachable — is it running with the MCP extension?")]
    ConnectionFailed(#[from] reqwest::Error),
    #[error("Auth token not found — is Thunderbird running?")]
    NoAuthToken(#[from] AuthError),
    #[error("Extension error: {0}")]
    ExtensionError(String),
    #[error("Invalid response from extension")]
    InvalidJson(#[from] serde_json::Error),
}
```

- `thiserror` for `BridgeError` / `AuthError` (matchable variants)
- `anyhow` at `main.rs` level (setup errors, just propagate with context)
- Each tool maps `BridgeError` to an MCP tool error response with a human-readable message

---

## Testing

| Layer | Approach |
|---|---|
| `sanitize.rs` | Pure unit tests |
| `auth.rs` | Unit tests with `tempfile` crate |
| `bridge.rs` | Integration tests with `mockito` (mock HTTP server, fixture JSON) |
| Full tool call | `echo '...' \| cargo run` against live Thunderbird |
| Extension endpoints | `curl -X POST http://localhost:8765/accounts/list -H "Authorization: Bearer <token>" -d '{}'` |

Extension REST endpoints are independently curl-testable — no need to construct
a full MCP `tools/call` envelope to exercise a single XPCOM operation.

---

## Parity Checklist (20 tools)

### Mail
- [ ] listAccounts
- [ ] listFolders
- [ ] searchMessages
- [ ] getMessage
- [ ] getRecentMessages
- [ ] updateMessage
- [ ] deleteMessages
- [ ] createFolder

### Compose
- [ ] sendMail
- [ ] replyToMessage
- [ ] forwardMessage

### Filters
- [ ] listFilters
- [ ] createFilter
- [ ] updateFilter
- [ ] deleteFilter
- [ ] reorderFilters
- [ ] applyFilters

### Contacts & Calendar
- [ ] searchContacts
- [ ] listCalendars
- [ ] createEvent

---

## Key Gotchas Carried Forward

- `experiment_apis` chrome scope: no Web Crypto, no `fetch` — use XPCOM equivalents
- Snap `$HOME` remapping: `~/snap/thunderbird/common/` is the extension's home
- `/tmp` inside snap is private — do not use it for shared state
- IMAP staleness: `searchMessages` and `getRecentMessages` return `imap_sync_pending: true` flag when IMAP folders are involved
- No hot reload: after changing extension code, must fully quit and restart Thunderbird
- Do NOT delete `~/.thunderbird-mcp-auth` on shutdown — extension update lifecycle race

---

## Out of Scope (Phase 1)

- WebSocket IPC (Approach C) — future, enables push notifications
- New tools beyond the 20 existing ones
- Windows / macOS support (snap gotchas are Linux-specific; auth path logic may need extension)
