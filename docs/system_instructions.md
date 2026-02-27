# Thunderbird MCP — System Instructions

> Keep this file in sync with the `instructions` field in `.mcp.json` and any changes to tool schemas or extension logic.

- `create_filter` conditions are ANDed by default — create one filter per sender for OR-style grouping
- `create_folder` parent_uri must be an existing folder (e.g. INBOX), not the account root — returns `path: null`, call `list_folders` to get the real URI
- `get_recent_messages` without `folder` returns all accounts interleaved; Gmail duplicates messages across labels — always scope by folder
- The Spam folder causes JSON parse errors due to unsanitized control characters — do not read it
- Filters apply to future incoming mail only; to backfill: Tools → Message Filters → Run Now
