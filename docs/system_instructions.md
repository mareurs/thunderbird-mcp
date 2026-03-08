# Thunderbird MCP — System Instructions

> Keep this file in sync with the `instructions` field in `.mcp.json` and any changes to tool schemas or extension logic.

## Mail

- `get_recent_messages` without `folder` returns all accounts interleaved and is slow — always scope by folder URI from `list_folders`
- Gmail duplicates messages across labels — always scope `get_recent_messages` / `search_messages` to a specific folder (e.g. INBOX)
- IMAP results may be stale (`imapSyncPending: true`) — retry if expected messages are missing
- `search_messages` searches headers (subject, from, to, cc) by default; set `search_body: true` to also search message bodies — requires at least one narrowing filter (folder, sender, date_from, or date_to); body-matched messages include a `bodySnippet` field (≤200 chars); max 20 body fetches per call
- `get_message` body can be very large (5000+ chars for newsletters) — use `search_messages` to filter before fetching full bodies
- `update_message` supports `read`, `flagged`, `move_to`, and `trash` fields — omit any field you don't want to change
- `delete_messages_by_filter` deletes all messages in a folder matching date/sender/query filters in one call — requires `folder_uri` plus at least one of `date_from`, `date_to`, `sender`, or `query`; `date_from` with a date-only string starts at midnight (beginning of that day); `date_to` with a date-only string includes the full day; returns `{ deleted: N }` or `{ deleted: 0, note: "No messages matched the filters" }`

## Compose

- `send_email`, `reply_to_message`, and `forward_message` open a compose window for user review — they do not send silently
- `reply_to_message` with `reply_all: true` includes all recipients

## Folders

- `create_folder` requires `parent_uri` to be an existing folder URI (e.g. INBOX), not the account root
- `create_folder` returns `path: null` — call `list_folders` after creation to get the real URI
- `delete_folder` moves the folder to the account Trash — it does not permanently delete; to permanently remove, empty Trash afterward

## Filters

- `create_filter` conditions are ANDed by default; add `booleanAnd: false` to a condition to OR it with the previous one — use this for multi-sender filters instead of creating one filter per sender
- `apply_filters` is async — the MCP response returns before messages are actually moved; wait a moment before verifying
- `apply_filters` returns `warning` when the folder is IMAP — only locally cached messages are processed; messages not yet downloaded are skipped
- Filters apply to future incoming mail only; to backfill existing messages: Tools → Message Filters → Run Now

## Contacts

- Some contacts have no `primaryEmail` set — these return `email: null`, not an empty string; this is a data quality issue
- `list_address_books` returns `[{ uri, name, readOnly }]` — use `uri` as `address_book_uri` in `create_contact`
- `create_contact` requires `address_book_uri` — call `list_address_books` first to find the right book; returns `{ id, displayName }` for the new contact
- `update_contact` takes `contact_id` (UID) from `search_contacts` results; omit fields you don't want to change
- `delete_contact` requires `confirm: true` to execute; pass `false` (or omit) to preview the contact details before committing to deletion — deletion is permanent

## Calendar

- `list_events` without a date range returns up to 50 events across all calendars — always provide `date_from`/`date_to` for targeted queries
- `create_event` opens Thunderbird's event dialog instead of creating silently — the user must confirm and save

## Infrastructure

- The extension HTTP server runs on `localhost:45678` with Bearer token auth from `~/.thunderbird-mcp-auth`
- Direct `curl` against port 45678 may return invalid JSON for mail endpoints — control characters in message content are sanitized by the Rust layer, not the extension; use the MCP interface in production
