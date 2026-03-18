use elegy_adapter_fs::{
    compose_filesystem_resource, read_filesystem_resource, read_static_resource,
    resolve_allowed_roots, try_compose_static_resource, FsReadError, FsResolvedFilesystemResource,
    FsResolvedStaticResource,
};
use elegy_adapter_http::{
    compose_http_resource, read_http_resource, validate_http_policy, HttpClient, HttpReadError,
    HttpResolvedResource, ReqwestHttpClient,
};
#[cfg(test)]
use elegy_adapter_http::{HttpClientError, HttpRequest, HttpResponse};
use elegy_descriptor::{Diagnostic, LoadedProject, NormalizedResource, ResourceFamily};
use elegy_policy::{FilesystemPolicy, HttpPolicy, PolicyConfig};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt;
use std::path::PathBuf;

pub const MCP_SPEC_BASELINE: &str = "2025-11-25";

#[derive(Clone, Debug)]
pub struct CompositionError {
    diagnostics: Vec<Diagnostic>,
}

impl CompositionError {
    pub fn new(mut diagnostics: Vec<Diagnostic>) -> Self {
        diagnostics.sort();
        diagnostics.dedup();
        Self { diagnostics }
    }

    pub fn diagnostics(&self) -> &[Diagnostic] {
        &self.diagnostics
    }
}

impl fmt::Display for CompositionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "runtime composition failed with {} diagnostic(s)",
            self.diagnostics.len()
        )
    }
}

impl std::error::Error for CompositionError {}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Catalog {
    pub spec_baseline: String,
    pub project_name: String,
    pub resource_count: usize,
    pub policy_summary: CatalogPolicySummary,
    pub resources: Vec<CatalogResource>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct CatalogPolicySummary {
    pub filesystem_root_count: usize,
    pub filesystem_max_file_size_bytes: u64,
    pub filesystem_allow_symlinks: bool,
    pub http_target_count: usize,
    pub http_allow_plaintext_http: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct CatalogResource {
    pub id: String,
    pub uri: String,
    pub family: ResourceFamily,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub mime_type: String,
    pub source: CatalogSource,
    pub limits: ResourceLimits,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum CatalogSource {
    Inline {
        descriptor: String,
    },
    Filesystem {
        descriptor: String,
        root: String,
        path: String,
    },
    Http {
        descriptor: String,
        base_url: String,
        path: String,
        method: String,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ResourceLimits {
    pub max_size_bytes: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResourceReadResult {
    pub uri: String,
    pub mime_type: String,
    pub bytes: Vec<u8>,
    pub http_response: Option<HttpReadMetadata>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HttpReadMetadata {
    pub target_url: String,
    pub status_code: u16,
    pub content_type: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ReadResourceError {
    UnknownResource {
        uri: String,
    },
    AccessDenied {
        uri: String,
        message: String,
    },
    InvalidResourceState {
        uri: String,
        message: String,
    },
    Io {
        uri: String,
        message: String,
    },
    Http(HttpReadError),
    NotYetSupported {
        uri: String,
        family: ResourceFamily,
        message: String,
    },
}

impl fmt::Display for ReadResourceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownResource { uri } => write!(f, "unknown resource URI {uri:?}"),
            Self::AccessDenied { uri, message } => {
                write!(f, "access denied for resource URI {uri:?}: {message}")
            }
            Self::InvalidResourceState { uri, message } => {
                write!(f, "invalid resource state for URI {uri:?}: {message}")
            }
            Self::Io { uri, message } => {
                write!(f, "I/O failure for resource URI {uri:?}: {message}")
            }
            Self::Http(error) => write!(f, "{error}"),
            Self::NotYetSupported {
                uri,
                family,
                message,
            } => write!(
                f,
                "resource URI {uri:?} with family {family:?} is not yet supported: {message}"
            ),
        }
    }
}

impl std::error::Error for ReadResourceError {}

pub struct RuntimeState {
    catalog: Catalog,
    project_root: PathBuf,
    allowed_roots: Vec<PathBuf>,
    filesystem_policy: FilesystemPolicy,
    http_policy: HttpPolicy,
    entries: Vec<ResolvedResource>,
    uri_index: BTreeMap<String, usize>,
}

impl RuntimeState {
    pub fn catalog(&self) -> &Catalog {
        &self.catalog
    }

    pub fn read_resource(&self, uri: &str) -> Result<ResourceReadResult, ReadResourceError> {
        self.read_resource_with_http_client(uri, &ReqwestHttpClient)
    }

    fn read_resource_with_http_client<C: HttpClient>(
        &self,
        uri: &str,
        client: &C,
    ) -> Result<ResourceReadResult, ReadResourceError> {
        let Some(index) = self.uri_index.get(uri).copied() else {
            return Err(ReadResourceError::UnknownResource {
                uri: uri.to_string(),
            });
        };

        match &self.entries[index] {
            ResolvedResource::Static(entry) => Ok(ResourceReadResult {
                uri: entry.catalog.uri.clone(),
                mime_type: entry.catalog.mime_type.clone(),
                bytes: read_static_resource(&entry.resource),
                http_response: None,
            }),
            ResolvedResource::Filesystem(entry) => {
                let bytes = read_filesystem_resource(
                    &self.project_root,
                    &self.allowed_roots,
                    &self.filesystem_policy,
                    &entry.resource,
                )
                .map_err(map_fs_read_error)?;

                Ok(ResourceReadResult {
                    uri: entry.catalog.uri.clone(),
                    mime_type: entry.catalog.mime_type.clone(),
                    bytes,
                    http_response: None,
                })
            }
            ResolvedResource::Http(entry) => {
                let result = read_http_resource(&self.http_policy, &entry.resource, client)
                    .map_err(ReadResourceError::Http)?;

                Ok(ResourceReadResult {
                    uri: entry.catalog.uri.clone(),
                    mime_type: result.mime_type,
                    bytes: result.bytes,
                    http_response: Some(HttpReadMetadata {
                        target_url: result.target_url,
                        status_code: result.status_code,
                        content_type: result.content_type,
                    }),
                })
            }
        }
    }
}

pub fn compose_catalog(
    project: &LoadedProject,
    policy: &PolicyConfig,
) -> Result<Catalog, CompositionError> {
    Ok(compose_runtime_state(project, policy)?.catalog)
}

pub fn compose_runtime_state(
    project: &LoadedProject,
    policy: &PolicyConfig,
) -> Result<RuntimeState, CompositionError> {
    let allowed_roots = resolve_allowed_roots(&project.project_root, &policy.filesystem)
        .map_err(CompositionError::new)?;
    validate_http_policy(&policy.http).map_err(CompositionError::new)?;

    let mut diagnostics = Vec::new();
    let mut seen_ids = BTreeMap::<String, String>::new();
    let mut seen_uris = BTreeMap::<String, String>::new();
    let mut entries = Vec::new();

    for resource in &project.resources {
        let id = resource.id().to_string();
        let uri = resource.uri().to_string();

        if let Some(existing_uri) = seen_ids.insert(id.clone(), uri.clone()) {
            diagnostics.push(
                Diagnostic::error(
                    "RUNTIME-DUPLICATE-ID-001",
                    format!("resource id {id:?} is already bound to URI {existing_uri:?}"),
                )
                .with_path(resource.descriptor_path().to_string())
                .with_field("resources[].id"),
            );
            continue;
        }

        if let Some(existing_id) = seen_uris.insert(uri.clone(), id.clone()) {
            diagnostics.push(
                Diagnostic::error(
                    "RUNTIME-DUPLICATE-URI-001",
                    format!(
                        "resource URI {uri:?} is already declared by resource id {existing_id:?}"
                    ),
                )
                .with_path(resource.descriptor_path().to_string())
                .with_field("resources[].uri"),
            );
            continue;
        }

        match resource {
            NormalizedResource::Static(resource) => {
                match try_compose_static_resource(resource, &policy.filesystem) {
                    Ok(resolved) => {
                        entries.push(ResolvedResource::Static(ResolvedStaticEntry {
                            catalog: catalog_from_static_resource(&resolved),
                            resource: resolved,
                        }));
                    }
                    Err(error) => diagnostics.extend(error),
                }
            }
            NormalizedResource::Filesystem(resource) => match compose_filesystem_resource(
                &project.project_root,
                &allowed_roots,
                &policy.filesystem,
                resource,
            ) {
                Ok(resolved) => {
                    entries.push(ResolvedResource::Filesystem(ResolvedFilesystemEntry {
                        catalog: catalog_from_filesystem_resource(&resolved),
                        resource: resolved,
                    }));
                }
                Err(error) => diagnostics.extend(error),
            },
            NormalizedResource::Http(resource) => {
                match compose_http_resource(&policy.http, resource) {
                    Ok(resolved) => {
                        entries.push(ResolvedResource::Http(ResolvedHttpEntry {
                            catalog: catalog_from_http_resource(&resolved),
                            resource: resolved,
                        }));
                    }
                    Err(error) => diagnostics.extend(error),
                }
            }
            NormalizedResource::OpenApi(resource) => {
                diagnostics.push(unsupported_family_diagnostic(
                    "RUNTIME-UNSUPPORTED-FAMILY-002",
                    "open_api",
                    &resource.id,
                    &resource.descriptor_path,
                ))
            }
        }
    }

    if !diagnostics.is_empty() {
        return Err(CompositionError::new(diagnostics));
    }

    entries.sort_by(|left, right| {
        left.catalog()
            .uri
            .cmp(&right.catalog().uri)
            .then_with(|| left.catalog().id.cmp(&right.catalog().id))
            .then_with(|| left.catalog().family.cmp(&right.catalog().family))
    });

    let resources: Vec<CatalogResource> = entries
        .iter()
        .map(ResolvedResource::catalog)
        .cloned()
        .collect();
    let uri_index = resources
        .iter()
        .enumerate()
        .map(|(index, resource)| (resource.uri.clone(), index))
        .collect();

    Ok(RuntimeState {
        catalog: Catalog {
            spec_baseline: MCP_SPEC_BASELINE.to_string(),
            project_name: project.config.project_name.clone(),
            resource_count: resources.len(),
            policy_summary: CatalogPolicySummary {
                filesystem_root_count: policy.filesystem.roots.len(),
                filesystem_max_file_size_bytes: policy.filesystem.max_file_size_bytes,
                filesystem_allow_symlinks: policy.filesystem.allow_symlinks,
                http_target_count: policy.http.allowed_targets.len(),
                http_allow_plaintext_http: policy.http.allow_plaintext_http,
            },
            resources,
        },
        project_root: project.project_root.clone(),
        allowed_roots,
        filesystem_policy: policy.filesystem.clone(),
        http_policy: policy.http.clone(),
        entries,
        uri_index,
    })
}

fn catalog_from_static_resource(resource: &FsResolvedStaticResource) -> CatalogResource {
    CatalogResource {
        id: resource.id.clone(),
        uri: resource.uri.clone(),
        family: ResourceFamily::Static,
        title: resource.title.clone(),
        description: resource.description.clone(),
        mime_type: resource.mime_type.clone(),
        source: CatalogSource::Inline {
            descriptor: resource.descriptor_path.clone(),
        },
        limits: ResourceLimits {
            max_size_bytes: resource.max_size_bytes,
        },
    }
}

fn catalog_from_filesystem_resource(resource: &FsResolvedFilesystemResource) -> CatalogResource {
    CatalogResource {
        id: resource.id.clone(),
        uri: resource.uri.clone(),
        family: ResourceFamily::Filesystem,
        title: resource.title.clone(),
        description: resource.description.clone(),
        mime_type: resource.mime_type.clone(),
        source: CatalogSource::Filesystem {
            descriptor: resource.descriptor_path.clone(),
            root: resource.root.clone(),
            path: resource.path.clone(),
        },
        limits: ResourceLimits {
            max_size_bytes: resource.max_size_bytes,
        },
    }
}

fn catalog_from_http_resource(resource: &HttpResolvedResource) -> CatalogResource {
    CatalogResource {
        id: resource.id.clone(),
        uri: resource.uri.clone(),
        family: ResourceFamily::Http,
        title: resource.title.clone(),
        description: resource.description.clone(),
        mime_type: resource.mime_type.clone(),
        source: CatalogSource::Http {
            descriptor: resource.descriptor_path.clone(),
            base_url: resource.base_url.clone(),
            path: resource.path.clone(),
            method: resource.method.clone(),
        },
        limits: ResourceLimits {
            max_size_bytes: resource.max_size_bytes,
        },
    }
}

fn map_fs_read_error(error: FsReadError) -> ReadResourceError {
    match error {
        FsReadError::AccessDenied { uri, message } => {
            ReadResourceError::AccessDenied { uri, message }
        }
        FsReadError::InvalidResourceState { uri, message } => {
            ReadResourceError::InvalidResourceState { uri, message }
        }
        FsReadError::Io { uri, message } => ReadResourceError::Io { uri, message },
    }
}

fn unsupported_family_diagnostic(
    code: &str,
    family: &str,
    resource_id: &str,
    descriptor_path: &str,
) -> Diagnostic {
    Diagnostic::error(
        code,
        format!(
            "resource family {family} is scaffold-only in the current runtime slice; descriptor validation accepted resource id {resource_id:?}, but runtime composition does not execute that family yet"
        ),
    )
    .with_path(descriptor_path.to_string())
    .with_field("resources[].kind")
    .with_hint("plain constrained HTTP resources are supported; OpenAPI-derived runtime execution remains deferred")
}

enum ResolvedResource {
    Static(ResolvedStaticEntry),
    Filesystem(ResolvedFilesystemEntry),
    Http(ResolvedHttpEntry),
}

impl ResolvedResource {
    fn catalog(&self) -> &CatalogResource {
        match self {
            Self::Static(entry) => &entry.catalog,
            Self::Filesystem(entry) => &entry.catalog,
            Self::Http(entry) => &entry.catalog,
        }
    }
}

struct ResolvedStaticEntry {
    catalog: CatalogResource,
    resource: FsResolvedStaticResource,
}

struct ResolvedFilesystemEntry {
    catalog: CatalogResource,
    resource: FsResolvedFilesystemResource,
}

struct ResolvedHttpEntry {
    catalog: CatalogResource,
    resource: HttpResolvedResource,
}

#[cfg(test)]
mod tests {
    use super::*;
    use elegy_descriptor::{load_project_from_root_config, StaticResource};

    fn repo_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(|path| path.parent())
            .expect("workspace root")
            .to_path_buf()
    }

    fn policy_from_loaded(project: &LoadedProject) -> PolicyConfig {
        PolicyConfig {
            filesystem: FilesystemPolicy {
                roots: project.config.policy.filesystem.roots.clone(),
                max_file_size_bytes: project.config.policy.filesystem.max_file_size_bytes,
                allow_symlinks: project.config.policy.filesystem.allow_symlinks,
            },
            http: HttpPolicy {
                allowed_targets: project.config.policy.http.allowed_targets.clone(),
                allow_plaintext_http: project.config.policy.http.allow_plaintext_http,
                timeout_ms: project.config.policy.http.timeout_ms,
                max_response_size_bytes: project.config.policy.http.max_response_size_bytes,
            },
        }
    }

    fn http_example_runtime_state() -> RuntimeState {
        let example = repo_root().join("examples/http-minimal");
        let loaded = load_project_from_root_config(&example.join("elegy.toml"))
            .expect("example config should validate");
        let policy = policy_from_loaded(&loaded);

        compose_runtime_state(&loaded, &policy).expect("HTTP example should compose")
    }

    #[test]
    fn oversized_inline_static_resources_return_diagnostics_instead_of_panicking() {
        let example = repo_root().join("examples/fs-static-minimal");
        let mut loaded = load_project_from_root_config(&example.join("elegy.toml"))
            .expect("example config should validate");
        loaded.config.policy.filesystem.max_file_size_bytes = 4;
        loaded.resources = vec![NormalizedResource::Static(StaticResource {
            id: "oversized-inline".to_string(),
            uri: "elegy://fs-static-minimal/resource/oversized-inline".to_string(),
            title: None,
            description: None,
            mime_type: None,
            content: "hello world".to_string(),
            descriptor_path: "tests/oversized-static.toml".to_string(),
        })];
        let policy = policy_from_loaded(&loaded);

        let error = match compose_runtime_state(&loaded, &policy) {
            Ok(_) => panic!("oversized static content should surface diagnostics"),
            Err(error) => error,
        };

        assert_eq!(
            error.diagnostics(),
            [Diagnostic::error(
                "RUNTIME-POLICY-003",
                "file size 11 exceeds configured limit 4",
            )
            .with_path("tests/oversized-static.toml")
            .with_field("resources[].content")]
        );
    }

    #[test]
    fn http_openapi_example_keeps_openapi_scaffold_only() {
        let example = repo_root().join("examples/http-openapi-minimal");
        let loaded = load_project_from_root_config(&example.join("elegy.toml"))
            .expect("example config should validate");
        let policy = policy_from_loaded(&loaded);

        let error = compose_catalog(&loaded, &policy)
            .expect_err("runtime should still reject open_api resources");

        assert_eq!(
            error.diagnostics(),
            [Diagnostic::error(
                "RUNTIME-UNSUPPORTED-FAMILY-002",
                "resource family open_api is scaffold-only in the current runtime slice; descriptor validation accepted resource id \"pet-list\", but runtime composition does not execute that family yet",
            )
            .with_path("elegy.resources.d/http-openapi.toml")
            .with_field("resources[].kind")
            .with_hint(
                "plain constrained HTTP resources are supported; OpenAPI-derived runtime execution remains deferred",
            )]
        );
    }

    #[test]
    fn http_openapi_example_snapshot_matches_openapi_scaffold_contract() {
        let snapshot = std::fs::read_to_string(
            repo_root().join("examples/http-openapi-minimal/expected-resources.json"),
        )
        .expect("read scaffold-only example snapshot")
        .replace("\r\n", "\n");

        assert_eq!(
            snapshot,
            r#"{
  "status": "scaffold_only",
  "project_name": "http-openapi-minimal",
  "message": "Config validation still succeeds for the mixed HTTP/OpenAPI example, but runtime validation remains intentionally invalid until open_api composition is implemented.",
  "supported_runtime_families": [
    "static",
    "filesystem",
    "http"
  ],
  "scaffold_only_family": "open_api",
  "expected_outputs": {
    "validate_config": "expected-validate-config.json",
    "validate_runtime": "expected-validate-runtime.json"
  },
  "expected_runtime_diagnostics": [
    {
      "code": "RUNTIME-UNSUPPORTED-FAMILY-002",
      "family": "open_api",
      "resource_id": "pet-list"
    }
  ]
}
"#
        );
    }

    #[test]
    fn runtime_dispatch_reads_http_resource_and_maps_adapter_output() {
        let state = http_example_runtime_state();
        let uri = "elegy://http-minimal/resource/status";
        let client = StaticStubHttpClient {
            response: Some((
                200,
                Some("application/json".to_string()),
                None,
                br#"{"ok":true}"#.to_vec(),
                Some(11),
            )),
            error: None,
        };

        let result = state
            .read_resource_with_http_client(uri, &client)
            .expect("HTTP resource should read successfully");

        assert_eq!(
            result,
            ResourceReadResult {
                uri: uri.to_string(),
                mime_type: "application/json".to_string(),
                bytes: br#"{"ok":true}"#.to_vec(),
                http_response: Some(HttpReadMetadata {
                    target_url: "https://api.example.com/status".to_string(),
                    status_code: 200,
                    content_type: Some("application/json".to_string()),
                }),
            }
        );
    }

    #[test]
    fn runtime_dispatch_preserves_http_policy_denial_before_request() {
        let mut state = http_example_runtime_state();
        state.http_policy.allowed_targets = vec!["https://api.other.test".to_string()];

        let error = state
            .read_resource_with_http_client(
                "elegy://http-minimal/resource/status",
                &PanicHttpClient,
            )
            .expect_err("out-of-policy target must fail closed");

        assert_eq!(
            error,
            ReadResourceError::Http(HttpReadError::PolicyDenied {
                uri: "elegy://http-minimal/resource/status".to_string(),
                target: "https://api.example.com/status".to_string(),
                message: "HTTP target https://api.example.com/status is not allowed".to_string(),
            })
        );
    }

    type StubResponse = (u16, Option<String>, Option<String>, Vec<u8>, Option<u64>);

    struct StaticStubHttpClient {
        response: Option<StubResponse>,
        error: Option<HttpClientError>,
    }

    impl HttpClient for StaticStubHttpClient {
        fn get(&self, _request: &HttpRequest) -> Result<HttpResponse, HttpClientError> {
            if let Some(error) = &self.error {
                return Err(error.clone());
            }

            let (status_code, content_type, location, body, content_length) = self
                .response
                .clone()
                .expect("response or error must be configured");

            Ok(HttpResponse {
                status_code,
                content_type,
                content_length,
                location,
                body: Box::new(std::io::Cursor::new(body)),
            })
        }
    }

    struct PanicHttpClient;

    impl HttpClient for PanicHttpClient {
        fn get(&self, _request: &HttpRequest) -> Result<HttpResponse, HttpClientError> {
            panic!("HTTP client should not be invoked");
        }
    }
}
