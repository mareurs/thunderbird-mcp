# delete_folder Tool — Design

## Goal

Add a `delete_folder` MCP tool that moves a folder to the account's Trash using Thunderbird's XPCOM `propagateDelete` API.

## Architecture

Follows the exact same pattern as `create_folder`: Rust MCP tool in `src/server.rs` → thin wrapper in `src/tools/mail.rs` → bridge POST to `/folders/delete` → `deleteFolder()` function in `extension/mcp_server/api.js`.

## Extension (`api.js`)

New function placed after `createFolder`:

```js
function deleteFolder(folderUri) {
  if (typeof folderUri !== "string" || !folderUri)
    return { error: "folder_uri must be a non-empty string" };
  const folder = MailServices.folderLookup.getFolderForURL(folderUri);
  if (!folder) return { error: `Folder not found: ${folderUri}` };
  if (!folder.parent) return { error: "Cannot delete root folder" };
  folder.parent.propagateDelete(folder, false, null);
  return { success: true, message: `Folder moved to Trash` };
}
```

New ROUTES entry: `"/folders/delete": async ({ folder_uri }) => deleteFolder(folder_uri)`

`deleteStorage=false` triggers Thunderbird's trash semantics (as opposed to `true` which permanently deletes).

## Rust

`src/tools/mail.rs` — new `delete_folder` function:
```rust
pub async fn delete_folder(bridge: &Bridge, folder_uri: String) -> Result<CallToolResult, McpError> {
    let r = bridge.call("/folders/delete", json!({ "folder_uri": folder_uri }))
        .await.map_err(bridge_err)?;
    Ok(result_text(r))
}
```

`src/server.rs` — new `#[tool]` method:
```rust
async fn delete_folder(
    &self,
    #[tool(param)]
    #[schemars(description = "URI of the folder to delete (moves to Trash)")]
    folder_uri: String,
) -> Result<CallToolResult, McpError> {
    mail::delete_folder(&self.bridge, folder_uri).await
}
```

## Docs

- `docs/system_instructions.md` — add `delete_folder` tool entry
- `README.md` — add to features list

## Tests

Two tests in `src/tools/mail.rs`:
1. `delete_folder_sends_folder_uri` — param marshaling
2. `delete_folder_propagates_bridge_error` — error propagation
