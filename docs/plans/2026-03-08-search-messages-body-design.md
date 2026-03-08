# search_messages Body Search — Design

## Goal

Add optional body search to `search_messages` via a `search_body: bool` param, returning a `bodySnippet` (≤200 chars) for body-matched messages.

## Architecture

- New `search_body: Option<bool>` param in Rust (`src/tools/mail.rs`, `src/server.rs`)
- `searchMessages` in `extension/mcp_server/api.js` gets `searchBody` param; when true, two-pass execution: sync header pre-filter → async body fetch loop
- `MsgHdrToMimeMessage` wrapped in a Promise for async body access
- Zero change to existing behavior when `search_body` is false/omitted

## Constraints (token awareness)

- `bodySnippet` is ≤200 chars centered on the first match — never return full body in search results
- Hard cap: 20 body fetches per call
- Requires at least one narrowing filter (folder, sender, date_from, or date_to) when `search_body: true`

## Execution Flow

**Pass 1 (sync):** iterate folders/messages, apply date/sender/folder pre-filters; collect header-passing candidates up to 20 (ignoring `query` match for now)

**Pass 2 (async):** for each candidate:
1. Check if `query` matches headers (subject/author/recipients/ccList) — if yes, include without body fetch
2. Otherwise `await fetchBody(msgHdr)` → check body → skip if no match → add `bodySnippet`

Return up to `effectiveLimit` matched results.

## New JS helper

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
```

## Snippet extraction

```js
function makeSnippet(text, query, maxLen = 200) {
  const lower = text.toLowerCase();
  const idx = lower.indexOf(query.toLowerCase());
  if (idx === -1) return text.slice(0, maxLen).trim();
  const start = Math.max(0, idx - 80);
  return text.slice(start, start + maxLen).trim();
}
```

## Response shape

Existing fields unchanged. Body-matched messages gain:
```json
{ "bodySnippet": "...up to 200 chars around the match..." }
```
Header-matched messages have no `bodySnippet` (query hit headers directly).

## Rust changes

- `src/tools/mail.rs`: add `search_body: Option<bool>` to `search_messages` function signature and `json!({...})` payload
- `src/server.rs`: add `#[tool(param)]` + `#[schemars(description)]` for `search_body`

## Tests

Two new Rust tests in `src/tools/mail.rs`:
- `search_messages_sends_search_body_true` — PartialJson verifies `search_body: true` sent
- `search_messages_omits_search_body_when_false` — verifies omitting param works (or passing false)

## Docs

`docs/system_instructions.md` — update the `search_messages` section to mention `search_body: true`, the narrowing filter requirement, `bodySnippet` field, and 20-fetch cap.
