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

    #[tool(description = "Search messages by subject, sender, recipient, date range or folder")]
    async fn search_messages(
        &self,
        #[tool(param)]
        #[schemars(description = "Text to search in subject/body")]
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
    ) -> Result<CallToolResult, McpError> {
        mail::search_messages(&self.bridge, query, folder, sender, recipient, date_from, date_to, max_results).await
    }

    #[tool(description = "Read full email content, optionally save attachments to disk")]
    async fn get_message(
        &self,
        #[tool(param)]
        #[schemars(description = "Message ID")]
        message_id: String,
        #[tool(param)]
        #[schemars(description = "Save attachments to ~/thunderbird-mcp-attachments/")]
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
        #[schemars(description = "Array of condition objects with field/op/value")]
        conditions: Value,
        #[tool(param)]
        #[schemars(description = "Array of action objects with type/value")]
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
