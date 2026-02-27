mod auth;
mod bridge;
mod sanitize;
mod server;
mod tools;

use anyhow::Context;
use bridge::Bridge;
use server::ThunderbirdMcp;
use std::sync::Arc;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Read auth token written by the Thunderbird extension on startup
    let token = auth::find_token()
        .context("Is Thunderbird running with the MCP extension installed?")?;

    let bridge = Arc::new(Bridge::new(token));
    let handler = ThunderbirdMcp { bridge };

    // Start MCP server on stdio (Claude connects via stdin/stdout)
    let service = rmcp::serve_server(handler, rmcp::transport::stdio())
        .await
        .context("Failed to start MCP server")?;

    // Wait for the client to disconnect (EOF on stdin)
    service.waiting().await?;

    Ok(())
}
