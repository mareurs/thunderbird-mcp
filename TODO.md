# TODO

## Search Fixes

### 1. Text search is header-only (no body search)
`searchMessages` only checks `subject`, `author`, `recipients`, `ccList` — never the message body.
Searching `"buddhism"` returns empty because "Daily Words of the Buddha" headers never contain that word.

**Fix options:**
- Use Thunderbird's native `nsIMsgSearchSession` API (supports body search, online IMAP search)
- Download body on demand via `MsgHdrToMimeMessage` for matched headers (slow but thorough)

### 2. IMAP folder sync — messages not yet in local DB
`updateFolder(null)` is async — search proceeds immediately on the local `msgDatabase`.
Folders never opened in Thunderbird have empty local DBs → zero results.

**Fix options:**
- Use `nsIMsgSearchSession` which can issue server-side IMAP SEARCH commands
- Expose a `sync_folder` tool that triggers `updateFolder` and waits before searching

### 3. Dead code in `searchMessages` (extension/mcp_server/api.js)
- Line 363: `lowerQuery` is modified to include sender, but line 408 re-derives
  `textQuery = (query || "").toLowerCase()` fresh — the modification is silently ignored
- Line 366: `hasQuery` is declared but never used anywhere in the function
- These should be cleaned up to avoid confusion

## Delete with Date Filters

No tool supports date-based deletion. Current workaround requires:
1. `search_messages(folder, date_to=...)` → extract IDs (100 at a time)
2. `delete_messages([...ids])` → repeat until empty

**Proposed:** Add `date_from`/`date_to` parameters to `delete_messages`, or a dedicated
`delete_messages_by_filter(folder, date_from, date_to, sender, query)` tool that handles
pagination internally.
