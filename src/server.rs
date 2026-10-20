use std::sync::Arc;
use rmcp::{
    ServerHandler,
    model::{CallToolResult, ServerCapabilities, ServerInfo},
    schemars, tool, Error as McpError,
};
use serde_json::Value;
use crate::bridge::Bridge;
use crate::tools::{mail, compose, filters, contacts};

#[derive(Clone)]
pub struct ThunderbirdMcp {
    pub bridge: Arc<Bridge>,
}

#[tool(tool_box)]
impl ThunderbirdMcp {
    #[tool(description = "List all email accounts and their identities")]
    async fn list_accounts(&self) -> Result<CallToolResult, McpError> {
        mail::list_accounts(&self.bridge).await
    }

    #[tool(description = "Browse folder tree. Optionally filter by account or a specific subtree.")]
    async fn list_folders(
        &self,
        #[tool(param)]
        #[schemars(description = "Account ID to filter by")]
        account_id: Option<String>,
        #[tool(param)]
        #[schemars(description = "Folder URI to list subtree from")]
        folder_uri: Option<String>,
    ) -> Result<CallToolResult, McpError> {
        mail::list_folders(&self.bridge, account_id, folder_uri).await
    }

    #[tool(description = "Search messages by subject, sender, recipient, date range, folder, or body content")]
    async fn search_messages(
        &self,
        #[tool(param)]
        #[schemars(description = "Text to search in subject, sender, and recipient fields; also searches body when search_body is true")]
        query: Option<String>,
        #[tool(param)]
        #[schemars(description = "Folder URI to scope search")]
        folder: Option<String>,
        #[tool(param)]
        #[schemars(description = "Filter by sender address")]
        sender: Option<String>,
        #[tool(param)]
        #[schemars(description = "Filter by recipient address")]
        recipient: Option<String>,
        #[tool(param)]
        #[schemars(description = "Start date (ISO 8601)")]
        date_from: Option<String>,
        #[tool(param)]
        #[schemars(description = "End date (ISO 8601)")]
        date_to: Option<String>,
        #[tool(param)]
        #[schemars(description = "Max results, default 20, max 100")]
        max_results: Option<u32>,
        #[tool(param)]
        #[schemars(description = "Set true to also search message bodies (requires at least one of: folder, sender, date_from, date_to); returns bodySnippet (≤200 chars) for body-matched messages; max 20 body fetches per call")]
        search_body: Option<bool>,
    ) -> Result<CallToolResult, McpError> {
        mail::search_messages(&self.bridge, query, folder, sender, recipient, date_from, date_to, max_results, search_body).await
    }

    #[tool(description = "Read email content and attachments. Bodies over 4000 chars are written to a private file: the response then has bodyFile (path), bodyLength and a bodyPreview — grep/awk/sed that file instead of reading it whole")]
    async fn get_message(
        &self,
        #[tool(param)]
        #[schemars(description = "Message ID")]
        message_id: String,
        #[tool(param)]
        #[schemars(description = "Save attachments to /tmp/thunderbird-mcp/<message>/ and return their paths")]
        save_attachments: Option<bool>,
    ) -> Result<CallToolResult, McpError> {
        mail::get_message(&self.bridge, message_id, save_attachments).await
    }

    #[tool(description = "Get recent messages with optional date and unread filtering")]
    async fn get_recent_messages(
        &self,
        #[tool(param)]
        #[schemars(description = "Folder URI")]
        folder: Option<String>,
        #[tool(param)]
        #[schemars(description = "Number of messages, default 20")]
        limit: Option<u32>,
        #[tool(param)]
        #[schemars(description = "Return only unread messages")]
        unread_only: Option<bool>,
        #[tool(param)]
        #[schemars(description = "Return messages newer than this date (ISO 8601)")]
        since_date: Option<String>,
    ) -> Result<CallToolResult, McpError> {
        mail::get_recent_messages(&self.bridge, folder, limit, unread_only, since_date).await
    }

    #[tool(description = "Mark read/unread, flag/unflag, move between folders, or trash a message")]
    async fn update_message(
        &self,
        #[tool(param)]
        #[schemars(description = "Message ID")]
        message_id: String,
        #[tool(param)]
        #[schemars(description = "Mark as read (true) or unread (false)")]
        read: Option<bool>,
        #[tool(param)]
        #[schemars(description = "Flag or unflag")]
        flagged: Option<bool>,
        #[tool(param)]
        #[schemars(description = "Folder URI to move message to")]
        move_to: Option<String>,
        #[tool(param)]
        #[schemars(description = "Move to trash")]
        trash: Option<bool>,
    ) -> Result<CallToolResult, McpError> {
        mail::update_message(&self.bridge, message_id, read, flagged, move_to, trash).await
    }

    #[tool(description = "Delete messages — drafts are moved to Trash")]
    async fn delete_messages(
        &self,
        #[tool(param)]
        #[schemars(description = "Array of message IDs to delete")]
        message_ids: Vec<String>,
    ) -> Result<CallToolResult, McpError> {
        mail::delete_messages(&self.bridge, message_ids).await
    }

    #[tool(description = "Delete messages from a folder matching date/sender/query filters. folder_uri is required. At least one filter must be provided.")]
    async fn delete_messages_by_filter(
        &self,
        #[tool(param)]
        #[schemars(description = "Folder URI to delete from (required)")]
        folder_uri: String,
        #[tool(param)]
        #[schemars(description = "Delete messages older than this date (ISO 8601, e.g. 2026-01-01)")]
        date_to: Option<String>,
        #[tool(param)]
        #[schemars(description = "Delete messages on or after this date (ISO 8601, e.g. 2026-01-01); date-only strings start at midnight")]
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

    #[tool(description = "Create a new subfolder under a parent folder")]
    async fn create_folder(
        &self,
        #[tool(param)]
        #[schemars(description = "Parent folder URI")]
        parent_uri: String,
        #[tool(param)]
        #[schemars(description = "New folder name")]
        name: String,
    ) -> Result<CallToolResult, McpError> {
        mail::create_folder(&self.bridge, parent_uri, name).await
    }

    #[tool(description = "Delete a folder by moving it to the account Trash")]
    async fn delete_folder(
        &self,
        #[tool(param)]
        #[schemars(description = "URI of the folder to delete (moves to Trash)")]
        folder_uri: String,
    ) -> Result<CallToolResult, McpError> {
        mail::delete_folder(&self.bridge, folder_uri).await
    }

    #[tool(description = "Open a compose window with pre-filled recipients, subject, and body. Nothing sends without your review.")]
    async fn send_mail(
        &self,
        #[tool(param)]
        #[schemars(description = "Recipient addresses")]
        to: Vec<String>,
        #[tool(param)]
        #[schemars(description = "Email subject")]
        subject: String,
        #[tool(param)]
        #[schemars(description = "Email body (plain text)")]
        body: String,
        #[tool(param)]
        #[schemars(description = "CC addresses")]
        cc: Option<Vec<String>>,
        #[tool(param)]
        #[schemars(description = "BCC addresses")]
        bcc: Option<Vec<String>>,
        #[tool(param)]
        #[schemars(description = "From identity (email address)")]
        from_identity: Option<String>,
    ) -> Result<CallToolResult, McpError> {
        compose::send_mail(&self.bridge, to, subject, body, cc, bcc, from_identity).await
    }

    #[tool(description = "Reply to a message with quoted original. Opens compose window for review.")]
    async fn reply_to_message(
        &self,
        #[tool(param)]
        #[schemars(description = "Message ID to reply to")]
        message_id: String,
        #[tool(param)]
        #[schemars(description = "Reply body text")]
        body: String,
        #[tool(param)]
        #[schemars(description = "Reply to all recipients")]
        reply_all: Option<bool>,
    ) -> Result<CallToolResult, McpError> {
        compose::reply_to_message(&self.bridge, message_id, body, reply_all).await
    }

    #[tool(description = "Forward a message with all attachments. Opens compose window for review.")]
    async fn forward_message(
        &self,
        #[tool(param)]
        #[schemars(description = "Message ID to forward")]
        message_id: String,
        #[tool(param)]
        #[schemars(description = "Recipient addresses")]
        to: Vec<String>,
        #[tool(param)]
        #[schemars(description = "Optional forwarding note")]
        body: Option<String>,
    ) -> Result<CallToolResult, McpError> {
        compose::forward_message(&self.bridge, message_id, to, body).await
    }

    #[tool(description = "List all message filter rules with human-readable conditions and actions")]
    async fn list_filters(
        &self,
        #[tool(param)]
        #[schemars(description = "Account ID to list filters for")]
        account_id: Option<String>,
    ) -> Result<CallToolResult, McpError> {
        filters::list_filters(&self.bridge, account_id).await
    }

    #[tool(description = "Create a message filter with structured conditions and actions")]
    async fn create_filter(
        &self,
        #[tool(param)]
        #[schemars(description = "Account ID")]
        account_id: String,
        #[tool(param)]
        #[schemars(description = "Filter name")]
        name: String,
        #[tool(param)]
        #[schemars(description = "Array of condition objects with attrib/op/value (attrib: subject|from|body|to|cc|toOrCc; op: contains|doesntContain|is|isnt|beginsWith|endsWith); add booleanAnd: false to OR a condition with the previous one (default is AND)")]
        conditions: Value,
        #[tool(param)]
        #[schemars(description = "Array of action objects with type/value (type: moveToFolder|copyToFolder|markRead|markUnread|markFlagged|delete|addTag; moveToFolder/copyToFolder require value=folderURI)")]
        actions: Value,
        #[tool(param)]
        #[schemars(description = "Enable filter immediately (default true)")]
        enabled: Option<bool>,
    ) -> Result<CallToolResult, McpError> {
        filters::create_filter(&self.bridge, account_id, name, conditions, actions, enabled).await
    }

    #[tool(description = "Modify a filter's name, enabled state, conditions, or actions")]
    async fn update_filter(
        &self,
        #[tool(param)]
        #[schemars(description = "Account ID")]
        account_id: String,
        #[tool(param)]
        #[schemars(description = "Filter index (from list_filters)")]
        filter_index: u32,
        #[tool(param)]
        #[schemars(description = "New name")]
        name: Option<String>,
        #[tool(param)]
        #[schemars(description = "Enable or disable")]
        enabled: Option<bool>,
        #[tool(param)]
        #[schemars(description = "New conditions array")]
        conditions: Option<Value>,
        #[tool(param)]
        #[schemars(description = "New actions array")]
        actions: Option<Value>,
    ) -> Result<CallToolResult, McpError> {
        filters::update_filter(&self.bridge, account_id, filter_index, name, enabled, conditions, actions).await
    }

    #[tool(description = "Remove a filter by its index")]
    async fn delete_filter(
        &self,
        #[tool(param)]
        #[schemars(description = "Account ID")]
        account_id: String,
        #[tool(param)]
        #[schemars(description = "Filter index (from list_filters)")]
        filter_index: u32,
    ) -> Result<CallToolResult, McpError> {
        filters::delete_filter(&self.bridge, account_id, filter_index).await
    }

    #[tool(description = "Change filter execution priority by moving a filter to a new index")]
    async fn reorder_filters(
        &self,
        #[tool(param)]
        #[schemars(description = "Account ID")]
        account_id: String,
        #[tool(param)]
        #[schemars(description = "Current filter index")]
        from_index: u32,
        #[tool(param)]
        #[schemars(description = "Target filter index")]
        to_index: u32,
    ) -> Result<CallToolResult, McpError> {
        filters::reorder_filters(&self.bridge, account_id, from_index, to_index).await
    }

    #[tool(description = "Run all filters on a folder on demand")]
    async fn apply_filters(
        &self,
        #[tool(param)]
        #[schemars(description = "Account ID")]
        account_id: String,
        #[tool(param)]
        #[schemars(description = "Folder URI to run filters on")]
        folder_uri: String,
    ) -> Result<CallToolResult, McpError> {
        filters::apply_filters(&self.bridge, account_id, folder_uri).await
    }

    #[tool(description = "Search contacts across all address books")]
    async fn search_contacts(
        &self,
        #[tool(param)]
        #[schemars(description = "Name, email, or any contact field to search")]
        query: String,
        #[tool(param)]
        #[schemars(description = "Max results (default 20)")]
        limit: Option<u32>,
    ) -> Result<CallToolResult, McpError> {
        contacts::search_contacts(&self.bridge, query, limit).await
    }

    #[tool(description = "List all address books with their URIs, names, and read-only status")]
    async fn list_address_books(&self) -> Result<CallToolResult, McpError> {
        contacts::list_address_books(&self.bridge).await
    }

    #[tool(description = "Create a new contact in an address book")]
    async fn create_contact(
        &self,
        #[tool(param)]
        #[schemars(description = "URI of the address book to add the contact to (from list_address_books)")]
        address_book_uri: String,
        #[tool(param)]
        #[schemars(description = "Contact email address")]
        email: String,
        #[tool(param)]
        #[schemars(description = "First name")]
        first_name: Option<String>,
        #[tool(param)]
        #[schemars(description = "Last name")]
        last_name: Option<String>,
        #[tool(param)]
        #[schemars(description = "Display name (auto-derived from first/last name or email if omitted)")]
        display_name: Option<String>,
    ) -> Result<CallToolResult, McpError> {
        contacts::create_contact(&self.bridge, address_book_uri, email, first_name, last_name, display_name).await
    }

    #[tool(description = "Update fields on an existing contact")]
    async fn update_contact(
        &self,
        #[tool(param)]
        #[schemars(description = "Contact UID from search_contacts results")]
        contact_id: String,
        #[tool(param)]
        #[schemars(description = "New email address")]
        email: Option<String>,
        #[tool(param)]
        #[schemars(description = "New first name")]
        first_name: Option<String>,
        #[tool(param)]
        #[schemars(description = "New last name")]
        last_name: Option<String>,
        #[tool(param)]
        #[schemars(description = "New display name")]
        display_name: Option<String>,
    ) -> Result<CallToolResult, McpError> {
        contacts::update_contact(&self.bridge, contact_id, email, first_name, last_name, display_name).await
    }

    #[tool(description = "Permanently delete a contact from the address book. Pass confirm=true to execute; omit or pass false to preview what would be deleted.")]
    async fn delete_contact(
        &self,
        #[tool(param)]
        #[schemars(description = "Contact UID from search_contacts results")]
        contact_id: String,
        #[tool(param)]
        #[schemars(description = "Pass true to confirm permanent deletion; pass false to preview the contact without deleting")]
        confirm: bool,
    ) -> Result<CallToolResult, McpError> {
        contacts::delete_contact(&self.bridge, contact_id, confirm).await
    }

    #[tool(description = "List all calendars (local and CalDAV)")]
    async fn list_calendars(&self) -> Result<CallToolResult, McpError> {
        contacts::list_calendars(&self.bridge).await
    }

    #[tool(description = "Open a pre-filled calendar event dialog for review before saving")]
    async fn create_event(
        &self,
        #[tool(param)]
        #[schemars(description = "Calendar ID")]
        calendar_id: String,
        #[tool(param)]
        #[schemars(description = "Event title")]
        title: String,
        #[tool(param)]
        #[schemars(description = "Start time (ISO 8601)")]
        start: String,
        #[tool(param)]
        #[schemars(description = "End time (ISO 8601)")]
        end: String,
        #[tool(param)]
        #[schemars(description = "Event description")]
        description: Option<String>,
        #[tool(param)]
        #[schemars(description = "Event location")]
        location: Option<String>,
    ) -> Result<CallToolResult, McpError> {
        contacts::create_event(&self.bridge, calendar_id, title, start, end, description, location).await
    }

    #[tool(description = "List calendar events, optionally filtered by calendar and date range")]
    async fn list_events(
        &self,
        #[tool(param)]
        #[schemars(description = "Calendar ID to filter (omit for all calendars)")]
        calendar_id: Option<String>,
        #[tool(param)]
        #[schemars(description = "Start of date range (ISO 8601, e.g. 2026-02-01T00:00:00Z)")]
        date_from: Option<String>,
        #[tool(param)]
        #[schemars(description = "End of date range (ISO 8601, e.g. 2026-02-28T23:59:59Z)")]
        date_to: Option<String>,
        #[tool(param)]
        #[schemars(description = "Max events to return (default 50, max 200)")]
        limit: Option<u32>,
    ) -> Result<CallToolResult, McpError> {
        contacts::list_events(&self.bridge, calendar_id, date_from, date_to, limit).await
    }
}

#[tool(tool_box)]
impl ServerHandler for ThunderbirdMcp {
fn get_info(&self) -> ServerInfo {
        ServerInfo {
            capabilities: ServerCapabilities::builder().enable_tools().build(),
            ..Default::default()
        }
    }
}
