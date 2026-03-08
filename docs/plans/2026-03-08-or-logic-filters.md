# OR Logic in Filter Conditions Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Document the existing `booleanAnd: false` feature in filter conditions so AI clients can create OR-style multi-sender filters without the one-filter-per-sender workaround.

**Architecture:** Pure documentation change — no new code. The JS extension already sets `term.booleanAnd = cond.booleanAnd !== false` in `buildTerms()`, so OR logic already works. We update two schemars description strings and one bullet in `docs/system_instructions.md`.

**Tech Stack:** Rust (schemars descriptions in server.rs), Markdown

---

### Task 1: Update conditions descriptions + system_instructions + ROADMAP

**Files:**
- Modify: `src/server.rs` (conditions schemars on `create_filter` and `update_filter`)
- Modify: `docs/system_instructions.md`
- Modify: `ROADMAP.md`

**Background:** In `src/server.rs`, the `create_filter` method (around line 260) has a `conditions` param with a `#[schemars(description = "Array of condition objects...")]`. The same description appears on `update_filter` a few lines lower. Neither mentions `booleanAnd`. The `system_instructions.md` Filters section has a bullet that incorrectly describes the workaround.

**Step 1: Update `create_filter` conditions description in `src/server.rs`**

Use `mcp__codescout__search_pattern` with pattern `"Array of condition objects"` in `src/server.rs` to find both occurrences. There will be two — one for `create_filter`, one for `update_filter`.

Use `mcp__codescout__edit_file` with `replace_all=true` to replace:
```
Array of condition objects with attrib/op/value (attrib: subject|from|body|to|cc|toOrCc; op: contains|doesntContain|is|isnt|beginsWith|endsWith)
```
With:
```
Array of condition objects with attrib/op/value (attrib: subject|from|body|to|cc|toOrCc; op: contains|doesntContain|is|isnt|beginsWith|endsWith); add booleanAnd: false to OR a condition with the previous one (default is AND)
```

**Step 2: Update `docs/system_instructions.md`**

Use `mcp__codescout__edit_file` to replace:
```
- `create_filter` conditions are ANDed by default — create one filter per sender for OR-style grouping across multiple senders
```
With:
```
- `create_filter` conditions are ANDed by default; add `booleanAnd: false` to a condition to OR it with the previous one — use this for multi-sender filters instead of creating one filter per sender
```

**Step 3: Update `ROADMAP.md`**

Use `mcp__codescout__edit_file` to replace:
```
- [ ] **OR logic across senders** — currently conditions are ANDed; multi-sender OR requires creating one filter per sender
```
With:
```
- [x] **OR logic across senders** — use `booleanAnd: false` on a condition to OR it with the previous one
```

**Step 4: Build + test**

```bash
cargo build 2>&1 | head -10
cargo test
```
Expected: clean build, 67 tests pass (no new tests — this is documentation only).

**Step 5: Commit**

```bash
git add src/server.rs docs/system_instructions.md ROADMAP.md
git commit -m "docs: document booleanAnd: false for OR conditions in create_filter and update_filter"
```
