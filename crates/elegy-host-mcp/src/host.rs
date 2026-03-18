use crate::HostError;
use base64::Engine as _;
use elegy_core::{
    compose_runtime_state, CatalogResource, ProjectLocator, ReadResourceError, ResourceReadResult,
    RuntimeState,
};
use rmcp::{
    model::*, transport::stdio, ErrorData as McpError, RoleServer, ServerHandler, ServiceExt,
};
use serde_json::json;
use std::sync::Arc;
use tokio::task;

pub struct ElegyMcpHost {
    state: Arc<RuntimeState>,
}

impl ElegyMcpHost {
    pub fn new(state: RuntimeState) -> Self {
        Self {
            state: Arc::new(state),
        }
    }
}

pub async fn serve_stdio(locator: ProjectLocator) -> Result<(), HostError> {
    let state = compose_runtime_state(locator)?;
    let server = ElegyMcpHost::new(state).serve(stdio()).await?;
    server.waiting().await?;
    Ok(())
}

impl ServerHandler for ElegyMcpHost {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(ServerCapabilities::builder().enable_resources().build())
            .with_server_info(Implementation::from_build_env())
            .with_instructions(
                "Elegy exposes runtime-composed MCP resources over stdio. This host currently supports resources/list, resources/read, and an empty resources/templates surface."
                    .to_string(),
            )
    }

    async fn list_resources(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: rmcp::service::RequestContext<RoleServer>,
    ) -> Result<ListResourcesResult, McpError> {
        Ok(ListResourcesResult {
            resources: self
                .state
                .catalog()
                .resources
                .iter()
                .map(resource_to_mcp)
                .collect(),
            next_cursor: None,
            meta: None,
        })
    }

    async fn list_resource_templates(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: rmcp::service::RequestContext<RoleServer>,
    ) -> Result<ListResourceTemplatesResult, McpError> {
        Ok(ListResourceTemplatesResult {
            resource_templates: Vec::new(),
            next_cursor: None,
            meta: None,
        })
    }

    async fn read_resource(
        &self,
        request: ReadResourceRequestParams,
        _context: rmcp::service::RequestContext<RoleServer>,
    ) -> Result<ReadResourceResult, McpError> {
        let uri = request.uri;
        let state = Arc::clone(&self.state);
        let read_uri = uri.clone();
        let result = task::spawn_blocking(move || state.read_resource(read_uri.as_str()))
            .await
            .map_err(|_| {
                McpError::internal_error("resource read task failed", Some(json!({ "uri": uri })))
            })?;
        match result {
            Ok(result) => Ok(ReadResourceResult::new(vec![
                resource_contents_from_read_result(result),
            ])),
            Err(error) => Err(map_read_error(uri.as_str(), error)),
        }
    }
}

fn resource_to_mcp(resource: &CatalogResource) -> Resource {
    let mut raw = RawResource::new(
        resource.uri.clone(),
        resource
            .title
            .clone()
            .unwrap_or_else(|| resource.id.clone()),
    )
    .with_mime_type(resource.mime_type.clone());

    if let Some(title) = &resource.title {
        raw = raw.with_title(title.clone());
    }
    if let Some(description) = &resource.description {
        raw = raw.with_description(description.clone());
    }
    if let Ok(size) = u32::try_from(resource.limits.max_size_bytes) {
        raw = raw.with_size(size);
    }

    raw.no_annotation()
}

fn resource_contents_from_read_result(result: ResourceReadResult) -> ResourceContents {
    if mime_type_is_textual(&result.mime_type) {
        match String::from_utf8(result.bytes) {
            Ok(text) => ResourceContents::text(text, result.uri).with_mime_type(result.mime_type),
            Err(error) => ResourceContents::blob(
                base64::engine::general_purpose::STANDARD.encode(error.into_bytes()),
                result.uri,
            )
            .with_mime_type(result.mime_type),
        }
    } else {
        ResourceContents::blob(
            base64::engine::general_purpose::STANDARD.encode(result.bytes),
            result.uri,
        )
        .with_mime_type(result.mime_type)
    }
}

fn mime_type_is_textual(mime_type: &str) -> bool {
    let essence = mime_type
        .split(';')
        .next()
        .unwrap_or_default()
        .trim()
        .to_ascii_lowercase();

    essence.starts_with("text/")
        || matches!(
            essence.as_str(),
            "application/json"
                | "application/xml"
                | "application/yaml"
                | "application/x-yaml"
                | "application/javascript"
                | "application/ecmascript"
                | "image/svg+xml"
        )
        || essence.ends_with("+json")
        || essence.ends_with("+xml")
}

fn map_read_error(uri: &str, error: ReadResourceError) -> McpError {
    match error {
        ReadResourceError::UnknownResource { .. } => {
            McpError::resource_not_found("resource_not_found", Some(json!({ "uri": uri })))
        }
        other => McpError::internal_error(
            read_error_message(&other),
            Some(json!({
                "uri": uri,
            })),
        ),
    }
}

fn read_error_message(error: &ReadResourceError) -> &'static str {
    match error {
        ReadResourceError::AccessDenied { .. } => "resource access denied",
        ReadResourceError::InvalidResourceState { .. } => "resource state is invalid",
        ReadResourceError::Io { .. } => "resource read failed",
        ReadResourceError::Http(_) => "resource HTTP read failed",
        ReadResourceError::NotYetSupported { .. } => {
            "resource family is not supported by this host"
        }
        ReadResourceError::UnknownResource { .. } => "resource not found",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use elegy_core::ProjectLocator;
    use rmcp::{ClientHandler, ServiceExt};
    use std::path::PathBuf;

    #[derive(Default, Clone)]
    struct TestClient;

    impl ClientHandler for TestClient {}

    fn repo_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(|path| path.parent())
            .expect("workspace root")
            .to_path_buf()
    }

    #[test]
    fn read_result_uses_blob_for_binary_mime_even_when_bytes_are_utf8() {
        let result = resource_contents_from_read_result(ResourceReadResult {
            uri: "elegy://tests/resource/blob".to_string(),
            mime_type: "application/octet-stream".to_string(),
            bytes: b"plain text bytes".to_vec(),
            http_response: None,
        });

        assert_eq!(
            result,
            ResourceContents::blob(
                base64::engine::general_purpose::STANDARD.encode("plain text bytes"),
                "elegy://tests/resource/blob",
            )
            .with_mime_type("application/octet-stream")
        );
    }

    #[test]
    fn read_result_uses_text_for_utf8_json_payloads() {
        let result = resource_contents_from_read_result(ResourceReadResult {
            uri: "elegy://tests/resource/json".to_string(),
            mime_type: "application/json".to_string(),
            bytes: br#"{"status":"ok"}"#.to_vec(),
            http_response: None,
        });

        assert_eq!(
            result,
            ResourceContents::text(r#"{"status":"ok"}"#, "elegy://tests/resource/json")
                .with_mime_type("application/json")
        );
    }

    #[tokio::test]
    async fn host_lists_supported_resources_over_duplex_transport() {
        let state = compose_runtime_state(ProjectLocator::Path(
            repo_root().join("examples/http-minimal"),
        ))
        .expect("example runtime should compose");
        let server = ElegyMcpHost::new(state);
        let client = TestClient;
        let (server_transport, client_transport) = tokio::io::duplex(4096);

        let server_task = tokio::spawn(async move {
            let service = server
                .serve(server_transport)
                .await
                .expect("server should initialize");
            service.waiting().await.expect("server should run cleanly");
        });

        let client_service = client
            .serve(client_transport)
            .await
            .expect("client should initialize");
        let resources = client_service
            .list_all_resources()
            .await
            .expect("client should list resources");

        assert_eq!(resources.len(), 1);
        assert_eq!(resources[0].uri, "elegy://http-minimal/resource/status");
        assert_eq!(
            resources[0].mime_type.as_deref(),
            Some("application/octet-stream")
        );

        client_service.cancel().await.expect("client should cancel");
        server_task.await.expect("server task should join");
    }

    #[tokio::test]
    async fn host_reads_static_resource_over_duplex_transport() {
        let state = compose_runtime_state(ProjectLocator::Path(
            repo_root().join("examples/fs-static-minimal"),
        ))
        .expect("example runtime should compose");
        let server = ElegyMcpHost::new(state);
        let client = TestClient;
        let (server_transport, client_transport) = tokio::io::duplex(4096);

        let server_task = tokio::spawn(async move {
            let service = server
                .serve(server_transport)
                .await
                .expect("server should initialize");
            service.waiting().await.expect("server should run cleanly");
        });

        let client_service = client
            .serve(client_transport)
            .await
            .expect("client should initialize");
        let result = client_service
            .read_resource(ReadResourceRequestParams::new(
                "elegy://fs-static-minimal/resource/welcome",
            ))
            .await
            .expect("client should read static resource");

        assert_eq!(result.contents.len(), 1);
        assert_eq!(
            result.contents[0],
            ResourceContents::text(
                "Hello from Elegy.\n",
                "elegy://fs-static-minimal/resource/welcome"
            )
            .with_mime_type("text/plain; charset=utf-8")
        );

        client_service.cancel().await.expect("client should cancel");
        server_task.await.expect("server task should join");
    }
}
