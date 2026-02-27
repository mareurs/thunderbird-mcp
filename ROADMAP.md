# Thunderbird MCP — Roadmap

## Planned Features

### Calendar
- [x] **`list_events`** — query events by calendar ID and date range using `cal.manager`

### Mail
- [ ] **`get_message` body sanitization** — strip control characters in `get_message` body (same as already done for subject/author in search/recent)
- [ ] **`apply_filters` for IMAP** — filters only run on locally cached messages; no fix without async sync, but could add a warning or a "fetch + filter" workflow

### Contacts
- [ ] **`create_contact` / `update_contact`** — write-side contact management
- [ ] **`delete_contact`** — remove contacts from address book

### Filters
- [ ] **OR logic across senders** — currently conditions are ANDed; multi-sender OR requires creating one filter per sender

### Folders
- [ ] **`delete_folder`** — remove a folder by URI

### Infrastructure
- [x] **`list_events` MCP tool in Rust** — wired through `src/server.rs` and `src/tools/contacts.rs`
