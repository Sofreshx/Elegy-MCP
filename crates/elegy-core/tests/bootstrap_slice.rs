use elegy_core::{compose_runtime, validate_descriptor_set, Catalog, ProjectLocator};
use std::fs;
use std::path::PathBuf;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|path| path.parent())
        .expect("workspace root")
        .to_path_buf()
}

#[test]
fn fs_static_example_validates() {
    let example = repo_root().join("examples/fs-static-minimal");
    let inspection =
        validate_descriptor_set(ProjectLocator::Path(example)).expect("config validates");

    assert_eq!(inspection.project_name, "fs-static-minimal");
    assert_eq!(inspection.root_config, "elegy.toml");
    assert_eq!(
        inspection.descriptor_files,
        vec!["elegy.resources.d/fs-static.toml"]
    );
    assert_eq!(inspection.resource_count, 2);
}

#[test]
fn fs_static_example_catalog_is_deterministic() {
    let example = repo_root().join("examples/fs-static-minimal");
    let first =
        compose_runtime(ProjectLocator::Path(example.clone())).expect("first composition succeeds");
    let second = compose_runtime(ProjectLocator::Path(example.clone()))
        .expect("second composition succeeds");
    let expected: Catalog = serde_json::from_str(
        &fs::read_to_string(example.join("expected-resources.json"))
            .expect("read expected manifest"),
    )
    .expect("parse expected manifest");

    assert_eq!(first, second);
    assert_eq!(first, expected);
}

#[test]
fn http_example_catalog_is_deterministic() {
    let example = repo_root().join("examples/http-minimal");
    let first =
        compose_runtime(ProjectLocator::Path(example.clone())).expect("first composition succeeds");
    let second = compose_runtime(ProjectLocator::Path(example.clone()))
        .expect("second composition succeeds");
    let expected: Catalog = serde_json::from_str(
        &fs::read_to_string(example.join("expected-resources.json"))
            .expect("read expected manifest"),
    )
    .expect("parse expected manifest");

    assert_eq!(first, second);
    assert_eq!(first, expected);
}

#[test]
fn http_openapi_example_still_rejects_openapi_runtime_execution() {
    let example = repo_root().join("examples/http-openapi-minimal");
    let error = compose_runtime(ProjectLocator::Path(example))
        .expect_err("open_api runtime support should remain scaffold-only");
    let codes: Vec<&str> = error
        .diagnostics()
        .iter()
        .map(|diagnostic| diagnostic.code.as_str())
        .collect();

    assert_eq!(codes, vec!["RUNTIME-UNSUPPORTED-FAMILY-002"]);
}

#[test]
fn duplicate_resource_uris_are_rejected() {
    let fixture = repo_root().join("tests/fixtures/fs/duplicate-uri");
    let error =
        compose_runtime(ProjectLocator::Path(fixture)).expect_err("duplicate URI should fail");
    let codes: Vec<&str> = error
        .diagnostics()
        .iter()
        .map(|diagnostic| diagnostic.code.as_str())
        .collect();

    assert!(codes.contains(&"RUNTIME-DUPLICATE-URI-001"));
}
