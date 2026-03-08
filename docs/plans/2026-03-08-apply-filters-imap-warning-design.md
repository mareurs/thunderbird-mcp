# apply_filters IMAP Warning — Design

## Goal

Add a `warning` field to `apply_filters` responses when the target folder is an IMAP folder, so AI clients know that only locally cached messages were processed.

## Architecture

Change is entirely in `extension/mcp_server/api.js` — no Rust changes needed. The `warning` field passes through transparently in the JSON response that `Bridge::call` returns.

## Change

In `applyFilters()`, replace the current success return with:

```js
const isImap = folder.server && folder.server.type === "imap";
return {
  success: true,
  message: "Filters applied (processing may take a moment)",
  folder: folderUri,
  enabledFilters: enabledCount,
  ...(isImap ? { warning: "IMAP folder: filters only apply to locally cached messages; messages not yet downloaded are skipped" } : {}),
};
```

Detection uses `folder.server.type === "imap"` (XPCOM server object) rather than URI string matching — more robust and semantically correct.

## Docs

`docs/system_instructions.md` — update the existing `apply_filters` bullet in `## Filters` to mention the warning field for IMAP folders.

## Tests

Add `apply_filters_propagates_bridge_error` to `src/tools/filters.rs` — the only missing test for this tool.
