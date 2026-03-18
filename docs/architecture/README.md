---
created: 2026-03-17
updated: 2026-03-18
category: architecture
status: active
doc_kind: overview
---

# Architecture Overview

## Purpose

Provide a repo-local architecture entrypoint for the bootstrap stage of Elegy MCP.

This document summarizes the current bootstrap architecture so contributors can understand what the repository already implements, what remains scaffolded, and where the trust boundaries sit.

## Context

Elegy MCP is no longer just a greenfield plan. The repository now contains the first implemented bootstrap slice.

The accepted direction remains:

- Rust-first
- runtime composition for v1
- resources-only v1
- first resource families: filesystem/static, plain constrained HTTP, and OpenAPI descriptor scaffolding
- Apache-2.0 licensing baseline
- MCP spec baseline `2025-11-25`

This architecture document is intentionally honest about timing: it describes a **partially implemented bootstrap architecture**, not a fully implemented system.

## Current bootstrap package shape

The thin-first Rust workspace now exists in bootstrap form:

```text
Elegy-MCP/
├─ crates/
│  ├─ elegy-adapter-fs/
│  ├─ elegy-adapter-http/
│  ├─ elegy-descriptor/
│  ├─ elegy-policy/
│  ├─ elegy-runtime/
│  ├─ elegy-core/
│  ├─ elegy-cli/
│  └─ elegy-host-mcp/
├─ docs/
│  └─ architecture/
└─ examples/
   ├─ fs-static-minimal/
   ├─ http-minimal/
   └─ http-openapi-minimal/
```

Adapter extraction is now implemented for the proven bootstrap families:

- `elegy-adapter-fs` owns static/filesystem family-specific composition and read behavior
- `elegy-adapter-http` owns plain constrained HTTP policy/composition/read behavior
- `elegy-runtime` remains the public orchestrator and caller-facing runtime API

Later crates are still planned, but intentionally deferred until the foundational contracts are stable:

- `elegy-audit-functional`
- `elegy-audit-security`

## Responsibilities by layer

### Descriptor

`elegy-descriptor` owns parsing, normalization, and structural validation of project config and resource descriptors.

In the current bootstrap slice it already understands:

- root project config discovery
- descriptor file expansion
- static resources
- filesystem resources
- HTTP resource descriptors
- OpenAPI resource descriptors

### Policy

`elegy-policy` defines the rules that bound runtime behavior, including filesystem roots, outbound HTTP targets, and safe defaults.

Today that includes:

- filesystem root allowlists
- file-size limits
- symlink policy
- allowed HTTP target prefixes
- plaintext HTTP allowance
- timeout and max-response settings as normalized policy values

### Runtime

`elegy-runtime` composes validated descriptors into a deterministic in-memory resource catalog, dispatches the implemented adapter crates, and enforces cross-resource rules for the implemented families.

Current bootstrap behavior is intentionally split:

- **implemented**: static, filesystem, and plain constrained HTTP resource composition
- **not yet composed**: OpenAPI resources

For OpenAPI resources, the runtime currently fails with explicit diagnostics rather than pretending that family is fully supported. The implemented HTTP path remains intentionally narrow and bounded by declared policy.

### Core facade

`elegy-core` presents the narrow public library API used by both automation and the CLI.

It currently handles:

- project discovery
- config validation
- runtime composition orchestration
- host-ready runtime state composition
- normalized config inspection output

### Host

`elegy-host-mcp` is the live protocol adapter for the current resources-only surface.

Today it owns:

- stdio transport startup
- MCP resources/list mapping from the runtime catalog
- MCP resources/read mapping from runtime reads
- an intentionally empty resources/templates surface

It does not own project discovery, descriptor validation, or policy enforcement.

### CLI

`elegy-cli` remains a thin operator shell over the core library, not a parallel implementation of validation or runtime logic.

Implemented commands today are:

- `validate config`
- `validate runtime`
- `inspect resources`
- `run`
- `run --dry-run`

`run` starts the live stdio host for the current resources-only surface. `run --dry-run` preserves the transport-free validation path. Audit flows, packaging, and project initialization remain outside the current bootstrap implementation.

## Trust boundaries

The main trust boundaries for v1 are already visible in the code and examples.

### Less-trusted inputs

- project config and descriptor files
- inline static content declarations
- upstream HTTP/OpenAPI descriptions and responses
- filesystem paths and file contents under configured roots
- CLI flags and environment inputs
- MCP client read inputs

### Trusted core

- descriptor normalization
- policy evaluation
- runtime composition and dispatch
- audit orchestration

### External systems

- upstream HTTP services
- local filesystem
- future secret-reference systems

## Security-sensitive seams

The design assumes special care at these seams:

1. descriptor ingestion
2. secret and auth reference handling
3. outbound HTTP access
4. filesystem traversal and normalization
5. MCP response and logging surfaces
6. persisted audit artifacts

The project should fail closed when a descriptor is ambiguous, unsupported, or widens access beyond the declared trust boundary.

## What this document does not claim

This page does not claim that:

- every planned crate exists
- the CLI contract is fully shipped
- OpenAPI runtime composition is complete
- the security or functional audit subsystems are complete

Those pieces remain accepted direction, not finished implementation, with the exception that stdio MCP hosting now exists for the current resources-only slice.

## References

- [Repository README](../../README.md)
- [Spec baseline](../spec-baseline.md)
- [Examples overview](../../examples/README.md)
- [Adapter extraction design](./adapter-extraction-design.md)
