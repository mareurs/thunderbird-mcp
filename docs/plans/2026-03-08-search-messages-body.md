# search_messages Body Search Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add optional body search to `search_messages` via a `search_body: bool` param, returning a `bodySnippet` (≤200 chars centered on the match) for body-matched messages.

**Architecture:** Two-pass execution in the JS extension: pass 1 collects header-pre-filtered candidates (sync, up to 20); pass 2 async-fetches bodies via `MsgHdrToMimeMessage` for candidates where `query` didn't match headers. Rust adds `search_body: Option<bool>` to the thin wrapper and `#[tool]` registration. Zero behavior change when `search_body` is false/omitted.

**Tech Stack:** Rust (rmcp, serde_json, mockito), JavaScript (Thunderbird XPCOM: `MsgHdrToMimeMessage`, `nsIMimeMessage.coerceBodyToPlaintext`)

---

## Background

- `src/tools/mail.rs:32–48` — `search_messages` thin wrapper. Currently 8 params. Tests at lines 170–181.
- `src/server.rs:36–61` — `ThunderbirdMcp::search_messages` `#[tool]` method with 7 optional params.
- `extension/mcp_server/api.js:359` — `searchMessages(query, folderPath, sender, recipient, startDate, endDate, maxResults)` — sync header-only search, collects into `results[]` via recursive `searchFolder()`.
- `MsgHdrToMimeMessage` is already used in `getMessage` (~line 760) — same pattern to reuse here.
- Token awareness rule (CLAUDE.md): never return full bodies in search results; use ≤200 char snippets.

---

### Task 1: Rust wrapper update + tests in `src/tools/mail.rs`

**Files:**
- Modify: `src/tools/mail.rs`

**Step 1: Write one new failing test**

Add after `search_messages_sends_query_and_sender` (after line 181), inside the `#[cfg(test)]` block:

```rust
    #[tokio::test]
    async fn search_messages_sends_search_body_true() {
        let mut server = Server::new_async().await;
        let mock = server.mock("POST", "/messages/search")
            .match_body(mockito::Matcher::PartialJson(json!({
                "query": "invoice",
                "search_body": true
            })))
            .with_status(200).with_body("{}").create_async().await;
        search_messages(
            &mock_bridge(&server).await,
            Some("invoice".into()), None, None,
            None, None, None, None, Some(true),
        ).await.unwrap();
        mock.assert_async().await;
    }
```

**Step 2: Run — expect compile error**

```bash
cargo test search_messages -- --nocapture 2>&1 | head -20
```
Expected: compile error (wrong number of arguments to `search_messages`).

**Step 3: Update `search_messages` function signature and json payload**

Use `mcp__codescout__replace_symbol` with name_path `search_messages`, path `src/tools/mail.rs`:

```rust
pub async fn search_messages(
    bridge: &Bridge,
    query: Option<String>,
    folder: Option<String>,
    sender: Option<String>,
    recipient: Option<String>,
    date_from: Option<String>,
    date_to: Option<String>,
    max_results: Option<u32>,
    search_body: Option<bool>,
) -> Result<CallToolResult, McpError> {
    let r = bridge.call("/messages/search", json!({
        "query": query, "folder": folder, "sender": sender,
        "recipient": recipient, "date_from": date_from,
        "date_to": date_to, "max_results": max_results,
        "search_body": search_body,
    })).await.map_err(bridge_err)?;
    Ok(result_text(r))
}
```

**Step 4: Update the existing test call**

The existing test `search_messages_sends_query_and_sender` calls `search_messages` with 8 positional args (after `&mock_bridge`). It now needs a 9th `None` for `search_body`.

Use `mcp__codescout__search_pattern` with pattern `search_messages_sends_query_and_sender` context_lines=15 to find it. Then `mcp__codescout__edit_file` to replace the call:

Find:
```rust
        search_messages(
            &mock_bridge(&server).await,
            Some("invoice".into()), None, Some("boss@corp.com".into()),
            None, None, None, None,
        ).await.unwrap();
```
Replace with:
```rust
        search_messages(
            &mock_bridge(&server).await,
            Some("invoice".into()), None, Some("boss@corp.com".into()),
            None, None, None, None, None,
        ).await.unwrap();
```

**Step 5: Run tests — expect both PASS**

```bash
cargo test search_messages -- --nocapture
```
Expected: `search_messages_sends_query_and_sender` and `search_messages_sends_search_body_true` both PASS.

**Step 6: Run full suite**

```bash
cargo test
```
Expected: all 68 tests PASS (was 67).

**Step 7: Commit**

```bash
git add src/tools/mail.rs
git commit -m "feat: add search_body param to search_messages Rust wrapper"
```

---

### Task 2: MCP tool registration update in `src/server.rs`

**Files:**
- Modify: `src/server.rs`

**Step 1: Update the `search_messages` method**

Use `mcp__codescout__search_pattern` with pattern `async fn search_messages` in `src/server.rs` context_lines=30 to see the full current method.

Use `mcp__codescout__edit_file` to:

1. Update the tool description from:
```rust
    #[tool(description = "Search messages by subject, sender, recipient, date range or folder")]
```
to:
```rust
    #[tool(description = "Search messages by subject, sender, recipient, date range, folder, or body content")]
```

2. Update the `query` param description from:
```rust
        #[schemars(description = "Text to search in subject, sender, and recipient fields (not body)")]
```
to:
```rust
        #[schemars(description = "Text to search in subject, sender, and recipient fields; also searches body when search_body is true")]
```

3. Add new param and update delegation — use `mcp__codescout__edit_file` to replace the closing section of the method. Find:
```rust
        #[tool(param)]
        #[schemars(description = "Max results, default 20, max 100")]
        max_results: Option<u32>,
    ) -> Result<CallToolResult, McpError> {
        mail::search_messages(&self.bridge, query, folder, sender, recipient, date_from, date_to, max_results).await
    }
```
Replace with:
```rust
        #[tool(param)]
        #[schemars(description = "Max results, default 20, max 100")]
        max_results: Option<u32>,
        #[tool(param)]
        #[schemars(description = "Set true to also search message bodies (requires at least one of: folder, sender, date_from, date_to); returns bodySnippet (≤200 chars) for body-matched messages; max 20 body fetches per call")]
        search_body: Option<bool>,
    ) -> Result<CallToolResult, McpError> {
        mail::search_messages(&self.bridge, query, folder, sender, recipient, date_from, date_to, max_results, search_body).await
    }
```

**Step 2: Build + test**

```bash
cargo build 2>&1 | head -10
cargo test
```
Expected: clean build, 68 tests PASS.

**Step 3: Commit**

```bash
git add src/server.rs
git commit -m "feat: register search_body param in search_messages MCP tool"
```

---

### Task 3: Extension two-pass body search in `extension/mcp_server/api.js`

**Files:**
- Modify: `extension/mcp_server/api.js`

This is the most complex task. Read the current `searchMessages` function carefully before editing.

**Step 1: Read current searchMessages**

Use `mcp__codescout__search_pattern` with pattern `function searchMessages` context_lines=80 to read the full function.

**Step 2: Add `fetchBody` and `makeSnippet` helpers**

Use `mcp__codescout__search_pattern` with pattern `function buildTerms` to find the insertion point (insert BEFORE `buildTerms`).

Use `mcp__codescout__edit_file` to insert before `function buildTerms`:

```js
            function fetchBody(msgHdr) {
              return new Promise((resolve) => {
                try {
                  MsgHdrToMimeMessage(msgHdr, null, (aMsgHdr, aMimeMsg) => {
                    try {
                      resolve(aMimeMsg ? aMimeMsg.coerceBodyToPlaintext() || "" : "");
                    } catch { resolve(""); }
                  }, false, { examineEncryptedParts: true, saneBodySize: true });
                } catch { resolve(""); }
              });
            }

            function makeSnippet(text, query, maxLen = 200) {
              const lower = text.toLowerCase();
              const idx = lower.indexOf(query.toLowerCase());
              if (idx === -1) return text.slice(0, maxLen).trim();
              const start = Math.max(0, idx - 80);
              return text.slice(start, start + maxLen).trim();
            }

```

**Step 3: Update `searchMessages` signature and add two-pass logic**

The current function signature is:
```js
function searchMessages(query, folderPath, sender, recipient, startDate, endDate, maxResults) {
```

And the ROUTES entry calls it as:
```js
async ({ query, folder, sender, recipient, date_from, date_to, max_results }) =>
  searchMessages(query, folder, sender, recipient, date_from, date_to, max_results),
```

Use `mcp__codescout__edit_file` to:

1. Update the ROUTES entry — find:
```js
              "/messages/search":       async ({ query, folder, sender, recipient, date_from, date_to, max_results }) =>
                                          searchMessages(query, folder, sender, recipient, date_from, date_to, max_results),
```
Replace with:
```js
              "/messages/search":       async ({ query, folder, sender, recipient, date_from, date_to, max_results, search_body }) =>
                                          searchMessages(query, folder, sender, recipient, date_from, date_to, max_results, search_body),
```

2. Rewrite `searchMessages` — use `mcp__codescout__search_pattern` to read the FULL current function body (pattern `function searchMessages` context_lines=120). Then use `mcp__codescout__edit_file` to replace the entire function definition with the new version below.

The key structural change: when `searchBody` is true, `searchFolder` collects candidates into `candidates[]` (up to `BODY_FETCH_CAP = 20`) instead of `results[]`, skipping the `query` text match. After folder traversal, a second async pass fetches bodies and applies the query.

```js
            function searchMessages(query, folderPath, sender, recipient, startDate, endDate, maxResults, searchBody) {
              const BODY_FETCH_CAP = 20;
              const results = [];
              const candidates = [];
              const lowerSender = sender ? sender.toLowerCase() : null;
              const lowerRecipient = recipient ? recipient.toLowerCase() : null;

              const parsedStartDate = startDate ? new Date(startDate).getTime() : NaN;
              const parsedEndDate = endDate ? new Date(endDate).getTime() : NaN;
              const startDateTs = Number.isFinite(parsedStartDate) ? parsedStartDate * 1000 : null;
              const endDateOffset = endDate && !endDate.includes("T") ? 86400000 : 0;
              const endDateTs = Number.isFinite(parsedEndDate) ? (parsedEndDate + endDateOffset) * 1000 : null;
              const requestedLimit = Number(maxResults);
              const effectiveLimit = Math.min(
                Number.isFinite(requestedLimit) && requestedLimit > 0 ? Math.floor(requestedLimit) : DEFAULT_MAX_RESULTS,
                MAX_SEARCH_RESULTS_CAP
              );

              let hasImapFolders = false;

              function searchFolder(folder) {
                if (searchBody ? candidates.length >= BODY_FETCH_CAP : results.length >= SEARCH_COLLECTION_CAP) return;
                try {
                  if (folder.server && folder.server.type === "imap") hasImapFolders = true;
                  if (folder.hasSubFolders) {
                    for (const sf of folder.subFolders) searchFolder(sf);
                  }
                } catch {}
                try {
                  const db = folder.msgDatabase;
                  if (!db) return;

                  for (const msgHdr of db.enumerateMessages()) {
                    if (searchBody ? candidates.length >= BODY_FETCH_CAP : results.length >= SEARCH_COLLECTION_CAP) break;

                    const subject = (msgHdr.mime2DecodedSubject || msgHdr.subject || "").toLowerCase();
                    const author = (msgHdr.mime2DecodedAuthor || msgHdr.author || "").toLowerCase();
                    const recipients = (msgHdr.mime2DecodedRecipients || msgHdr.recipients || "").toLowerCase();
                    const ccList = (msgHdr.ccList || "").toLowerCase();
                    const msgDateTs = msgHdr.date || 0;

                    if (startDateTs !== null && msgDateTs < startDateTs) continue;
                    if (endDateTs !== null && msgDateTs > endDateTs) continue;
                    if (lowerSender && !author.includes(lowerSender)) continue;
                    if (lowerRecipient && !recipients.includes(lowerRecipient) && !ccList.includes(lowerRecipient)) continue;

                    const textQuery = (query || "").toLowerCase();
                    const headerMatch = textQuery && (subject.includes(textQuery) || author.includes(textQuery) ||
                        recipients.includes(textQuery) || ccList.includes(textQuery));

                    if (searchBody) {
                      // Collect all header-pre-filtered candidates for async body pass
                      candidates.push({ msgHdr, headerMatch });
                    } else {
                      if (textQuery && !headerMatch) continue;
                      results.push({
                        id: msgHdr.messageId,
                        subject: sanitizeStr(msgHdr.mime2DecodedSubject || msgHdr.subject),
                        author: sanitizeStr(msgHdr.mime2DecodedAuthor || msgHdr.author),
                        recipients: sanitizeStr(msgHdr.mime2DecodedRecipients || msgHdr.recipients),
                        ccList: msgHdr.ccList,
                        date: msgHdr.date ? new Date(msgHdr.date / 1000).toISOString() : null,
                        folder: folder.prettyName,
                        folderPath: folder.URI,
                        read: msgHdr.isRead,
                        flagged: msgHdr.isFlagged,
                        _dateTs: msgDateTs
                      });
                    }
                  }
                } catch (e) { console.debug("searchMessages folder error", e); }
              }

              async function run() {
                if (searchBody) {
                  const hasNarrowingFilter = folderPath || sender || startDate || endDate;
                  if (!hasNarrowingFilter) return { error: "search_body requires at least one narrowing filter: folder, sender, date_from, or date_to" };
                }

                if (folderPath) {
                  const folder = MailServices.folderLookup.getFolderForURL(folderPath);
                  if (!folder) return { error: `Folder not found: ${folderPath}` };
                  searchFolder(folder);
                } else {
                  for (const account of MailServices.accounts.accounts) {
                    try {
                      const root = account?.incomingServer?.rootFolder;
                      if (root) searchFolder(root);
                    } catch {}
                  }
                }

                if (searchBody) {
                  const textQuery = (query || "").toLowerCase();
                  for (const { msgHdr, headerMatch } of candidates) {
                    if (results.length >= effectiveLimit) break;
                    let bodySnippet;
                    if (headerMatch) {
                      // Header already matched — no body fetch needed
                    } else {
                      const body = await fetchBody(msgHdr);
                      if (textQuery && !body.toLowerCase().includes(textQuery)) continue;
                      bodySnippet = makeSnippet(body, textQuery);
                    }
                    const entry = {
                      id: msgHdr.messageId,
                      subject: sanitizeStr(msgHdr.mime2DecodedSubject || msgHdr.subject),
                      author: sanitizeStr(msgHdr.mime2DecodedAuthor || msgHdr.author),
                      recipients: sanitizeStr(msgHdr.mime2DecodedRecipients || msgHdr.recipients),
                      ccList: msgHdr.ccList,
                      date: msgHdr.date ? new Date(msgHdr.date / 1000).toISOString() : null,
                      folderPath: msgHdr.folder ? msgHdr.folder.URI : null,
                      folder: msgHdr.folder ? msgHdr.folder.prettyName : null,
                      read: msgHdr.isRead,
                      flagged: msgHdr.isFlagged,
                    };
                    if (bodySnippet !== undefined) entry.bodySnippet = bodySnippet;
                    results.push(entry);
                  }
                  results.sort((a, b) => (b._dateTs || 0) - (a._dateTs || 0));
                  results.forEach(r => delete r._dateTs);
                  return {
                    messages: results.slice(0, effectiveLimit),
                    total: results.length,
                    ...(hasImapFolders ? { imapSyncPending: true } : {}),
                  };
                }

                results.sort((a, b) => (b._dateTs || 0) - (a._dateTs || 0));
                results.forEach(r => delete r._dateTs);
                const messages = results.slice(0, effectiveLimit);
                return {
                  messages,
                  total: results.length,
                  ...(hasImapFolders ? { imapSyncPending: true } : {}),
                };
              }

              return run();
            }
```

**Step 4: Syntax check**

```bash
node --check extension/mcp_server/api.js
```
Expected: no output (clean).

**Step 5: Full test suite**

```bash
cargo test
```
Expected: 68 tests PASS.

**Step 6: Commit**

```bash
git add extension/mcp_server/api.js
git commit -m "feat: add two-pass body search to searchMessages extension"
```

---

### Task 4: Update docs

**Files:**
- Modify: `docs/system_instructions.md`
- Modify: `ROADMAP.md`

**Step 1: Update `docs/system_instructions.md`**

Find:
```
- `search_messages` only searches message headers (subject, from, to, cc) — it does NOT search message bodies; to find messages by body content, use `get_message` to fetch individual messages
```

Replace with:
```
- `search_messages` searches headers (subject, from, to, cc) by default; set `search_body: true` to also search message bodies — requires at least one narrowing filter (folder, sender, date_from, or date_to); body-matched messages include a `bodySnippet` field (≤200 chars); max 20 body fetches per call
```

**Step 2: Update `ROADMAP.md`**

Find:
```
- [ ] **`apply_filters` for IMAP** — filters only run on locally cached messages; no fix without async sync, but could add a warning or a "fetch + filter" workflow
```

(This line is already done. Find the body-search related pending item if any, otherwise just leave ROADMAP as-is — the feature was listed under Known Issues, not ROADMAP.)

Actually find and update the search_messages note in ROADMAP if present, otherwise skip Step 2.

Use `mcp__codescout__search_pattern` pattern `search_messages` in `ROADMAP.md` to check. If found, mark done. If not found, skip.

**Step 3: Run full suite**

```bash
cargo test
```
Expected: 68 PASS.

**Step 4: Commit**

```bash
git add docs/system_instructions.md ROADMAP.md
git commit -m "docs: document search_body param and bodySnippet in search_messages"
```
