# Search & Delete Fixes Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Fix three bugs in `searchMessages` and add a `delete_messages_by_filter` tool that deletes messages from a folder matching date/sender/query filters in one call.

**Architecture:** All logic lives in `extension/mcp_server/api.js` (JS, runs inside Thunderbird) and the Rust thin wrappers in `src/tools/mail.rs` + `src/server.rs`. The extension exposes HTTP endpoints; Rust proxies them as MCP tools. No body search in scope — that requires `nsIMsgSearchSession` and is tracked separately in TODO.md.

**Tech Stack:** JavaScript (SpiderMonkey/XPCOM inside Thunderbird), Rust (rmcp 0.1), curl for smoke tests.

**After each task:** Update `docs/system_instructions.md` and `README.md` if tool schemas changed (per CLAUDE.md Hard Rules). Review diff before committing — this repo is public, no personal data.

---

### Task 1: Dead code cleanup in `searchMessages`

**Files:**
- Modify: `extension/mcp_server/api.js:361-366`

**Context:**
Two unused variables exist in `searchMessages`:
- Line 363: `lowerQuery` is mutated to include sender, but line 408 re-derives `textQuery = (query || "").toLowerCase()` from scratch — the mutation is silently ignored.
- Line 366: `hasQuery` is declared but never read.

**Step 1: Verify the dead code before touching anything**

```bash
grep -n "lowerQuery\|hasQuery\|textQuery" extension/mcp_server/api.js
```

Expected: `lowerQuery` mutated on line 363, `textQuery` freshly derived on line 408, `hasQuery` declared on 366 and never used after.

**Step 2: Remove the two dead lines**

In `extension/mcp_server/api.js`, replace lines 361–366:

```js
// BEFORE (lines 361–366):
let lowerQuery = (query || "").toLowerCase();
// Append sender/recipient as search terms if provided
if (sender) lowerQuery = lowerQuery ? `${lowerQuery} ${sender.toLowerCase()}` : sender.toLowerCase();
const lowerSender = sender ? sender.toLowerCase() : null;
const lowerRecipient = recipient ? recipient.toLowerCase() : null;
const hasQuery = !!(query || sender || recipient);

// AFTER:
const lowerSender = sender ? sender.toLowerCase() : null;
const lowerRecipient = recipient ? recipient.toLowerCase() : null;
```

**Step 3: Verify nothing else references the removed variables**

```bash
grep -n "lowerQuery\|hasQuery" extension/mcp_server/api.js
```

Expected: zero matches.

**Step 4: Smoke test — search still works**

```bash
TOKEN=$(cat ~/.thunderbird-mcp-auth)
curl -s -X POST http://localhost:45678/messages/search \
  -H "Authorization: Bearer $TOKEN" -H "Content-Type: application/json" \
  -d '{"sender": "pariyatti.org", "max_results": 3}'
```

Expected: JSON with `messages` array (not empty), `imapSyncPending: true`.

**Step 5: Commit**

```bash
git add extension/mcp_server/api.js
git commit -m "fix: remove dead lowerQuery mutation and unused hasQuery in searchMessages"
```

---

### Task 2: Add `delete_messages_by_filter` tool

**Files:**
- Modify: `extension/mcp_server/api.js` (add endpoint, add function)
- Modify: `src/tools/mail.rs` (add Rust handler)
- Modify: `src/server.rs` (register tool)
- Modify: `docs/system_instructions.md` (add tool docs)
- Modify: `README.md` (add tool to instructions example if present)

**Context:**
Deleting old emails by date requires: search → collect IDs → delete in batches. With 1000+ messages this is tedious. A single `delete_messages_by_filter` call should do it all internally.

Parameters:
- `folder_uri` (required) — scope deletion to one folder
- `date_to` (optional ISO 8601) — delete messages older than this date
- `date_from` (optional ISO 8601) — delete messages newer than this date
- `sender` (optional) — filter by sender
- `query` (optional) — filter by subject/author/recipients keyword

Returns: `{ deleted: N, skipped: M }` where skipped = not found in local DB.

**Step 1: Write the failing smoke test (before any code)**

```bash
TOKEN=$(cat ~/.thunderbird-mcp-auth)
curl -s -X POST http://localhost:45678/messages/delete-by-filter \
  -H "Authorization: Bearer $TOKEN" -H "Content-Type: application/json" \
  -d '{"folder": "imap://test@example.com/INBOX", "date_to": "2020-01-01"}'
```

Expected right now: HTTP 404 (endpoint doesn't exist yet).

**Step 2: Add `deleteMessagesByFilter` function in `api.js`**

Add this function after the `deleteMessages` function (around line 1183 in api.js):

```js
function deleteMessagesByFilter(folderPath, dateFrom, dateTo, sender, query, maxResults) {
  try {
    if (!folderPath) return { error: "folder_uri is required" };

    const folder = MailServices.folderLookup.getFolderForURL(folderPath);
    if (!folder) return { error: `Folder not found: ${folderPath}` };

    const lowerSender = sender ? sender.toLowerCase() : null;
    const lowerQuery = query ? query.toLowerCase() : null;

    const parsedDateFrom = dateFrom ? new Date(dateFrom).getTime() : NaN;
    const parsedDateTo   = dateTo   ? new Date(dateTo).getTime()   : NaN;
    // Add one day offset for date-only strings (no T) so "2026-02-07" includes that whole day
    const dateFromOffset = dateFrom && !dateFrom.includes("T") ? 0 : 0;
    const dateToOffset   = dateTo   && !dateTo.includes("T")   ? 86400000 : 0;
    const startTs = Number.isFinite(parsedDateFrom) ? (parsedDateFrom + dateFromOffset) * 1000 : null;
    const endTs   = Number.isFinite(parsedDateTo)   ? (parsedDateTo   + dateToOffset)   * 1000 : null;

    if (!startTs && !endTs && !lowerSender && !lowerQuery) {
      return { error: "At least one filter (date_from, date_to, sender, query) is required" };
    }

    const db = folder.msgDatabase;
    if (!db) return { error: "Folder database not available — open the folder in Thunderbird first" };

    const toDelete = [];
    for (const msgHdr of db.enumerateMessages()) {
      const msgDate = msgHdr.date || 0;
      if (startTs !== null && msgDate < startTs) continue;
      if (endTs   !== null && msgDate > endTs)   continue;

      if (lowerSender) {
        const author = (msgHdr.mime2DecodedAuthor || msgHdr.author || "").toLowerCase();
        if (!author.includes(lowerSender)) continue;
      }
      if (lowerQuery) {
        const subject    = (msgHdr.mime2DecodedSubject    || msgHdr.subject    || "").toLowerCase();
        const author     = (msgHdr.mime2DecodedAuthor     || msgHdr.author     || "").toLowerCase();
        const recipients = (msgHdr.mime2DecodedRecipients || msgHdr.recipients || "").toLowerCase();
        const ccList     = (msgHdr.ccList || "").toLowerCase();
        if (!subject.includes(lowerQuery) && !author.includes(lowerQuery) &&
            !recipients.includes(lowerQuery) && !ccList.includes(lowerQuery)) continue;
      }
      toDelete.push(msgHdr);
    }

    if (toDelete.length === 0) return { deleted: 0, note: "No messages matched the filters" };

    folder.deleteMessages(toDelete, null, false, true, null, false);
    return { deleted: toDelete.length };
  } catch (e) { return { error: e.toString() }; }
}
```

**Step 3: Register the endpoint in the route table**

In `api.js`, find the route table (around line 1629, where `/messages/delete` is defined) and add:

```js
"/messages/delete-by-filter": async ({ folder_uri, date_from, date_to, sender, query, max_results }) =>
  deleteMessagesByFilter(folder_uri, date_from, date_to, sender, query, max_results),
```

**Step 4: Smoke test the JS endpoint directly**

```bash
TOKEN=$(cat ~/.thunderbird-mcp-auth)
# Should return { deleted: 0, note: "No messages matched..." } or an actual count
curl -s -X POST http://localhost:45678/messages/delete-by-filter \
  -H "Authorization: Bearer $TOKEN" -H "Content-Type: application/json" \
  -d '{
    "folder_uri": "imap://ailinca.marius%40gmail.com@imap.gmail.com/INBOX/Newsletters",
    "date_to": "2020-01-01"
  }'
```

Expected: `{"deleted": 0, "note": "No messages matched the filters"}` or a real count if old messages exist locally.

**Step 5: Add Rust handler in `src/tools/mail.rs`**

Add after the `delete_messages` function (around line 99):

```rust
pub async fn delete_messages_by_filter(
    bridge: &Bridge,
    folder_uri: String,
    date_from: Option<String>,
    date_to: Option<String>,
    sender: Option<String>,
    query: Option<String>,
) -> Result<CallToolResult, McpError> {
    let r = bridge.call("/messages/delete-by-filter", serde_json::json!({
        "folder_uri": folder_uri,
        "date_from": date_from,
        "date_to": date_to,
        "sender": sender,
        "query": query,
    })).await.map_err(bridge_err)?;
    Ok(result_text(r))
}
```

**Step 6: Register tool in `src/server.rs`**

Add after the `delete_messages` tool block (around line 126):

```rust
#[tool(description = "Delete messages from a folder matching date/sender/query filters. At least one filter required. folder_uri is mandatory.")]
async fn delete_messages_by_filter(
    &self,
    #[tool(param)]
    #[schemars(description = "Folder URI to delete from (required)")]
    folder_uri: String,
    #[tool(param)]
    #[schemars(description = "Delete messages older than this date (ISO 8601, e.g. 2026-01-01)")]
    date_to: Option<String>,
    #[tool(param)]
    #[schemars(description = "Delete messages newer than this date (ISO 8601)")]
    date_from: Option<String>,
    #[tool(param)]
    #[schemars(description = "Only delete messages from this sender")]
    sender: Option<String>,
    #[tool(param)]
    #[schemars(description = "Only delete messages matching this keyword in subject/from/to/cc")]
    query: Option<String>,
) -> Result<CallToolResult, McpError> {
    mail::delete_messages_by_filter(&self.bridge, folder_uri, date_from, date_to, sender, query).await
}
```

**Step 7: Build and verify compilation**

```bash
cargo build 2>&1 | tail -5
```

Expected: `Finished ... in Xs` with no errors.

**Step 8: Run existing tests**

```bash
cargo test 2>&1 | tail -10
```

Expected: all tests pass.

**Step 9: End-to-end MCP smoke test**

Rebuild and deploy extension, then via MCP:
```
delete_messages_by_filter(
  folder_uri="imap://ailinca.marius%40gmail.com@imap.gmail.com/INBOX/Newsletters",
  date_to="2026-01-01"
)
```

Expected: `{"deleted": N}` where N > 0 if old newsletters exist.

**Step 10: Update `docs/system_instructions.md`**

Add `delete_messages_by_filter` to the tools section with its parameters and return format.

**Step 11: Commit**

```bash
git add extension/mcp_server/api.js src/tools/mail.rs src/server.rs docs/system_instructions.md
git commit -m "feat: add delete_messages_by_filter tool with date/sender/query filters"
```

---

### Task 3: Fix search — document header-only limitation and add note to response

**Files:**
- Modify: `extension/mcp_server/api.js` (add `note` to search response)
- Modify: `docs/system_instructions.md` (clarify search scope)
- Modify: `src/server.rs` (update tool description)

**Context:**
`searchMessages` only searches `subject`, `author`, `recipients`, `ccList` — never the message body.
The tool description currently says "Text to search in subject/body" which is wrong.
Body search via `nsIMsgSearchSession` is a larger effort (tracked in TODO.md).
This task fixes the misleading description and adds a `note` field to responses when a text query was used.

**Step 1: Fix the misleading schema description in `src/server.rs`**

Find the `search_messages` tool param for `query`:
```rust
// BEFORE:
#[schemars(description = "Text to search in subject/body")]

// AFTER:
#[schemars(description = "Text to search in subject, sender, and recipient fields (not body)")]
```

**Step 2: Add search scope note to responses in `api.js`**

In `searchMessages`, after building the `messages` array and before returning, add a note when `query` was provided:

```js
// Find this block near end of searchMessages (around line 456):
if (hasImapFolders) {
  return { messages, imapSyncPending: true, note: "IMAP folder sync is async..." };
}
return messages;

// Replace with:
const searchNote = query
  ? "Search covers subject, sender, and recipient fields only — not message body."
  : undefined;
if (hasImapFolders) {
  const result = { messages, imapSyncPending: true, note: "IMAP folder sync is async - results may not include the latest messages. Retry if expected messages are missing." };
  if (searchNote) result.searchNote = searchNote;
  return result;
}
if (searchNote) return { messages, searchNote };
return messages;
```

**Step 3: Smoke test**

```bash
TOKEN=$(cat ~/.thunderbird-mcp-auth)
curl -s -X POST http://localhost:45678/messages/search \
  -H "Authorization: Bearer $TOKEN" -H "Content-Type: application/json" \
  -d '{"query": "test", "max_results": 3}'
```

Expected: response includes `"searchNote": "Search covers subject, sender, and recipient fields only — not message body."`.

**Step 4: Update `docs/system_instructions.md`**

Clarify under `search_messages`: query searches subject/from/to/cc only, not body.

**Step 5: Build and test**

```bash
cargo build 2>&1 | tail -3
cargo test 2>&1 | tail -5
```

**Step 6: Commit**

```bash
git add extension/mcp_server/api.js src/server.rs docs/system_instructions.md
git commit -m "fix: clarify search_messages only searches headers not body, add searchNote to responses"
```
