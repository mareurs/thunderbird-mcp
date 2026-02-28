# Thunderbird MCP — System Instructions

> Keep this file in sync with the `instructions` field in `.mcp.json` and any changes to tool schemas or extension logic.

## Mail

- `get_recent_messages` without `folder` returns all accounts interleaved and is slow — always scope by folder URI from `list_folders`
- Gmail duplicates messages across labels — always scope `get_recent_messages` / `search_messages` to a specific folder (e.g. INBOX)
- IMAP results may be stale (`imapSyncPending: true`) — retry if expected messages are missing
- `get_message` body can be very large (5000+ chars for newsletters) — use `search_messages` to filter before fetching full bodies
- `update_message` supports `read`, `flagged`, `move_to`, and `trash` fields — omit any field you don't want to change

## Compose

- `send_email`, `reply_to_message`, and `forward_message` open a compose window for user review — they do not send silently
- `reply_to_message` with `reply_all: true` includes all recipients

## Folders

- `create_folder` requires `parent_uri` to be an existing folder URI (e.g. INBOX), not the account root
- `create_folder` returns `path: null` — call `list_folders` after creation to get the real URI

## Filters

- `create_filter` conditions are ANDed by default — create one filter per sender for OR-style grouping across multiple senders
- `apply_filters` is async — the MCP response returns before messages are actually moved; wait a moment before verifying
- Filters apply to future incoming mail only; to backfill existing messages: Tools → Message Filters → Run Now

## Contacts

- Some contacts have no `primaryEmail` set — these return `email: null`, not an empty string; this is a data quality issue

## Calendar

- `list_events` without a date range returns up to 50 events across all calendars — always provide `date_from`/`date_to` for targeted queries
- `create_event` opens Thunderbird's event dialog instead of creating silently — the user must confirm and save

## Infrastructure

- The extension HTTP server runs on `localhost:45678` with Bearer token auth from `~/.thunderbird-mcp-auth`
- Direct `curl` against port 45678 may return invalid JSON for mail endpoints — control characters in message content are sanitized by the Rust layer, not the extension; use the MCP interface in production
