# Thunderbird MCP — Roadmap

For shipped features, see [FEATURES.md](FEATURES.md).

## Pending

### Needs Design (complex)

- **IMAP staleness** — folder contents lag until Thunderbird syncs; no clean fix without triggering async sync before search/filter operations. Needs brainstorming.
- **`apply_filters` full IMAP fix** — warning is returned today, but filters still only run on locally cached messages. A "fetch + filter" workflow would require async sync.

### Future Ideas

- **`delete_contact`** batch delete — currently one at a time by UID
- **OR logic across filter groups** — today `booleanAnd: false` ORs adjacent conditions; true grouped OR (e.g., OR between condition sets) is not supported by Thunderbird's filter model
- **`search_messages` body search beyond 20** — current cap is 20 body fetches per call; a streaming or paginated approach could remove the cap
