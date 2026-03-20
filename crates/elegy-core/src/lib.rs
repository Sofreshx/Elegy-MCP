pub use elegy_contracts::*;
use elegy_descriptor::{
    load_project_from_root_config, LoadedProject, RawPolicyConfig, ValidationError,
};
use elegy_policy::{FilesystemPolicy, HttpPolicy, PolicyConfig};
use elegy_runtime::{
    compose_catalog, compose_runtime_state as compose_runtime_state_impl, CompositionError,
};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::path::{Path, PathBuf};

pub use elegy_descriptor::{
    Diagnostic, DiagnosticLocation, NormalizedDescriptor, NormalizedResource, ResourceFamily,
    Severity,
};
pub use elegy_runtime::{
    Catalog, CatalogPolicySummary, CatalogResource, CatalogSource, ReadResourceError,
    ResourceLimits, ResourceReadResult, RuntimeState, MCP_SPEC_BASELINE,
};

pub const CLI_SCHEMA_VERSION: &str = "elegy.cli/v1";

#[derive(Clone, Debug)]
pub enum ProjectLocator {
    Auto,
    Path(PathBuf),
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ConfigInspection {
    pub spec_baseline: String,
    pub project_name: String,
    pub root_config: String,
    pub descriptor_files: Vec<String>,
    pub resource_count: usize,
    pub policy: PolicyConfig,
}

#[derive(Clone, Debug)]
pub struct CoreError {
    diagnostics: Vec<Diagnostic>,
}

impl CoreError {
    pub fn new(mut diagnostics: Vec<Diagnostic>) -> Self {
        diagnostics.sort();
        diagnostics.dedup();
        Self { diagnostics }
    }

    pub fn diagnostics(&self) -> &[Diagnostic] {
        &self.diagnostics
    }
}

impl fmt::Display for CoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "core operation failed with {} diagnostic(s)",
            self.diagnostics.len()
        )
    }
}

impl std::error::Error for CoreError {}

pub fn load_descriptor_set(locator: ProjectLocator) -> Result<LoadedProject, CoreError> {
    let config_path = resolve_project(locator)?;
    load_project_from_root_config(&config_path).map_err(core_error_from_validation)
}

pub fn validate_descriptor_set(locator: ProjectLocator) -> Result<ConfigInspection, CoreError> {
    let loaded = load_descriptor_set(locator)?;
    Ok(ConfigInspection {
        spec_baseline: MCP_SPEC_BASELINE.to_string(),
        project_name: loaded.config.project_name.clone(),
        root_config: project_relative_display(&loaded.project_root, &loaded.root_config_path),
        descriptor_files: loaded
            .descriptors
            .iter()
            .map(|descriptor| descriptor.source_path.clone())
            .collect(),
        resource_count: loaded.resources.len(),
        policy: policy_from_raw(&loaded.config.policy),
    })
}

pub fn compose_runtime(locator: ProjectLocator) -> Result<Catalog, CoreError> {
    let loaded = load_descriptor_set(locator)?;
    let policy = policy_from_raw(&loaded.config.policy);
    compose_catalog(&loaded, &policy).map_err(core_error_from_composition)
}

pub fn compose_runtime_state(locator: ProjectLocator) -> Result<RuntimeState, CoreError> {
    let loaded = load_descriptor_set(locator)?;
    let policy = policy_from_raw(&loaded.config.policy);
    compose_runtime_state_impl(&loaded, &policy).map_err(core_error_from_composition)
}

fn policy_from_raw(raw: &RawPolicyConfig) -> PolicyConfig {
    PolicyConfig {
        filesystem: FilesystemPolicy {
            roots: raw.filesystem.roots.clone(),
            max_file_size_bytes: raw.filesystem.max_file_size_bytes,
            allow_symlinks: raw.filesystem.allow_symlinks,
        },
        http: HttpPolicy {
            allowed_targets: raw.http.allowed_targets.clone(),
            allow_plaintext_http: raw.http.allow_plaintext_http,
            timeout_ms: raw.http.timeout_ms,
            max_response_size_bytes: raw.http.max_response_size_bytes,
        },
    }
}

fn resolve_project(locator: ProjectLocator) -> Result<PathBuf, CoreError> {
    match locator {
        ProjectLocator::Auto => {
            let current_dir = std::env::current_dir().map_err(|error| {
                CoreError::new(vec![Diagnostic::error(
                    "CORE-DISCOVERY-001",
                    format!("failed to determine the current working directory: {error}"),
                )])
            })?;
            discover_root_config(&current_dir).ok_or_else(|| {
                CoreError::new(vec![Diagnostic::error(
                    "CORE-DISCOVERY-002",
                    format!(
                        "could not discover elegy.toml starting from {}",
                        current_dir.display()
                    ),
                )
                .with_hint(
                    "pass --project <PATH> or run the command inside an Elegy project",
                )])
            })
        }
        ProjectLocator::Path(path) => resolve_explicit_project(path),
    }
}

fn discover_root_config(start: &Path) -> Option<PathBuf> {
    for candidate in start.ancestors() {
        let root_config = candidate.join("elegy.toml");
        if root_config.is_file() {
            return Some(root_config);
        }
    }
    None
}

fn resolve_explicit_project(path: PathBuf) -> Result<PathBuf, CoreError> {
    if path.is_file() {
        if path.file_name().and_then(|value| value.to_str()) != Some("elegy.toml") {
            return Err(CoreError::new(vec![Diagnostic::error(
                "CORE-DISCOVERY-003",
                "explicit project file must be named elegy.toml",
            )
            .with_path(path.display().to_string())]));
        }
        return Ok(path);
    }

    if path.is_dir() {
        let root_config = path.join("elegy.toml");
        if root_config.is_file() {
            return Ok(root_config);
        }
        return Err(CoreError::new(vec![Diagnostic::error(
            "CORE-DISCOVERY-004",
            format!("directory {} does not contain elegy.toml", path.display()),
        )
        .with_path(path.display().to_string())]));
    }

    Err(CoreError::new(vec![Diagnostic::error(
        "CORE-DISCOVERY-005",
        format!("project path {} does not exist", path.display()),
    )
    .with_path(path.display().to_string())]))
}

fn core_error_from_validation(error: ValidationError) -> CoreError {
    CoreError::new(error.diagnostics().to_vec())
}

fn core_error_from_composition(error: CompositionError) -> CoreError {
    CoreError::new(error.diagnostics().to_vec())
}

fn project_relative_display(project_root: &Path, path: &Path) -> String {
    path.strip_prefix(project_root)
        .map(|relative| {
            relative
                .components()
                .filter_map(|component| match component {
                    std::path::Component::Normal(value) => {
                        Some(value.to_string_lossy().to_string())
                    }
                    _ => None,
                })
                .collect::<Vec<_>>()
                .join("/")
        })
        .unwrap_or_else(|_| path.display().to_string())
}
