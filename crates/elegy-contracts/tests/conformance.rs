use elegy_contracts::{
    default_support_manifest_path, load_compatibility_manifest_from_dir,
    load_consumer_support_manifest, load_mcp_analysis_result_fixture_from_dir,
    load_mcp_server_descriptor_fixture_from_dir, load_skill_definition_fixture_from_dir,
    load_skill_discovery_index_fixture_from_dir, resolve_upstream_contracts_dir,
    validate_mcp_analysis_result, validate_mcp_server_descriptor, validate_skill_definition,
    validate_support_manifest_against_upstream, McpAnalysisResult, McpServerDescriptor,
    McpToolAnalysis, McpToolDefinition, SkillApprovalRequirement, SkillDefinition,
    SkillGovernanceMetadata, SkillMaterializationKind, SkillOrigin, SkillSourceKind,
};
use std::collections::BTreeSet;

#[test]
fn upstream_bundle_contains_supported_schema_entries() {
    let contracts_dir = resolve_upstream_contracts_dir();
    let upstream = load_compatibility_manifest_from_dir(&contracts_dir)
        .expect("load upstream compatibility manifest");
    let support = load_consumer_support_manifest(&default_support_manifest_path())
        .expect("load local support manifest");

    validate_support_manifest_against_upstream(&support, &upstream)
        .expect("support manifest should match upstream bundle");

    let schema_names = upstream
        .schemas
        .iter()
        .map(|entry| entry.name.as_str())
        .collect::<BTreeSet<_>>();

    assert!(schema_names.contains("skill-definition"));
    assert!(schema_names.contains("skill-discovery-index"));
    assert!(schema_names.contains("mcp-tool-definition"));
    assert!(schema_names.contains("mcp-server-descriptor"));
    assert!(schema_names.contains("mcp-analysis-result"));
}

#[test]
fn upstream_skill_definition_fixture_is_semantically_valid() {
    let contracts_dir = resolve_upstream_contracts_dir();
    let definition = load_skill_definition_fixture_from_dir(&contracts_dir)
        .expect("load upstream skill-definition fixture");

    let validation = validate_skill_definition(&definition);
    assert!(
        validation.is_valid(),
        "unexpected issues: {:?}",
        validation.issues
    );
}

#[test]
fn upstream_skill_discovery_fixture_round_trips_as_projection() {
    let contracts_dir = resolve_upstream_contracts_dir();
    let index = load_skill_discovery_index_fixture_from_dir(&contracts_dir)
        .expect("load upstream skill-discovery fixture");

    assert_eq!(index.schema_version, 1);
    assert_eq!(index.entries.len(), 1);
    assert_eq!(index.entries[0].skill_id, "example-skill");
    assert_eq!(index.entries[0].manifest.id, "example-skill");

    let json = serde_json::to_string(&index).expect("serialize discovery index");
    let reparsed = serde_json::from_str(&json).expect("deserialize discovery index");

    assert_eq!(index, reparsed);
}

#[test]
fn validator_matches_phase_two_governance_and_origin_rules() {
    let approval_required = SkillDefinition {
        id: "skill.example".to_string(),
        name: "Example skill".to_string(),
        governance: SkillGovernanceMetadata {
            approval_requirement: SkillApprovalRequirement::Required,
            ..SkillGovernanceMetadata::default()
        },
        ..SkillDefinition::default()
    };

    let approval_validation = validate_skill_definition(&approval_required);
    assert!(approval_validation.issues.contains(
        &"Skills that require approval must declare at least one policy reference.".to_string()
    ));

    let dynamic_manual = SkillDefinition {
        id: "skill.dynamic".to_string(),
        name: "Dynamic skill".to_string(),
        origin: SkillOrigin {
            materialization_kind: SkillMaterializationKind::Dynamic,
            source_kind: SkillSourceKind::Manual,
            ..SkillOrigin::default()
        },
        ..SkillDefinition::default()
    };

    let origin_validation = validate_skill_definition(&dynamic_manual);
    assert!(origin_validation.issues.contains(
        &"Dynamic skills must declare either a source reference or a non-manual source kind."
            .to_string()
    ));
}

#[test]
fn upstream_mcp_server_descriptor_fixture_is_semantically_valid() {
    let contracts_dir = resolve_upstream_contracts_dir();
    let descriptor = load_mcp_server_descriptor_fixture_from_dir(&contracts_dir)
        .expect("load upstream mcp-server-descriptor fixture");

    let validation = validate_mcp_server_descriptor(&descriptor);
    assert!(
        validation.is_valid(),
        "unexpected issues: {:?}",
        validation.issues
    );

    assert_eq!(descriptor.server_name, "weather-server");
    assert_eq!(descriptor.tools.len(), 1);
    assert_eq!(descriptor.tools[0].name, "get-weather");
}

#[test]
fn upstream_mcp_analysis_result_fixture_is_semantically_valid() {
    let contracts_dir = resolve_upstream_contracts_dir();
    let analysis = load_mcp_analysis_result_fixture_from_dir(&contracts_dir)
        .expect("load upstream mcp-analysis-result fixture");

    let validation = validate_mcp_analysis_result(&analysis);
    assert!(
        validation.is_valid(),
        "unexpected issues: {:?}",
        validation.issues
    );

    assert_eq!(analysis.server_name, "weather-server");
    assert_eq!(analysis.analyses.len(), 1);
    assert_eq!(analysis.analyses[0].tool.name, "get-weather");
    assert_eq!(
        analysis.analyses[0].extracted_triggers[0].pattern,
        "get weather"
    );
}

#[test]
fn mcp_validators_reject_duplicate_and_inconsistent_entries() {
    let descriptor = McpServerDescriptor {
        server_name: "duplicate-server".to_string(),
        tools: vec![
            McpToolDefinition {
                name: "get-weather".to_string(),
                ..McpToolDefinition::default()
            },
            McpToolDefinition {
                name: "get-weather".to_string(),
                ..McpToolDefinition::default()
            },
        ],
        ..McpServerDescriptor::default()
    };

    let descriptor_validation = validate_mcp_server_descriptor(&descriptor);
    assert!(descriptor_validation
        .issues
        .contains(&"MCP server descriptor tool names must be unique.".to_string()));

    let analysis = McpAnalysisResult {
        server_name: "duplicate-server".to_string(),
        analyses: vec![McpToolAnalysis {
            tool: McpToolDefinition {
                name: "get-weather".to_string(),
                input_schema: None,
                ..McpToolDefinition::default()
            },
            has_valid_schema: true,
            ..McpToolAnalysis::default()
        }],
    };

    let analysis_validation = validate_mcp_analysis_result(&analysis);
    assert!(analysis_validation.issues.contains(
        &"MCP analysis entries marked as having a valid schema must include an input schema."
            .to_string()
    ));
}
