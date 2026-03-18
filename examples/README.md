# Examples

Examples in Elegy MCP are intended to be more than tutorial material.

From the beginning, the project treats examples as:

- contributor onboarding assets
- acceptance inputs for tests and audits
- small, reviewable demonstrations of the supported runtime model

## Bootstrap-stage status

The bootstrap slice currently includes both implemented acceptance examples and a deferred-scope example:

### Executable today

- `examples/fs-static-minimal/`
  - working bootstrap example for filesystem/static validation and runtime composition
  - appropriate for `validate config`, `validate runtime`, `inspect resources`, `run --dry-run`, and live `run`
  - includes `expected-resources.json` as a concrete catalog reference for the current implemented path
- `examples/http-minimal/`
  - working bootstrap example for the implemented plain constrained HTTP runtime path
  - appropriate for `validate config`, `validate runtime`, `inspect resources`, `run --dry-run`, and live `run`
  - demonstrates bounded outbound HTTP-backed resource composition under the current policy model

### Scaffolded today

- `examples/http-openapi-minimal/`
  - bootstrap placeholder for OpenAPI descriptor and policy shape
  - useful for contributor review and future runtime composition work
  - includes a placeholder `expected-resources.json` specifically because OpenAPI runtime composition is still deferred in the current bootstrap slice

This distinction matters: not every example in the repository is meant to prove an implemented end-to-end runtime path yet.

## Example rules

When examples are added, they should be:

- small
- hermetic where possible
- easy to understand without prior project knowledge
- safe to run in local development and CI
- aligned with the actual supported feature set

## How to use the current examples

### Filesystem/static minimal

This is the current acceptance example for the implemented bootstrap runtime. It exercises:

- project config loading
- descriptor normalization
- deterministic resource inspection
- bounded filesystem behavior
- runtime catalog composition for filesystem and inline static resources
- live stdio MCP hosting for resources/list-read

### HTTP minimal

This is the acceptance example for the implemented plain constrained HTTP slice. It exercises:

- HTTP descriptor loading and normalization
- policy-bounded outbound HTTP configuration
- runtime catalog composition for the current HTTP family
- deterministic inspection output for the implemented bootstrap path
- live stdio MCP hosting for the current resources-only host boundary

The example should still be read conservatively: it proves the narrow bootstrap HTTP path, not a broad HTTP integration surface.

### HTTP/OpenAPI minimal

This example currently exists to keep the OpenAPI/bootstrap story honest and reviewable:

- the repository already has OpenAPI descriptor forms
- policy configuration already models the related outbound HTTP constraints
- the example gives contributors a concrete placeholder input to work from

What it does **not** currently prove is OpenAPI operation execution or successful runtime composition for that family.

## Reference links

- [Repository README](../README.md)
- [Architecture overview](../docs/architecture/README.md)
