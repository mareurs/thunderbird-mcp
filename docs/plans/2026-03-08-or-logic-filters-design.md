# OR Logic in Filter Conditions — Design

## Goal

Document the existing `booleanAnd` field in filter conditions so AI clients can create OR-style filters without the one-filter-per-sender workaround.

## Finding

`buildTerms()` in `extension/mcp_server/api.js` already sets `term.booleanAnd = cond.booleanAnd !== false` — OR logic works today by passing `booleanAnd: false` on a condition. The feature was never documented.

## Change

**Documentation only — no code changes.**

### `src/server.rs`

Update the `conditions` `#[schemars(description)]` on both `create_filter` and `update_filter` to mention `booleanAnd`:

```
"Array of condition objects with attrib/op/value (attrib: subject|from|body|to|cc|toOrCc; op: contains|doesntContain|is|isnt|beginsWith|endsWith); add booleanAnd: false to OR with the previous condition (default is AND)"
```

### `docs/system_instructions.md`

Replace:
```
- `create_filter` conditions are ANDed by default — create one filter per sender for OR-style grouping across multiple senders
```

With:
```
- `create_filter` conditions are ANDed by default; add `booleanAnd: false` to a condition to OR it with the previous one — use this for multi-sender filters instead of creating one filter per sender
```

### `ROADMAP.md`

Mark OR logic item as done.
