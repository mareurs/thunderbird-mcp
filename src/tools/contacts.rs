use rmcp::{model::CallToolResult, Error as McpError};
use serde_json::json;
use crate::bridge::Bridge;
use super::mail::{bridge_err, result_text};

pub async fn search_contacts(
    bridge: &Bridge,
    query: String,
    limit: Option<u32>,
) -> Result<CallToolResult, McpError> {
    let r = bridge.call("/contacts/search", json!({
        "query": query, "limit": limit
    })).await.map_err(bridge_err)?;
    Ok(result_text(r))
}

pub async fn list_address_books(bridge: &Bridge) -> Result<CallToolResult, McpError> {
    let r = bridge.call("/contacts/list-books", json!({})).await.map_err(bridge_err)?;
    Ok(result_text(r))
}

pub async fn create_contact(
    bridge: &Bridge,
    address_book_uri: String,
    email: String,
    first_name: Option<String>,
    last_name: Option<String>,
    display_name: Option<String>,
) -> Result<CallToolResult, McpError> {
    let r = bridge.call("/contacts/create", json!({
        "address_book_uri": address_book_uri,
        "email": email,
        "first_name": first_name,
        "last_name": last_name,
        "display_name": display_name,
    })).await.map_err(bridge_err)?;
    Ok(result_text(r))
}

pub async fn update_contact(
    bridge: &Bridge,
    contact_id: String,
    email: Option<String>,
    first_name: Option<String>,
    last_name: Option<String>,
    display_name: Option<String>,
) -> Result<CallToolResult, McpError> {
    let r = bridge.call("/contacts/update", json!({
        "contact_id": contact_id,
        "email": email,
        "first_name": first_name,
        "last_name": last_name,
        "display_name": display_name,
    })).await.map_err(bridge_err)?;
    Ok(result_text(r))
}

pub async fn list_calendars(bridge: &Bridge) -> Result<CallToolResult, McpError> {
    let r = bridge.call("/calendars/list", json!({})).await.map_err(bridge_err)?;
    Ok(result_text(r))
}

pub async fn create_event(
    bridge: &Bridge,
    calendar_id: String,
    title: String,
    start: String,
    end: String,
    description: Option<String>,
    location: Option<String>,
) -> Result<CallToolResult, McpError> {
    let r = bridge.call("/calendar/create-event", json!({
        "calendar_id": calendar_id, "title": title,
        "start": start, "end": end,
        "description": description, "location": location
    })).await.map_err(bridge_err)?;
    Ok(result_text(r))
}

// TODO: this module has grown beyond contacts — rename to calendar.rs when extracting contacts
pub async fn list_events(
    bridge: &Bridge,
    calendar_id: Option<String>,
    date_from: Option<String>,
    date_to: Option<String>,
    limit: Option<u32>,
) -> Result<CallToolResult, McpError> {
    let r = bridge.call("/calendars/list-events", json!({
        "calendar_id": calendar_id, "date_from": date_from, "date_to": date_to,
        "limit": limit
    })).await.map_err(bridge_err)?;
    Ok(result_text(r))
}

#[cfg(test)]
mod tests {
    use super::*;
    use mockito::Server;
    use serde_json::json;

    async fn mock_bridge(server: &Server) -> Bridge {
        Bridge::with_base_url("tok".to_string(), server.url())
    }

    #[tokio::test]
    async fn search_contacts_sends_query_and_limit() {
        let mut server = Server::new_async().await;
        let mock = server.mock("POST", "/contacts/search")
            .match_body(mockito::Matcher::PartialJson(json!({"query": "alice", "limit": 10})))
            .with_status(200).with_body("{}").create_async().await;
        search_contacts(&mock_bridge(&server).await, "alice".into(), Some(10)).await.unwrap();
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn list_calendars_hits_correct_endpoint() {
        let mut server = Server::new_async().await;
        let mock = server.mock("POST", "/calendars/list")
            .with_status(200).with_body("{}").create_async().await;
        list_calendars(&mock_bridge(&server).await).await.unwrap();
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn create_event_sends_required_fields() {
        let mut server = Server::new_async().await;
        let mock = server.mock("POST", "/calendar/create-event")
            .match_body(mockito::Matcher::PartialJson(json!({
                "calendar_id": "cal-1",
                "title": "Team sync",
                "start": "2026-03-10T10:00:00Z",
                "end": "2026-03-10T11:00:00Z"
            })))
            .with_status(200).with_body("{}").create_async().await;
        create_event(
            &mock_bridge(&server).await,
            "cal-1".into(), "Team sync".into(),
            "2026-03-10T10:00:00Z".into(), "2026-03-10T11:00:00Z".into(),
            None, None,
        ).await.unwrap();
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn list_events_sends_date_range_and_limit() {
        let mut server = Server::new_async().await;
        let mock = server.mock("POST", "/calendars/list-events")
            .match_body(mockito::Matcher::PartialJson(json!({
                "date_from": "2026-03-01T00:00:00Z",
                "date_to": "2026-03-31T23:59:59Z",
                "limit": 50
            })))
            .with_status(200).with_body("{}").create_async().await;
        list_events(
            &mock_bridge(&server).await,
            None,
            Some("2026-03-01T00:00:00Z".into()),
            Some("2026-03-31T23:59:59Z".into()),
            Some(50),
        ).await.unwrap();
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn list_address_books_hits_correct_endpoint() {
        let mut server = Server::new_async().await;
        let mock = server.mock("POST", "/contacts/list-books")
            .with_status(200).with_body("[]").create_async().await;
        list_address_books(&mock_bridge(&server).await).await.unwrap();
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn list_address_books_propagates_bridge_error() {
        let mut server = Server::new_async().await;
        let _mock = server.mock("POST", "/contacts/list-books")
            .with_status(200)
            .with_body("{\"error\": \"unavailable\"}")
            .create_async().await;
        let result = list_address_books(&mock_bridge(&server).await).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn create_contact_sends_required_fields() {
        let mut server = Server::new_async().await;
        let mock = server.mock("POST", "/contacts/create")
            .match_body(mockito::Matcher::PartialJson(json!({
                "address_book_uri": "moz-abmdbdirectory://abook.mab",
                "email": "alice@example.com"
            })))
            .with_status(200).with_body("{}").create_async().await;
        create_contact(
            &mock_bridge(&server).await,
            "moz-abmdbdirectory://abook.mab".into(),
            "alice@example.com".into(),
            None, None, None,
        ).await.unwrap();
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn create_contact_propagates_bridge_error() {
        let mut server = Server::new_async().await;
        let _mock = server.mock("POST", "/contacts/create")
            .with_status(200)
            .with_body("{\"error\": \"address book not found\"}")
            .create_async().await;
        let result = create_contact(
            &mock_bridge(&server).await,
            "moz-abmdbdirectory://abook.mab".into(),
            "alice@example.com".into(),
            None, None, None,
        ).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn update_contact_sends_contact_id_and_fields() {
        let mut server = Server::new_async().await;
        let mock = server.mock("POST", "/contacts/update")
            .match_body(mockito::Matcher::PartialJson(json!({
                "contact_id": "uid-123",
                "email": "new@example.com"
            })))
            .with_status(200).with_body("{}").create_async().await;
        update_contact(
            &mock_bridge(&server).await,
            "uid-123".into(),
            Some("new@example.com".into()),
            None, None, None,
        ).await.unwrap();
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn update_contact_propagates_bridge_error() {
        let mut server = Server::new_async().await;
        let _mock = server.mock("POST", "/contacts/update")
            .with_status(200)
            .with_body("{\"error\": \"contact not found\"}")
            .create_async().await;
        let result = update_contact(
            &mock_bridge(&server).await,
            "uid-123".into(),
            None, None, None, None,
        ).await;
        assert!(result.is_err());
    }

}
