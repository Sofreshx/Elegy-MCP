use glob::glob;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::fmt;
use std::fs;
use std::path::{Component, Path, PathBuf};
use url::Url;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    Error,
    Warning,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub struct DiagnosticLocation {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub field: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub struct Diagnostic {
    pub code: String,
    pub severity: Severity,
    pub message: String,
    #[serde(default, skip_serializing_if = "location_is_empty")]
    pub location: DiagnosticLocation,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hint: Option<String>,
}

const fn location_is_empty(location: &DiagnosticLocation) -> bool {
    location.path.is_none() && location.field.is_none()
}

impl Diagnostic {
    pub fn error(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            severity: Severity::Error,
            message: message.into(),
            location: DiagnosticLocation::default(),
            hint: None,
        }
    }

    pub fn with_path(mut self, path: impl Into<String>) -> Self {
        self.location.path = Some(path.into());
        self
    }

    pub fn with_field(mut self, field: impl Into<String>) -> Self {
        self.location.field = Some(field.into());
        self
    }

    pub fn with_hint(mut self, hint: impl Into<String>) -> Self {
        self.hint = Some(hint.into());
        self
    }
}

#[derive(Clone, Debug)]
pub struct ValidationError {
    diagnostics: Vec<Diagnostic>,
}

impl ValidationError {
    pub fn new(mut diagnostics: Vec<Diagnostic>) -> Self {
        diagnostics.sort();
        diagnostics.dedup();
        Self { diagnostics }
    }

    pub fn diagnostics(&self) -> &[Diagnostic] {
        &self.diagnostics
    }
}

impl fmt::Display for ValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "validation failed with {} diagnostic(s)",
            self.diagnostics.len()
        )
    }
}

impl std::error::Error for ValidationError {}

#[derive(Clone, Debug)]
pub struct LoadedProject {
    pub project_root: PathBuf,
    pub root_config_path: PathBuf,
    pub config: NormalizedConfig,
    pub descriptors: Vec<NormalizedDescriptor>,
    pub resources: Vec<NormalizedResource>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct NormalizedConfig {
    pub version: String,
    pub project_name: String,
    pub descriptor_patterns: Vec<String>,
    pub policy: RawPolicyConfig,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct RawPolicyConfig {
    pub filesystem: RawFilesystemPolicy,
    pub http: RawHttpPolicy,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct RawFilesystemPolicy {
    pub roots: Vec<String>,
    pub max_file_size_bytes: u64,
    pub allow_symlinks: bool,
}

impl Default for RawFilesystemPolicy {
    fn default() -> Self {
        Self {
            roots: Vec::new(),
            max_file_size_bytes: 1_048_576,
            allow_symlinks: false,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct RawHttpPolicy {
    pub allowed_targets: Vec<String>,
    pub allow_plaintext_http: bool,
    pub timeout_ms: u64,
    pub max_response_size_bytes: u64,
}

impl Default for RawHttpPolicy {
    fn default() -> Self {
        Self {
            allowed_targets: Vec::new(),
            allow_plaintext_http: false,
            timeout_ms: 30_000,
            max_response_size_bytes: 1_048_576,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct NormalizedDescriptor {
    pub name: String,
    pub source_path: String,
    pub resources: Vec<NormalizedResource>,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum ResourceFamily {
    Static,
    Filesystem,
    Http,
    OpenApi,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "family", rename_all = "snake_case")]
pub enum NormalizedResource {
    Static(StaticResource),
    Filesystem(FilesystemResource),
    Http(HttpResource),
    OpenApi(OpenApiResource),
}

impl NormalizedResource {
    pub fn family(&self) -> ResourceFamily {
        match self {
            Self::Static(_) => ResourceFamily::Static,
            Self::Filesystem(_) => ResourceFamily::Filesystem,
            Self::Http(_) => ResourceFamily::Http,
            Self::OpenApi(_) => ResourceFamily::OpenApi,
        }
    }

    pub fn id(&self) -> &str {
        match self {
            Self::Static(resource) => &resource.id,
            Self::Filesystem(resource) => &resource.id,
            Self::Http(resource) => &resource.id,
            Self::OpenApi(resource) => &resource.id,
        }
    }

    pub fn uri(&self) -> &str {
        match self {
            Self::Static(resource) => &resource.uri,
            Self::Filesystem(resource) => &resource.uri,
            Self::Http(resource) => &resource.uri,
            Self::OpenApi(resource) => &resource.uri,
        }
    }

    pub fn descriptor_path(&self) -> &str {
        match self {
            Self::Static(resource) => &resource.descriptor_path,
            Self::Filesystem(resource) => &resource.descriptor_path,
            Self::Http(resource) => &resource.descriptor_path,
            Self::OpenApi(resource) => &resource.descriptor_path,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct StaticResource {
    pub id: String,
    pub uri: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mime_type: Option<String>,
    pub content: String,
    pub descriptor_path: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct FilesystemResource {
    pub id: String,
    pub uri: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mime_type: Option<String>,
    pub root: String,
    pub path: String,
    pub descriptor_path: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct HttpResource {
    pub id: String,
    pub uri: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub base_url: String,
    pub path: String,
    pub descriptor_path: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct OpenApiResource {
    pub id: String,
    pub uri: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub document: String,
    pub operation_id: String,
    pub descriptor_path: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(untagged)]
enum VersionValue {
    Integer(u64),
    String(String),
}

impl VersionValue {
    fn normalized(&self) -> String {
        match self {
            Self::Integer(value) => value.to_string(),
            Self::String(value) => value.clone(),
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RootConfigDocument {
    version: VersionValue,
    project: ProjectMetadataDocument,
    #[serde(default)]
    descriptors: DescriptorDiscoveryDocument,
    #[serde(default)]
    policy: PolicyConfigDocument,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProjectMetadataDocument {
    name: String,
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct DescriptorDiscoveryDocument {
    #[serde(default = "default_descriptor_patterns")]
    include: Vec<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct PolicyConfigDocument {
    #[serde(default)]
    filesystem: FilesystemPolicyDocument,
    #[serde(default)]
    http: HttpPolicyDocument,
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct FilesystemPolicyDocument {
    #[serde(default)]
    roots: Vec<String>,
    max_file_size_bytes: Option<u64>,
    allow_symlinks: Option<bool>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct HttpPolicyDocument {
    #[serde(default)]
    allowed_targets: Vec<String>,
    allow_plaintext_http: Option<bool>,
    timeout_ms: Option<u64>,
    max_response_size_bytes: Option<u64>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct DescriptorDocument {
    version: VersionValue,
    name: String,
    resources: Vec<ResourceDocument>,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum ResourceDocument {
    Static(StaticResourceDocument),
    Filesystem(FilesystemResourceDocument),
    Http(HttpResourceDocument),
    OpenApi(OpenApiResourceDocument),
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct StaticResourceDocument {
    id: String,
    uri: String,
    title: Option<String>,
    description: Option<String>,
    mime_type: Option<String>,
    content: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct FilesystemResourceDocument {
    id: String,
    uri: String,
    title: Option<String>,
    description: Option<String>,
    mime_type: Option<String>,
    root: String,
    path: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct HttpResourceDocument {
    id: String,
    uri: String,
    title: Option<String>,
    description: Option<String>,
    base_url: String,
    path: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct OpenApiResourceDocument {
    id: String,
    uri: String,
    title: Option<String>,
    description: Option<String>,
    document: String,
    operation_id: String,
}

pub fn load_project_from_root_config(
    root_config_path: &Path,
) -> Result<LoadedProject, ValidationError> {
    let root_config_path = root_config_path.canonicalize().map_err(|error| {
        ValidationError::new(vec![Diagnostic::error(
            "DESC-DISCOVERY-001",
            format!(
                "failed to resolve root config at {}: {error}",
                root_config_path.display()
            ),
        )
        .with_path(root_config_path.display().to_string())])
    })?;

    if root_config_path
        .file_name()
        .and_then(|value| value.to_str())
        != Some("elegy.toml")
    {
        return Err(ValidationError::new(vec![Diagnostic::error(
            "DESC-DISCOVERY-002",
            "root config file must be named elegy.toml",
        )
        .with_path(root_config_path.display().to_string())]));
    }

    let project_root = root_config_path
        .parent()
        .map(Path::to_path_buf)
        .ok_or_else(|| {
            ValidationError::new(vec![Diagnostic::error(
                "DESC-DISCOVERY-003",
                "root config must have a parent directory",
            )
            .with_path(root_config_path.display().to_string())])
        })?;

    let root_config_relative = project_relative_display(&project_root, &root_config_path);
    let raw_root =
        read_toml::<RootConfigDocument>(&root_config_path, &root_config_relative, "root config")?;
    validate_version(&raw_root.version, &root_config_relative, "version")?;

    let descriptor_patterns = if raw_root.descriptors.include.is_empty() {
        default_descriptor_patterns()
    } else {
        raw_root.descriptors.include
    };

    let project_name = normalize_non_empty(
        &raw_root.project.name,
        "project.name",
        &root_config_relative,
        "DESC-CONFIG-001",
    )?;

    let policy = RawPolicyConfig {
        filesystem: RawFilesystemPolicy {
            roots: normalize_unique_relative_paths(
                raw_root.policy.filesystem.roots,
                &root_config_relative,
                "policy.filesystem.roots",
            )?,
            max_file_size_bytes: raw_root
                .policy
                .filesystem
                .max_file_size_bytes
                .unwrap_or(1_048_576),
            allow_symlinks: raw_root.policy.filesystem.allow_symlinks.unwrap_or(false),
        },
        http: RawHttpPolicy {
            allowed_targets: normalize_http_targets(
                raw_root.policy.http.allowed_targets,
                &root_config_relative,
                "policy.http.allowed_targets",
            )?,
            allow_plaintext_http: raw_root.policy.http.allow_plaintext_http.unwrap_or(false),
            timeout_ms: raw_root.policy.http.timeout_ms.unwrap_or(30_000),
            max_response_size_bytes: raw_root
                .policy
                .http
                .max_response_size_bytes
                .unwrap_or(1_048_576),
        },
    };

    let descriptor_paths =
        expand_descriptor_patterns(&project_root, &descriptor_patterns, &root_config_relative)?;
    if descriptor_paths.is_empty() {
        return Err(ValidationError::new(vec![Diagnostic::error(
            "DESC-CONFIG-002",
            "descriptor include patterns did not match any files",
        )
        .with_path(root_config_relative)
        .with_field("descriptors.include")]));
    }

    let mut descriptors = Vec::new();
    let mut resources = Vec::new();

    for descriptor_path in descriptor_paths {
        let descriptor_relative = project_relative_display(&project_root, &descriptor_path);
        let raw_descriptor =
            read_toml::<DescriptorDocument>(&descriptor_path, &descriptor_relative, "descriptor")?;
        validate_version(&raw_descriptor.version, &descriptor_relative, "version")?;
        let descriptor_name = normalize_identifier(
            &raw_descriptor.name,
            &descriptor_relative,
            "name",
            "DESC-DESCRIPTOR-001",
        )?;
        let mut normalized_resources = Vec::new();
        for resource in raw_descriptor.resources {
            normalized_resources.push(normalize_resource(resource, &descriptor_relative)?);
        }
        normalized_resources.sort_by(|left, right| {
            left.uri()
                .cmp(right.uri())
                .then_with(|| left.id().cmp(right.id()))
                .then_with(|| left.family().cmp(&right.family()))
        });
        resources.extend(normalized_resources.iter().cloned());
        descriptors.push(NormalizedDescriptor {
            name: descriptor_name,
            source_path: descriptor_relative,
            resources: normalized_resources,
        });
    }

    resources.sort_by(|left, right| {
        left.uri()
            .cmp(right.uri())
            .then_with(|| left.id().cmp(right.id()))
            .then_with(|| left.family().cmp(&right.family()))
    });

    Ok(LoadedProject {
        project_root,
        root_config_path,
        config: NormalizedConfig {
            version: "1".to_string(),
            project_name,
            descriptor_patterns,
            policy,
        },
        descriptors,
        resources,
    })
}

fn read_toml<T: for<'de> Deserialize<'de>>(
    path: &Path,
    display_path: &str,
    kind: &str,
) -> Result<T, ValidationError> {
    let content = fs::read_to_string(path).map_err(|error| {
        ValidationError::new(vec![Diagnostic::error(
            "DESC-IO-001",
            format!("failed to read {kind} file {display_path}: {error}"),
        )
        .with_path(display_path.to_string())])
    })?;

    toml::from_str(&content).map_err(|error| {
        ValidationError::new(vec![Diagnostic::error(
            "DESC-PARSE-001",
            format!("failed to parse {kind} file {display_path}: {error}"),
        )
        .with_path(display_path.to_string())])
    })
}

fn validate_version(
    version: &VersionValue,
    path: &str,
    field: &str,
) -> Result<(), ValidationError> {
    let normalized = version.normalized();
    if normalized == "1" {
        Ok(())
    } else {
        Err(ValidationError::new(vec![Diagnostic::error(
            "DESC-VERSION-001",
            format!("unsupported version {normalized}; expected version 1"),
        )
        .with_path(path.to_string())
        .with_field(field.to_string())]))
    }
}

fn default_descriptor_patterns() -> Vec<String> {
    vec!["elegy.resources.d/*.toml".to_string()]
}

fn expand_descriptor_patterns(
    project_root: &Path,
    patterns: &[String],
    config_path: &str,
) -> Result<Vec<PathBuf>, ValidationError> {
    let mut matches = BTreeSet::new();
    for pattern in patterns {
        let absolute_pattern = project_root.join(pattern);
        let pattern_text = absolute_pattern.to_string_lossy().replace('\\', "/");
        let entries = glob(&pattern_text).map_err(|error| {
            ValidationError::new(vec![Diagnostic::error(
                "DESC-CONFIG-003",
                format!("invalid descriptor include pattern {pattern:?}: {error}"),
            )
            .with_path(config_path.to_string())
            .with_field("descriptors.include")])
        })?;
        for entry in entries {
            match entry {
                Ok(path) => {
                    matches.insert(path.canonicalize().map_err(|error| {
                        ValidationError::new(vec![Diagnostic::error(
                            "DESC-IO-002",
                            format!(
                                "failed to resolve descriptor file {}: {error}",
                                path.display()
                            ),
                        )
                        .with_path(path.display().to_string())])
                    })?);
                }
                Err(error) => {
                    return Err(ValidationError::new(vec![Diagnostic::error(
                        "DESC-CONFIG-004",
                        format!("failed to expand descriptor include pattern {pattern:?}: {error}"),
                    )
                    .with_path(config_path.to_string())
                    .with_field("descriptors.include")]));
                }
            }
        }
    }
    Ok(matches.into_iter().collect())
}

fn normalize_resource(
    resource: ResourceDocument,
    descriptor_path: &str,
) -> Result<NormalizedResource, ValidationError> {
    match resource {
        ResourceDocument::Static(resource) => Ok(NormalizedResource::Static(StaticResource {
            id: normalize_identifier(
                &resource.id,
                descriptor_path,
                "resources[].id",
                "DESC-RESOURCE-001",
            )?,
            uri: normalize_uri(&resource.uri, descriptor_path, "resources[].uri")?,
            title: normalize_optional_text(resource.title),
            description: normalize_optional_text(resource.description),
            mime_type: normalize_optional_text(resource.mime_type),
            content: resource.content,
            descriptor_path: descriptor_path.to_string(),
        })),
        ResourceDocument::Filesystem(resource) => {
            Ok(NormalizedResource::Filesystem(FilesystemResource {
                id: normalize_identifier(
                    &resource.id,
                    descriptor_path,
                    "resources[].id",
                    "DESC-RESOURCE-001",
                )?,
                uri: normalize_uri(&resource.uri, descriptor_path, "resources[].uri")?,
                title: normalize_optional_text(resource.title),
                description: normalize_optional_text(resource.description),
                mime_type: normalize_optional_text(resource.mime_type),
                root: normalize_relative_path(
                    &resource.root,
                    descriptor_path,
                    "resources[].root",
                    "DESC-RESOURCE-002",
                )?,
                path: normalize_relative_path(
                    &resource.path,
                    descriptor_path,
                    "resources[].path",
                    "DESC-RESOURCE-003",
                )?,
                descriptor_path: descriptor_path.to_string(),
            }))
        }
        ResourceDocument::Http(resource) => {
            normalize_http_target_value(
                &resource.base_url,
                descriptor_path,
                "resources[].base_url",
            )?;
            Ok(NormalizedResource::Http(HttpResource {
                id: normalize_identifier(
                    &resource.id,
                    descriptor_path,
                    "resources[].id",
                    "DESC-RESOURCE-001",
                )?,
                uri: normalize_uri(&resource.uri, descriptor_path, "resources[].uri")?,
                title: normalize_optional_text(resource.title),
                description: normalize_optional_text(resource.description),
                base_url: resource.base_url.trim().to_string(),
                path: normalize_http_path(&resource.path, descriptor_path, "resources[].path")?,
                descriptor_path: descriptor_path.to_string(),
            }))
        }
        ResourceDocument::OpenApi(resource) => Ok(NormalizedResource::OpenApi(OpenApiResource {
            id: normalize_identifier(
                &resource.id,
                descriptor_path,
                "resources[].id",
                "DESC-RESOURCE-001",
            )?,
            uri: normalize_uri(&resource.uri, descriptor_path, "resources[].uri")?,
            title: normalize_optional_text(resource.title),
            description: normalize_optional_text(resource.description),
            document: normalize_relative_path(
                &resource.document,
                descriptor_path,
                "resources[].document",
                "DESC-RESOURCE-004",
            )?,
            operation_id: normalize_identifier(
                &resource.operation_id,
                descriptor_path,
                "resources[].operation_id",
                "DESC-RESOURCE-005",
            )?,
            descriptor_path: descriptor_path.to_string(),
        })),
    }
}

fn normalize_non_empty(
    value: &str,
    field: &str,
    path: &str,
    code: &str,
) -> Result<String, ValidationError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        Err(ValidationError::new(vec![Diagnostic::error(
            code,
            "value must not be empty",
        )
        .with_path(path.to_string())
        .with_field(field.to_string())]))
    } else {
        Ok(trimmed.to_string())
    }
}

fn normalize_identifier(
    value: &str,
    path: &str,
    field: &str,
    code: &str,
) -> Result<String, ValidationError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(ValidationError::new(vec![Diagnostic::error(
            code,
            "identifier must not be empty",
        )
        .with_path(path.to_string())
        .with_field(field.to_string())]));
    }

    let mut normalized = String::new();
    let mut last_dash = false;
    for character in trimmed.chars() {
        if character.is_ascii_alphanumeric() {
            normalized.push(character.to_ascii_lowercase());
            last_dash = false;
        } else if matches!(character, '-' | '_' | ' ') {
            if !last_dash && !normalized.is_empty() {
                normalized.push('-');
                last_dash = true;
            }
        } else {
            return Err(ValidationError::new(vec![Diagnostic::error(
                code,
                format!("identifier contains unsupported character {character:?}"),
            )
            .with_path(path.to_string())
            .with_field(field.to_string())]));
        }
    }

    while normalized.ends_with('-') {
        normalized.pop();
    }

    if normalized.is_empty() {
        return Err(ValidationError::new(vec![Diagnostic::error(
            code,
            "identifier normalized to an empty value",
        )
        .with_path(path.to_string())
        .with_field(field.to_string())]));
    }

    Ok(normalized)
}

fn normalize_uri(value: &str, path: &str, field: &str) -> Result<String, ValidationError> {
    let trimmed = value.trim();
    let normalized = Url::parse(trimmed).map_err(|error| {
        ValidationError::new(vec![Diagnostic::error(
            "DESC-RESOURCE-006",
            format!("resource URI is invalid: {error}"),
        )
        .with_path(path.to_string())
        .with_field(field.to_string())])
    })?;
    Ok(normalized.to_string())
}

fn normalize_relative_path(
    value: &str,
    path: &str,
    field: &str,
    code: &str,
) -> Result<String, ValidationError> {
    let candidate = value.trim();
    if candidate.is_empty() {
        return Err(ValidationError::new(vec![Diagnostic::error(
            code,
            "path must not be empty",
        )
        .with_path(path.to_string())
        .with_field(field.to_string())]));
    }

    let mut components = Vec::new();
    for component in Path::new(candidate).components() {
        match component {
            Component::Normal(value) => components.push(value.to_string_lossy().to_string()),
            Component::CurDir => continue,
            Component::ParentDir | Component::Prefix(_) | Component::RootDir => {
                return Err(ValidationError::new(vec![Diagnostic::error(
                    code,
                    "path must stay relative and must not escape its root",
                )
                .with_path(path.to_string())
                .with_field(field.to_string())]));
            }
        }
    }

    if components.is_empty() {
        return Err(ValidationError::new(vec![Diagnostic::error(
            code,
            "path normalized to an empty value",
        )
        .with_path(path.to_string())
        .with_field(field.to_string())]));
    }

    Ok(components.join("/"))
}

fn normalize_http_targets(
    values: Vec<String>,
    path: &str,
    field: &str,
) -> Result<Vec<String>, ValidationError> {
    let mut targets = BTreeSet::new();
    for value in values {
        normalize_http_target_value(&value, path, field)?;
        targets.insert(value.trim().trim_end_matches('/').to_string());
    }
    Ok(targets.into_iter().collect())
}

fn normalize_http_target_value(
    value: &str,
    path: &str,
    field: &str,
) -> Result<(), ValidationError> {
    let url = Url::parse(value.trim()).map_err(|error| {
        ValidationError::new(vec![Diagnostic::error(
            "DESC-POLICY-001",
            format!("HTTP target is invalid: {error}"),
        )
        .with_path(path.to_string())
        .with_field(field.to_string())])
    })?;
    if !matches!(url.scheme(), "http" | "https") {
        return Err(ValidationError::new(vec![Diagnostic::error(
            "DESC-POLICY-002",
            "HTTP targets must use http or https",
        )
        .with_path(path.to_string())
        .with_field(field.to_string())]));
    }
    if !url.username().is_empty() || url.password().is_some() {
        return Err(ValidationError::new(vec![Diagnostic::error(
            "DESC-POLICY-004",
            "HTTP targets must not contain embedded credentials",
        )
        .with_path(path.to_string())
        .with_field(field.to_string())]));
    }
    Ok(())
}

fn normalize_http_path(value: &str, path: &str, field: &str) -> Result<String, ValidationError> {
    let trimmed = value.trim();
    if trimmed.is_empty() || !trimmed.starts_with('/') {
        return Err(ValidationError::new(vec![Diagnostic::error(
            "DESC-RESOURCE-007",
            "HTTP path must start with '/' and must not be empty",
        )
        .with_path(path.to_string())
        .with_field(field.to_string())]));
    }

    if trimmed.starts_with("//") || Url::parse(trimmed).is_ok() {
        return Err(ValidationError::new(vec![Diagnostic::error(
            "DESC-RESOURCE-008",
            "HTTP path must be a literal absolute path only and must not be a full URL",
        )
        .with_path(path.to_string())
        .with_field(field.to_string())]));
    }

    if trimmed.contains('?') {
        return Err(ValidationError::new(vec![Diagnostic::error(
            "DESC-RESOURCE-009",
            "HTTP path must not include a query string in the constrained runtime slice",
        )
        .with_path(path.to_string())
        .with_field(field.to_string())]));
    }

    if trimmed.contains('#') {
        return Err(ValidationError::new(vec![Diagnostic::error(
            "DESC-RESOURCE-010",
            "HTTP path must not include a fragment in the constrained runtime slice",
        )
        .with_path(path.to_string())
        .with_field(field.to_string())]));
    }

    if trimmed.contains('{') || trimmed.contains('}') {
        return Err(ValidationError::new(vec![Diagnostic::error(
            "DESC-RESOURCE-011",
            "HTTP path must stay literal and must not include templates or parameters",
        )
        .with_path(path.to_string())
        .with_field(field.to_string())]));
    }

    Ok(trimmed.to_string())
}

fn normalize_optional_text(value: Option<String>) -> Option<String> {
    value.and_then(|value| {
        let trimmed = value.trim().to_string();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed)
        }
    })
}

fn normalize_unique_relative_paths(
    values: Vec<String>,
    path: &str,
    field: &str,
) -> Result<Vec<String>, ValidationError> {
    let mut normalized = BTreeSet::new();
    for value in values {
        normalized.insert(normalize_relative_path(
            &value,
            path,
            field,
            "DESC-POLICY-003",
        )?);
    }
    Ok(normalized.into_iter().collect())
}

fn project_relative_display(project_root: &Path, path: &Path) -> String {
    path.strip_prefix(project_root)
        .map(to_forward_slashes)
        .unwrap_or_else(|_| path.display().to_string())
}

fn to_forward_slashes(path: &Path) -> String {
    path.components()
        .filter_map(|component| match component {
            Component::Normal(value) => Some(value.to_string_lossy().to_string()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("/")
}

#[cfg(test)]
mod tests {
    use super::{normalize_http_path, normalize_http_target_value};

    #[test]
    fn http_paths_allow_literal_rooted_paths() {
        let normalized =
            normalize_http_path(" /status/health ", "descriptor.toml", "resources[].path")
                .expect("literal rooted path should normalize");

        assert_eq!(normalized, "/status/health");
    }

    #[test]
    fn http_paths_reject_absolute_urls_and_network_paths() {
        let absolute_url = normalize_http_path(
            "https://api.example.com/status",
            "descriptor.toml",
            "resources[].path",
        )
        .expect_err("absolute URL must be rejected");
        assert_eq!(absolute_url.diagnostics()[0].code, "DESC-RESOURCE-007");

        let network_path = normalize_http_path(
            "//api.example.com/status",
            "descriptor.toml",
            "resources[].path",
        )
        .expect_err("network-path reference must be rejected");
        assert_eq!(network_path.diagnostics()[0].code, "DESC-RESOURCE-008");
    }

    #[test]
    fn http_paths_reject_query_strings_fragments_and_templates() {
        for (invalid, expected_code) in [
            ("/status?verbose=true", "DESC-RESOURCE-009"),
            ("/status#top", "DESC-RESOURCE-010"),
            ("/users/{id}", "DESC-RESOURCE-011"),
        ] {
            let error = normalize_http_path(invalid, "descriptor.toml", "resources[].path")
                .expect_err("non-literal HTTP path must be rejected");

            assert_eq!(error.diagnostics()[0].code, expected_code);
        }
    }

    #[test]
    fn http_targets_reject_embedded_credentials() {
        for field in ["resources[].base_url", "policy.http.allowed_targets"] {
            let error = normalize_http_target_value(
                "https://user:pass@api.example.com/status",
                "descriptor.toml",
                field,
            )
            .expect_err("HTTP targets with embedded credentials must be rejected");

            assert_eq!(error.diagnostics()[0].code, "DESC-POLICY-004");
        }
    }
}
