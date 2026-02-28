# Testing

## Automated tests

```bash
cargo test
```

Covers: auth token discovery, sanitize_str edge cases, bridge error handling.

## Manual smoke test

Thunderbird must be running with the extension installed.

```bash
TOKEN=$(cat ~/.thunderbird-mcp-auth)

# Accounts
curl -s -X POST http://localhost:45678/accounts/list \
  -H "Authorization: Bearer $TOKEN" -H "Content-Type: application/json" -d '{}'

# Recent messages (scope to a folder to avoid IMAP staleness)
curl -s -X POST http://localhost:45678/messages/recent \
  -H "Authorization: Bearer $TOKEN" -H "Content-Type: application/json" \
  -d '{"limit": 5}'
```

> **Note:** Direct `curl` against mail endpoints may return invalid JSON due to control characters in message content. This is expected — the Rust binary sanitizes responses in production. Use the MCP interface for end-to-end testing.

## End-to-end MCP test

With a Claude Code project configured (see README):

1. List your accounts and folders
2. Fetch the 5 most recent messages from INBOX
3. Search for a message by subject keyword
4. Read a specific message body
5. Create and delete a test folder
6. List calendar events for the current week

## Known issues to watch for

- **IMAP staleness** — `imapSyncPending: true` in responses; retry after a moment
- **Gmail label duplication** — always scope message queries to a specific folder
- **`create_event`** — opens compose dialog, requires manual confirmation
- **`apply_filters`** — async; verify results after a short delay
