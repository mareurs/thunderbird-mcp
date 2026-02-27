# Thunderbird MCP — Test Tracking

## Issues Found

### OPEN
|---|------|-------|----------|
| 2 | `apply_filters` | Processing is async — results appear after a delay; MCP response says "success" before messages are actually moved. Not a bug, just async. | Info |
| 3 | `get_message` | Body not sanitized — control characters can cause JSON parse errors on large/formatted emails | Low |
| 4 | `search_contacts` | Many contacts have empty `email` field — data quality issue in address book, not a bug | Info |
| 5 | `list_calendars` | Cannot read events — no `list_events` tool; impossible to inspect calendar contents or differentiate accounts | Medium |

### FIXED

| # | Tool | Issue | Fix |
|---|------|-------|-----|
| 1 | `get_recent_messages` / `search_messages` | Duplicate messages — same message ID appeared multiple times (one per Gmail label/folder) | Dedup by message ID in extension (`b610e9c`) |
| 6 | `update_message` | `flagged`/`read`/`trash` alone failed with "move_to must be a non-empty string" — Rust serializes `None` as JSON `null`, extension checked `!== undefined` (misses null) | Changed to `!= null` in extension |

## Test Sessions

### 2026-02-27 — Initial integration test (post rust-impl merge)
- `list_accounts` ✅ — 3 accounts returned
- `list_calendars` ✅ — 7 calendars returned
- `get_recent_messages` ✅ — dedup working after extension reinstall
- `search_messages` ✅ — dedup working, unique results
- `search_contacts` ✅ — returns results (some missing emails)
- `list_filters` ✅ — 12 filters on Gmail account
- `apply_filters` ⚠️ — ran but no messages moved (IMAP cache issue, see #2)
- `list_events` ❌ — not implemented (see ROADMAP)
