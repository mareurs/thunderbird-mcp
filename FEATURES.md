# Thunderbird MCP — Features

All tools and capabilities currently implemented and shipped.

## Tools (27 total)

### Mail
| Tool | Description |
|------|-------------|
| `get_recent_messages` | Fetch recent messages from a folder with optional date and unread filtering |
| `search_messages` | Search by subject, sender, recipient, date range, folder, or body content (`search_body: true`); body-matched results include `bodySnippet` ≤200 chars |
| `get_message` | Read full email body, headers, and attachments; optionally save attachments to disk |
| `update_message` | Mark read/unread/flagged, move to folder, or move to Trash |
| `delete_messages` | Delete messages by ID list |
| `delete_messages_by_filter` | Delete messages matching date/sender/query filters within a folder |

### Compose
| Tool | Description |
|------|-------------|
| `send_mail` | Open compose window with pre-filled recipients, subject, body, cc, bcc |
| `reply_to_message` | Reply to a message with optional reply-all |
| `forward_message` | Forward a message to one or more recipients |

### Folders
| Tool | Description |
|------|-------------|
| `list_accounts` | List all email accounts and identities |
| `list_folders` | Browse folder tree; filter by account or subtree |
| `create_folder` | Create a new folder under a parent folder URI |
| `delete_folder` | Remove a folder by URI (moves to Trash, not permanent delete) |

### Filters
| Tool | Description |
|------|-------------|
| `list_filters` | List all message filters with human-readable conditions and actions |
| `create_filter` | Create a message filter with structured conditions and actions; OR logic via `booleanAnd: false` |
| `update_filter` | Modify a filter's name, enabled state, conditions, or actions |
| `delete_filter` | Delete a filter by index |
| `reorder_filters` | Change filter execution priority |
| `apply_filters` | Run filters against a folder; returns `warning` when folder is IMAP (only locally cached messages are processed) |

### Contacts
| Tool | Description |
|------|-------------|
| `search_contacts` | Search contacts by name or email across all address books |
| `list_address_books` | List all address books with URI, name, and read-only status |
| `create_contact` | Create a new contact in a specified address book |
| `update_contact` | Update fields on an existing contact by UID |
| `delete_contact` | Permanently delete a contact; `confirm: true` required to execute (pass `false` to preview) |

### Calendar
| Tool | Description |
|------|-------------|
| `list_calendars` | List all calendars |
| `list_events` | Query calendar events by calendar ID and date range |
| `create_event` | Create a new calendar event (opens Thunderbird's event dialog) |

---

## Quality & Infrastructure

- **Bearer token auth** — auth token auto-discovered from `~/.thunderbird-mcp-auth`
- **Control char sanitization** — Rust `sanitize_str()` strips ASCII control chars before JSON parse; JS `sanitizeStr()` in extension for free-text fields
- **IMAP staleness detection** — responses include `imapSyncPending: true` when IMAP folders are involved
- **Token-aware responses** — search results return snippets, not full bodies; `bodySnippet` capped at 200 chars
- **68 Rust unit tests** — mockito-based; cover param marshaling and error propagation for all tools
- **OR filter conditions** — `booleanAnd: false` on a condition ORs it with the previous one (documented but was always supported by the extension)
- **IMAP warning on apply_filters** — response includes `warning` field when the target folder is IMAP

---

## Known Limitations

- **IMAP staleness** — folder contents lag behind the server until Thunderbird syncs; retry if results look stale (needs async sync design — brainstorm pending)
- **`apply_filters` on IMAP** — only locally cached messages are processed; a warning is returned but no full fix without triggering async sync first
- **`create_event`** — opens Thunderbird's event dialog instead of creating silently; requires user confirmation in Thunderbird
- **`search_messages` body search cap** — max 20 body fetches per call; scope with `folder`/`sender`/`date` filters for best results
- **Spam folder** — control characters in spam bodies can cause parse errors; avoid reading directly
- **Gmail** — duplicates messages across labels; always scope to a specific folder
