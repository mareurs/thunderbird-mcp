# Thunderbird MCP

Rust MCP server + slim Thunderbird extension that exposes 20 tools for email, contacts, calendar, and filters.

## Architecture

```
MCP Client (stdio) ──→ thunderbird-mcp (Rust binary) ──→ HTTP :45678 ──→ extension/mcp_server/api.js (Thunderbird XPCOM)
```

- **Rust binary** (`src/`) — owns the MCP surface (rmcp 0.1), auth token discovery, output sanitization
- **Extension** (`extension/`) — thin XPCOM adapter, 20 HTTP endpoints, no MCP knowledge
- **Port**: 45678, auth via Bearer token written to `~/.thunderbird-mcp-auth`

## Build & Run

```bash
cargo build --release          # build Rust binary
./scripts/build.sh             # package extension → dist/mcp-server.xpi

# Install extension: Thunderbird → Add-ons → Install from file → dist/mcp-server.xpi
# Restart Thunderbird, then:
./target/release/thunderbird-mcp   # run MCP server (reads token, connects stdio)
```

## Releasing

```bash
git tag v0.1.0 && git push origin v0.1.0
```

CI (`.github/workflows/release.yml`) picks up any `v*` tag, builds `mcp-server.xpi` and `thunderbird-mcp-linux-x86_64`, and attaches them to a GitHub release automatically. Bump `extension/manifest.json` version to match before tagging.

## Testing

```bash
# Smoke test extension HTTP directly
TOKEN=$(cat ~/.thunderbird-mcp-auth)
curl -s -X POST http://localhost:45678/accounts/list \
  -H "Authorization: Bearer $TOKEN" -H "Content-Type: application/json" -d '{}'

# Full MCP session test
cargo test
```

## Security — Public Repository

This project is **public on GitHub**. Before every commit:
- No personal email addresses, account IDs, or contact names
- No API keys, tokens, passwords, or credentials
- No server hostnames, IPs, or personal infrastructure details
- No real email content, folder names, or conversation data
- `.mcp.json` is gitignored — never commit it
- Run `git diff --staged` and review carefully before committing

## Rust Coding Standards

### Ownership first
- Restructure data and function signatures to satisfy the borrow checker.
  Never reach for `.clone()` as a first resort — only clone when semantically
  appropriate (the clone represents intentional duplication, not a workaround).
- Design ownership topology before writing implementations. Ask: who owns this
  data, and what borrows it?

### Error handling
- All fallible functions return `Result<T, E>`. Never use `.unwrap()` in
  library code. In application code, only use `.unwrap()` where a panic is
  genuinely the correct behavior and document why.
- Use `anyhow` for application-level error propagation, `thiserror` for
  library error types.
- Propagate errors with `?`. Avoid nested match blocks for error handling.

### Iterators over loops
- Prefer iterator adapters (`.map()`, `.filter()`, `.fold()`, `.collect()`)
  over explicit `for` loops when the intent is a transformation pipeline.
- Use `.iter()` / `.iter_mut()` / `.into_iter()` correctly — do not
  implicitly rely on auto-deref behavior.

### Generics vs trait objects
- Default to generics (`fn foo<T: Trait>(x: T)`) for zero-cost dispatch.
- Only use `Box<dyn Trait>` / `Arc<dyn Trait>` when you need runtime
  polymorphism (heterogeneous collections, plugin systems, dynamic dispatch).
  Always comment why dynamic dispatch is needed.

### Concurrency
- Before reaching for `Arc<Mutex<T>>`, ask whether ownership can be
  structured to avoid shared state entirely.
- Prefer message passing (channels) over shared state for complex coordination.
- Use `tokio` for async I/O. Never block inside async functions.

### Clippy
- All code must pass `cargo clippy -- -D warnings`.
- When clippy suggests an alternative, prefer it unless there's a documented
  reason not to.

### Lifetimes
- Annotate lifetimes explicitly when the compiler cannot infer them and
  explain the relationship in a comment.
- Prefer owned types in struct fields over references where the struct
  needs to be self-contained or sent across threads.

### unsafe
- Never write `unsafe` without a `// SAFETY:` comment explaining the
  invariants being upheld.
- Prefer safe abstractions. Reach for `unsafe` only when there is no
  safe alternative and the performance gain is measured and documented.

## Hard Rules

- **If you modify MCP tool schemas, action/condition formats, or extension logic — you MUST update `docs/system_instructions.md` and the `instructions` example in `README.md` to reflect the change.** (`.mcp.json` is gitignored — users create it locally from the README example.)

- **Token awareness** — Tool responses are fed directly into AI context windows. Never return full email bodies in list/search results; always use short excerpts (≤200 chars, centered on the match). Return counts and IDs first; let the caller decide whether to fetch full content via `get_message`. Prefer tight result caps over unbounded lists. Every new tool response field that could contain free-form text must have a documented size bound.

## Known Issues

- **Control chars in raw HTTP responses** — Email subjects/bodies can contain control characters that make the raw HTTP response invalid JSON. Not a problem in production (Rust's `sanitize_str()` strips them before parsing), but direct `curl` tests against port 45678 will fail with `JSONDecodeError`. **Fixed** for all mail tools: `get_recent_messages`, `search_messages`, and `get_message` all apply `sanitizeStr()` in the extension. Other tools (filters, contacts, calendar) don't return free-text user content so are not affected.

- **IMAP staleness** — `imapSyncPending: true` is returned when IMAP folders are involved. Message databases may lag behind the server until the user clicks the folder in Thunderbird. Not fixable without async sync; just retry.

- **`get_message` body quality** — HTML newsletters strip to plain text via `stripHtml()`, leaving `[image_url]` artifacts and losing formatting. Bodies over 4000 chars are written to a private file and returned as `bodyFile` plus a 500-char `bodyPreview`, so they do not flood the context (see `docs/system_instructions.md`). Plain text is the right default for AI consumption.

- **`search_contacts` missing emails** — Some contacts in the address book have no `primaryEmail` set. These return `email: null` (not an empty string). This is a data quality issue in the address book, not a bug.
