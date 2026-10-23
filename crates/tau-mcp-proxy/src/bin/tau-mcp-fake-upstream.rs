use std::sync::atomic::{AtomicU64, Ordering};

use rmcp::{ErrorData, RoleServer, ServerHandler, ServiceExt, model::*, service::RequestContext};
use serde_json::json;

#[derive(Default)]
struct Fake {
    calls: AtomicU64,
}

impl ServerHandler for Fake {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(
            ServerCapabilities::builder()
                .enable_tools()
                .enable_tool_list_changed()
                .build(),
        )
        .with_protocol_version(ProtocolVersion::V_2025_11_25)
        .with_server_info(Implementation::new("tau-mcp-fake-upstream", "0.1.0"))
        .with_instructions("fake upstream instructions")
    }

    async fn list_tools(
        &self,
        request: Option<PaginatedRequestParams>,
        _: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, ErrorData> {
        let names = match request.as_ref().and_then(|r| r.cursor.as_deref()) {
            Some("page-two") => vec!["crash"],
            _ => vec!["whoami", "crash"],
        };
        Ok(ListToolsResult {
            tools: names
                .into_iter()
                .map(|name| {
                    Tool::new(
                        name,
                        name,
                        std::sync::Arc::new(json!({"type":"object"}).as_object().unwrap().clone()),
                    )
                })
                .collect(),
            next_cursor: request
                .as_ref()
                .is_none_or(|r| r.cursor.is_none())
                .then(|| "page-two".into()),
            ..Default::default()
        })
    }

    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        context: RequestContext<RoleServer>,
    ) -> Result<CallToolResponse, ErrorData> {
        match request.name.as_ref() {
            "crash" => std::process::exit(1),
            "whoami" => {}
            _ => return Err(ErrorData::invalid_params("unknown tool", None)),
        }
        if let Some(args) = &request.arguments {
            if args.get("notify").and_then(|v| v.as_bool()) == Some(true) {
                context
                    .peer
                    .notify_tool_list_changed()
                    .await
                    .map_err(|e| ErrorData::internal_error(e.to_string(), None))?;
            }
            if let Some(ms) = args.get("sleep_ms").and_then(|v| v.as_u64()) {
                tokio::time::sleep(std::time::Duration::from_millis(ms)).await;
            }
        }
        let value = json!({
            "session": std::process::id(),
            "calls": self.calls.fetch_add(1, Ordering::Relaxed) + 1,
            "meta": context.meta,
        });
        let mut result = CallToolResult::success(vec![ContentBlock::text(value.to_string())]);
        result.structured_content = Some(value);
        result.meta = Some(
            json!({"upstream": true})
                .as_object()
                .unwrap()
                .clone()
                .into(),
        );
        Ok(result.into())
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let marker = std::env::var_os("TAUCODE_FAKE_LIFECYCLE_DIR")
        .map(|dir| std::path::PathBuf::from(dir).join(std::process::id().to_string()));
    if let Some(path) = &marker {
        std::fs::write(path, b"running")?;
    }
    Fake::default()
        .serve(rmcp::transport::stdio())
        .await?
        .waiting()
        .await?;
    if let Some(path) = marker {
        std::fs::remove_file(path)?;
    }
    Ok(())
}
