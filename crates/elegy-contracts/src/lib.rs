use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ContractsError {
    #[error("failed to read {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to parse JSON in {path}: {source}")]
    Json {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },
    #[error("compatibility manifest is missing schema '{0}'")]
    MissingSchema(String),
    #[error("{0}")]
    Compatibility(String),
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CompatibilityManifest {
    pub manifest_version: String,
    pub package: ContractPackage,
    pub schemas: Vec<SchemaEntry>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ContractPackage {
    pub name: String,
    pub version: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SchemaEntry {
    pub name: String,
    pub schema_version: String,
    pub file: String,
    pub fixtures: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ConsumerSupportManifest {
    pub consumer: String,
    pub consumer_version: String,
    pub upstream_package: ContractPackage,
    pub schemas: BTreeMap<String, String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SkillDefinition {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    #[serde(default)]
    pub identity: SkillIdentity,
    #[serde(default)]
    pub metadata: SkillMetadata,
    #[serde(default)]
    pub triggers: Vec<SkillTrigger>,
    #[serde(default)]
    pub constraints: Vec<SkillConstraint>,
    #[serde(default)]
    pub input: SkillInputContract,
    #[serde(default)]
    pub output: SkillOutputContract,
    #[serde(default)]
    pub execution: SkillExecutionContract,
    #[serde(default)]
    pub governance: SkillGovernanceMetadata,
    #[serde(default)]
    pub discovery: SkillDiscoveryMetadata,
    #[serde(default)]
    pub origin: SkillOrigin,
    #[serde(default)]
    pub lifecycle_state: SkillLifecycleState,
}

impl SkillDefinition {
    pub fn effective_id(&self) -> &str {
        if self.identity.definition_id.trim().is_empty() {
            self.id.as_str()
        } else {
            self.identity.definition_id.as_str()
        }
    }

    pub fn effective_name(&self) -> &str {
        if self.identity.display_name.trim().is_empty() {
            self.name.as_str()
        } else {
            self.identity.display_name.as_str()
        }
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SkillIdentity {
    #[serde(default)]
    pub definition_id: String,
    #[serde(default)]
    pub display_name: String,
    pub namespace: Option<String>,
    pub version: Option<String>,
    #[serde(default)]
    pub aliases: Vec<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SkillMetadata {
    pub summary: Option<String>,
    pub category: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub owners: Vec<String>,
    pub documentation_uri: Option<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SkillTrigger {
    pub pattern: String,
    pub description: Option<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SkillConstraint {
    pub constraint_id: String,
    pub description: Option<String>,
    pub required: bool,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SkillInputContract {
    #[serde(default)]
    pub parameters: Vec<SkillParameter>,
    pub schema_ref: Option<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SkillParameter {
    pub name: String,
    pub r#type: String,
    pub description: Option<String>,
    pub required: bool,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SkillOutputContract {
    pub result_type: Option<String>,
    pub schema_ref: Option<String>,
    pub returns_collection: bool,
    pub description: Option<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SkillExecutionContract {
    #[serde(default)]
    pub mode: SkillExecutionMode,
    pub is_deterministic: bool,
    pub has_side_effects: bool,
    pub timeout_seconds: Option<i32>,
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum SkillExecutionMode {
    #[default]
    RequestResponse,
    LongRunning,
    Streaming,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SkillGovernanceMetadata {
    #[serde(default)]
    pub risk_level: SkillRiskLevel,
    #[serde(default)]
    pub approval_requirement: SkillApprovalRequirement,
    #[serde(default)]
    pub policy_refs: Vec<String>,
    #[serde(default)]
    pub allowed_contexts: Vec<String>,
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum SkillRiskLevel {
    #[default]
    Low,
    Medium,
    High,
    Critical,
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum SkillApprovalRequirement {
    #[default]
    None,
    Advisory,
    Required,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SkillDiscoveryMetadata {
    #[serde(default)]
    pub keywords: Vec<String>,
    #[serde(default)]
    pub capability_hints: Vec<String>,
    pub is_hidden: bool,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SkillOrigin {
    #[serde(default)]
    pub materialization_kind: SkillMaterializationKind,
    #[serde(default)]
    pub source_kind: SkillSourceKind,
    pub source_ref: Option<String>,
    pub source_version: Option<String>,
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum SkillMaterializationKind {
    #[default]
    Declared,
    Dynamic,
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum SkillSourceKind {
    #[default]
    Manual,
    Imported,
    Generated,
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum SkillLifecycleState {
    #[default]
    Draft,
    Active,
    Deprecated,
    Archived,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SkillDiscoveryIndex {
    pub schema_version: i32,
    pub built_at: Option<String>,
    #[serde(default)]
    pub entries: Vec<SkillDiscoveryEntry>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SkillDiscoveryEntry {
    pub skill_id: String,
    pub name: String,
    pub description: String,
    #[serde(default)]
    pub lifecycle_state: SkillLifecycleState,
    #[serde(default)]
    pub triggers_on: Vec<String>,
    #[serde(default)]
    pub keywords: Vec<String>,
    #[serde(default)]
    pub capability_hints: Vec<String>,
    #[serde(default)]
    pub manifest: SkillDiscoveryManifest,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SkillDiscoveryManifest {
    pub id: String,
    #[serde(default)]
    pub load_mode: SkillLoadMode,
    pub vault_ref: Option<String>,
    #[serde(default)]
    pub source_kind: SkillSourceKind,
    #[serde(default)]
    pub materialization_kind: SkillMaterializationKind,
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum SkillLoadMode {
    Always,
    #[default]
    OnDemand,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct McpServerDescriptor {
    pub server_name: String,
    #[serde(default)]
    pub transport: McpTransportKind,
    #[serde(default)]
    pub tools: Vec<McpToolDefinition>,
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum McpTransportKind {
    #[default]
    Stdio,
    Http,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct McpToolDefinition {
    pub name: String,
    pub description: Option<String>,
    pub input_schema: Option<Value>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct McpAnalysisResult {
    pub server_name: String,
    #[serde(default)]
    pub analyses: Vec<McpToolAnalysis>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct McpToolAnalysis {
    #[serde(default)]
    pub tool: McpToolDefinition,
    #[serde(default)]
    pub extracted_triggers: Vec<SkillTrigger>,
    pub has_valid_schema: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SkillValidationResult {
    pub issues: Vec<String>,
}

impl SkillValidationResult {
    pub fn is_valid(&self) -> bool {
        self.issues.is_empty()
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct McpValidationResult {
    pub issues: Vec<String>,
}

impl McpValidationResult {
    pub fn is_valid(&self) -> bool {
        self.issues.is_empty()
    }
}

pub fn default_support_manifest_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("contracts")
        .join("elegy-consumer-support.json")
}

pub fn resolve_upstream_contracts_dir() -> PathBuf {
    if let Some(path) = env::var_os("ELEGY_CONTRACTS_DIR") {
        return PathBuf::from(path);
    }

    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("..")
        .join("Elegy")
        .join("artifacts")
        .join("contracts")
}

pub fn load_compatibility_manifest_from_dir(
    dir: &Path,
) -> Result<CompatibilityManifest, ContractsError> {
    load_json_file(&dir.join("compatibility-manifest.json"))
}

pub fn load_consumer_support_manifest(
    path: &Path,
) -> Result<ConsumerSupportManifest, ContractsError> {
    load_json_file(path)
}

pub fn load_skill_definition_fixture_from_dir(
    dir: &Path,
) -> Result<SkillDefinition, ContractsError> {
    load_json_file(&dir.join("fixtures").join("skill-definition.minimal.json"))
}

pub fn load_skill_discovery_index_fixture_from_dir(
    dir: &Path,
) -> Result<SkillDiscoveryIndex, ContractsError> {
    load_json_file(
        &dir.join("fixtures")
            .join("skill-discovery-index.minimal.json"),
    )
}

pub fn load_mcp_server_descriptor_fixture_from_dir(
    dir: &Path,
) -> Result<McpServerDescriptor, ContractsError> {
    load_json_file(
        &dir.join("fixtures")
            .join("mcp-server-descriptor.minimal.json"),
    )
}

pub fn load_mcp_analysis_result_fixture_from_dir(
    dir: &Path,
) -> Result<McpAnalysisResult, ContractsError> {
    load_json_file(
        &dir.join("fixtures")
            .join("mcp-analysis-result.minimal.json"),
    )
}

pub fn validate_support_manifest_against_upstream(
    support: &ConsumerSupportManifest,
    upstream: &CompatibilityManifest,
) -> Result<(), ContractsError> {
    if support.upstream_package.name != upstream.package.name {
        return Err(ContractsError::Compatibility(format!(
            "support manifest expects upstream package '{}', but bundle package is '{}'",
            support.upstream_package.name, upstream.package.name
        )));
    }

    if support.upstream_package.version != upstream.package.version {
        return Err(ContractsError::Compatibility(format!(
            "support manifest expects upstream package version '{}', but bundle version is '{}'",
            support.upstream_package.version, upstream.package.version
        )));
    }

    for (schema_name, expected_version) in &support.schemas {
        let entry = upstream
            .schemas
            .iter()
            .find(|candidate| candidate.name == *schema_name)
            .ok_or_else(|| ContractsError::MissingSchema(schema_name.clone()))?;

        if entry.schema_version != *expected_version {
            return Err(ContractsError::Compatibility(format!(
                "support manifest expects schema '{}' at version '{}', but bundle provides '{}'",
                schema_name, expected_version, entry.schema_version
            )));
        }
    }

    Ok(())
}

pub fn validate_skill_definition(definition: &SkillDefinition) -> SkillValidationResult {
    let mut issues = Vec::new();

    if definition.effective_id().trim().is_empty() {
        issues.push("Skill definition ID is required.".to_string());
    }

    if definition.effective_name().trim().is_empty() {
        issues.push("Skill name is required.".to_string());
    }

    if definition
        .triggers
        .iter()
        .any(|trigger| trigger.pattern.trim().is_empty())
    {
        issues.push("Skill triggers must define a non-empty pattern.".to_string());
    }

    if definition
        .constraints
        .iter()
        .any(|constraint| constraint.constraint_id.trim().is_empty())
    {
        issues.push("Skill constraints must define a non-empty constraint ID.".to_string());
    }

    if definition
        .identity
        .aliases
        .iter()
        .any(|alias| alias.trim().is_empty())
    {
        issues.push("Skill identity aliases must not be blank.".to_string());
    }

    if has_duplicate_values(definition.identity.aliases.iter().map(String::as_str)) {
        issues.push("Skill identity aliases must be unique.".to_string());
    }

    if definition
        .metadata
        .tags
        .iter()
        .any(|tag| tag.trim().is_empty())
    {
        issues.push("Skill metadata tags must not be blank.".to_string());
    }

    if definition
        .metadata
        .owners
        .iter()
        .any(|owner| owner.trim().is_empty())
    {
        issues.push("Skill metadata owners must not be blank.".to_string());
    }

    if definition
        .input
        .parameters
        .iter()
        .any(|parameter| parameter.name.trim().is_empty())
    {
        issues.push("Skill input parameters must define a non-empty name.".to_string());
    }

    if has_duplicate_values(
        definition
            .input
            .parameters
            .iter()
            .map(|parameter| parameter.name.as_str()),
    ) {
        issues.push("Skill input parameter names must be unique.".to_string());
    }

    if definition
        .input
        .parameters
        .iter()
        .any(|parameter| parameter.r#type.trim().is_empty())
    {
        issues.push("Skill input parameters must define a non-empty type.".to_string());
    }

    if definition
        .execution
        .timeout_seconds
        .is_some_and(|timeout| timeout <= 0)
    {
        issues.push(
            "Skill execution timeout, when provided, must be greater than zero seconds."
                .to_string(),
        );
    }

    if definition.governance.approval_requirement != SkillApprovalRequirement::None
        && definition.governance.policy_refs.is_empty()
    {
        issues.push(
            "Skills that require approval must declare at least one policy reference.".to_string(),
        );
    }

    if definition
        .governance
        .policy_refs
        .iter()
        .any(|policy_ref| policy_ref.trim().is_empty())
    {
        issues.push("Skill governance policy references must not be blank.".to_string());
    }

    if definition
        .governance
        .allowed_contexts
        .iter()
        .any(|context| context.trim().is_empty())
    {
        issues.push("Skill governance allowed contexts must not be blank.".to_string());
    }

    if definition
        .discovery
        .keywords
        .iter()
        .any(|keyword| keyword.trim().is_empty())
    {
        issues.push("Skill discovery keywords must not be blank.".to_string());
    }

    if definition
        .discovery
        .capability_hints
        .iter()
        .any(|hint| hint.trim().is_empty())
    {
        issues.push("Skill discovery capability hints must not be blank.".to_string());
    }

    if definition.origin.materialization_kind == SkillMaterializationKind::Dynamic
        && definition.origin.source_kind == SkillSourceKind::Manual
        && definition
            .origin
            .source_ref
            .as_ref()
            .is_none_or(|source_ref| source_ref.trim().is_empty())
    {
        issues.push(
            "Dynamic skills must declare either a source reference or a non-manual source kind."
                .to_string(),
        );
    }

    SkillValidationResult { issues }
}

pub fn validate_mcp_server_descriptor(descriptor: &McpServerDescriptor) -> McpValidationResult {
    let mut issues = Vec::new();

    if descriptor.server_name.trim().is_empty() {
        issues.push("MCP server descriptor must declare a server name.".to_string());
    }

    if descriptor
        .tools
        .iter()
        .any(|tool| tool.name.trim().is_empty())
    {
        issues.push("MCP server descriptor tools must define a non-empty name.".to_string());
    }

    if has_duplicate_values(descriptor.tools.iter().map(|tool| tool.name.as_str())) {
        issues.push("MCP server descriptor tool names must be unique.".to_string());
    }

    McpValidationResult { issues }
}

pub fn validate_mcp_analysis_result(result: &McpAnalysisResult) -> McpValidationResult {
    let mut issues = Vec::new();

    if result.server_name.trim().is_empty() {
        issues.push("MCP analysis result must declare a server name.".to_string());
    }

    if result
        .analyses
        .iter()
        .any(|analysis| analysis.tool.name.trim().is_empty())
    {
        issues.push("MCP analysis entries must define a non-empty tool name.".to_string());
    }

    if has_duplicate_values(
        result
            .analyses
            .iter()
            .map(|analysis| analysis.tool.name.as_str()),
    ) {
        issues.push("MCP analysis entries must be unique per tool name.".to_string());
    }

    if result.analyses.iter().any(|analysis| {
        analysis
            .extracted_triggers
            .iter()
            .any(|trigger| trigger.pattern.trim().is_empty())
    }) {
        issues.push("MCP analysis extracted triggers must define a non-empty pattern.".to_string());
    }

    if result
        .analyses
        .iter()
        .any(|analysis| analysis.has_valid_schema && analysis.tool.input_schema.is_none())
    {
        issues.push(
            "MCP analysis entries marked as having a valid schema must include an input schema."
                .to_string(),
        );
    }

    McpValidationResult { issues }
}

fn load_json_file<T>(path: &Path) -> Result<T, ContractsError>
where
    T: for<'de> Deserialize<'de>,
{
    let content = fs::read_to_string(path).map_err(|source| ContractsError::Io {
        path: path.to_path_buf(),
        source,
    })?;

    serde_json::from_str(&content).map_err(|source| ContractsError::Json {
        path: path.to_path_buf(),
        source,
    })
}

fn has_duplicate_values<'a>(values: impl Iterator<Item = &'a str>) -> bool {
    let mut distinct = BTreeSet::new();

    for value in values {
        let normalized = value.trim().to_ascii_lowercase();
        if normalized.is_empty() {
            continue;
        }

        if !distinct.insert(normalized) {
            return true;
        }
    }

    false
}
