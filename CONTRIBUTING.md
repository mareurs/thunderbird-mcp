# Contributing to thunderbird-mcp

## Development Setup

### Prerequisites

- Rust stable (`rustup install stable`)
- Thunderbird 115+ (as a snap or native install)
- A zip tool (`zip` CLI, used by `scripts/build.sh`)

### Build

```bash
cargo build                    # debug build
cargo build --release          # release build
./scripts/build.sh             # package extension → dist/mcp-server.xpi
```

### Install the extension for development

After every change to `extension/`:

```bash
./scripts/build.sh

# Copy directly into Thunderbird's profile (no reinstall dialog needed)
# Snap install:
cp dist/mcp-server.xpi \
  ~/snap/thunderbird/common/.thunderbird/<profile>.default/extensions/thunderbird-mcp@luthriel.dev.xpi

# Native install:
cp dist/mcp-server.xpi \
  ~/.thunderbird/<profile>.default/extensions/thunderbird-mcp@luthriel.dev.xpi
```

Find your profile name with `ls ~/snap/thunderbird/common/.thunderbird/` (snap) or `ls ~/.thunderbird/` (native).

Then restart Thunderbird.

> **Startup cache gotcha** — Thunderbird caches compiled JavaScript. If your extension changes are not picked up after a restart, clear the startup cache:
>
> ```bash
> # Snap:
> rm -rf ~/snap/thunderbird/common/.cache/thunderbird/<profile>/startupCache/
> # Native:
> rm -rf ~/.cache/thunderbird/<profile>/startupCache/
> ```
>
> Alternatively: Help → Troubleshooting Information → Clear Startup Cache → Restart.

### Run the MCP server manually

```bash
# Thunderbird must be running first
./target/release/thunderbird-mcp
```

The binary reads `~/.thunderbird-mcp-auth` for the token. If Thunderbird hasn't started yet, the file won't exist and the binary will exit with an error.

### Smoke test

```bash
TOKEN=$(cat ~/.thunderbird-mcp-auth)
curl -s -X POST http://localhost:8765/accounts/list \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{}'
```

> **Note:** Direct `curl` against port 8765 may fail with JSON errors on endpoints that return email content — control characters in message bodies/subjects are sanitized by the Rust layer, not the extension. This is expected; use the MCP interface in production.

### Unit tests

```bash
cargo test
```

## Code Structure

```
src/
  main.rs          — entry point, starts stdio MCP server
  auth.rs          — discovers ~/.thunderbird-mcp-auth token
  bridge.rs        — HTTP client (Bearer auth, JSON, error handling)
  sanitize.rs      — strips control characters from HTTP responses
  server.rs        — MCP tool registrations (rmcp #[tool] macros)
  tools/
    mail.rs        — get_recent_messages, search_messages, get_message, update_message
    compose.rs     — send_email, reply_to_message, forward_message
    filters.rs     — list/create/update/delete/apply filters
    contacts.rs    — search_contacts, list_calendars, list_events, create_event
                     (TODO: split calendar tools into calendar.rs)
    mod.rs         — re-exports

extension/
  manifest.json           — WebExtension manifest (MV2)
  background.js           — loads the experiment API
  mcp_server/
    schema.json           — JSON schema for the experiment API
    api.js                — all 20 HTTP endpoints (XPCOM implementation)
  httpd.sys.mjs           — Thunderbird's built-in HTTP server module

scripts/
  build.sh                — zips extension/ into dist/mcp-server.xpi

docs/
  system_instructions.md  — MCP tool quirks (keep in sync with README config example)
  plans/                  — architecture and implementation notes
```

## Adding a New Tool

1. **Extension** (`extension/mcp_server/api.js`):
   - Add a function implementing the XPCOM logic
   - Register a route in the handler map near line 1625

2. **Rust bridge** (`src/tools/<module>.rs`):
   - Add a `pub async fn` that calls `bridge.call("/route", json!({...}))` and returns `Ok(result_text(r))`

3. **MCP server** (`src/server.rs`):
   - Add a `#[tool(...)] async fn` on `ThunderbirdMcp` that delegates to your bridge function

4. **Docs**:
   - Add any quirks or gotchas to `docs/system_instructions.md`
   - Update the `instructions` example in `README.md` if needed
   - Update `ROADMAP.md` (check off the item)

> **Hard rule:** If you change a tool's schema or behavior, you must update `docs/system_instructions.md` and the `instructions` example in `README.md`.

## Thunderbird API Notes

- Thunderbird 128 changed `calendar.getItems()` to yield **arrays (batches)** per iteration, not individual items. Always unwrap batches:
  ```js
  for await (const batch of calendar.getItems(filter, 0, start, end)) {
    for (const item of (Array.isArray(batch) ? batch : [batch])) { ... }
  }
  ```
- `calIItemBase.getProperty(key)` may throw on items returned from `getItems()`. Wrap in try/catch:
  ```js
  const safeProp = (item, key) => { try { return item.getProperty(key) || null; } catch { return null; } };
  ```
- Result size: always apply `DEFAULT_MAX_RESULTS` / `MAX_SEARCH_RESULTS_CAP` caps to avoid unbounded responses.

## Pull Request Guidelines

- One logical change per PR
- Include a test session entry in `TESTING.md` for any user-facing tool changes
- Run `cargo test` before submitting
- Keep `docs/system_instructions.md` and `README.md` in sync with tool schema changes
