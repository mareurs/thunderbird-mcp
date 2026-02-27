# Thunderbird MCP

Rust MCP server + slim Thunderbird extension that exposes 20 tools for email, contacts, calendar, and filters.

## Architecture

```
MCP Client (stdio) ──→ thunderbird-mcp (Rust binary) ──→ HTTP :8765 ──→ extension/mcp_server/api.js (Thunderbird XPCOM)
```

- **Rust binary** (`src/`) — owns the MCP surface (rmcp 0.1), auth token discovery, output sanitization
- **Extension** (`extension/`) — thin XPCOM adapter, 20 HTTP endpoints, no MCP knowledge
- **Port**: 8765, auth via Bearer token written to `~/.thunderbird-mcp-auth`

## Build & Run

```bash
cargo build --release          # build Rust binary
./scripts/build.sh             # package extension → dist/mcp-server.xpi

# Install extension: Thunderbird → Add-ons → Install from file → dist/mcp-server.xpi
# Restart Thunderbird, then:
./target/release/thunderbird-mcp   # run MCP server (reads token, connects stdio)
```

## Testing

```bash
# Smoke test extension HTTP directly
TOKEN=$(cat ~/.thunderbird-mcp-auth)
curl -s -X POST http://localhost:8765/accounts/list \
  -H "Authorization: Bearer $TOKEN" -H "Content-Type: application/json" -d '{}'

# Full MCP session test
cargo test
```

## Hard Rules

- **If you modify MCP tool schemas, action/condition formats, or extension logic — you MUST update `docs/system_instructions.md` and the `instructions` field in `.mcp.json` to reflect the change.**

## Known Issues

- **Control chars in raw HTTP responses** — Email subjects/bodies can contain control characters that make the raw HTTP response invalid JSON. Not a problem in production (Rust's `sanitize_str()` strips them before parsing), but direct `curl` tests against port 8765 will fail with `JSONDecodeError`. Fix: add sanitization in the extension before JSON serialization. **Partially fixed**: `get_recent_messages` and `search_messages` now sanitize subject/author/recipients in the extension; `get_message` body and other tools do not yet.

- **IMAP staleness** — `imapSyncPending: true` is returned when IMAP folders are involved. Message databases may lag behind the server until the user clicks the folder in Thunderbird. Not fixable without async sync; just retry.

- **`get_message` body quality** — HTML newsletters strip to plain text via `stripHtml()`, leaving `[image_url]` artifacts and losing formatting. Bodies can be large (5000+ chars for newsletters). No fix planned; plain text is the right default for AI consumption.

- **`search_contacts` missing emails** — Some contacts in the address book have no `primaryEmail` set and appear with an empty email field. This is a data quality issue in the address book, not a bug.
