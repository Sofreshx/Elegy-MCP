# Elegy MCP

Elegy MCP is a Rust-first project for composing Model Context Protocol (MCP) servers at runtime from declarative resource definitions.

> [!IMPORTANT]
> This repository is in the **bootstrap stage**. It is not a full product yet. The current slice includes a real Rust workspace, foundational crates, a bootstrap CLI, working filesystem/static and plain constrained HTTP runtime paths, a live stdio MCP host for resources/list-read, and conservative OpenAPI scaffolding.

## Current status

Today, this repository contains the first implemented bootstrap slice rather than just a design stub.

### Implemented now

- a Rust workspace with:
  - `elegy-adapter-fs`
  - `elegy-adapter-http`
  - `elegy-descriptor`
  - `elegy-policy`
  - `elegy-runtime`
  - `elegy-core`
  - `elegy-cli`
  - `elegy-host-mcp`
- descriptor loading, normalization, and structural validation for:
   - static resources
   - filesystem resources
   - HTTP resource descriptors
   - OpenAPI resource descriptors
- runtime composition for:
   - static resources
   - filesystem resources
   - plain constrained HTTP resources
- bootstrap policy handling for:
  - filesystem roots, size limits, and symlink policy
  - allowed HTTP targets, plaintext HTTP policy, timeouts, and response-size limits
- a bootstrap CLI with these implemented commands:
  - `validate config`
  - `validate runtime`
  - `inspect resources`
  - `run`
  - `run --dry-run`
- a live stdio MCP host for the currently supported resources-only surface:
  - `resources/list`
  - `resources/read`
  - empty `resources/templates`
- a working example project in [`examples/fs-static-minimal`](examples/fs-static-minimal/) for filesystem/static validation and runtime composition
- a working example project in [`examples/http-minimal`](examples/http-minimal/) for the implemented plain constrained HTTP runtime path
- a placeholder OpenAPI example in [`examples/http-openapi-minimal`](examples/http-openapi-minimal/) that documents the deferred slice and exercises descriptor/policy shape
- bootstrap infra and documentation baselines for contributors

### Scaffolded or deferred

- write or mutation operations
- functional or security audit implementations
- the broader planned CLI surface such as `init`, `audit`, and `package`
- broad resource family support beyond the current bootstrap slice
- transports beyond stdio live hosting
- OpenAPI runtime composition and operation execution

In the current checkout, plain constrained HTTP should be treated as an implemented bootstrap runtime path, while OpenAPI should still be treated as **descriptor and policy scaffolding**. The repository includes OpenAPI in config/schema terms, but the bootstrap runtime only composes the filesystem/static and plain constrained HTTP families today.

### Accepted direction that still holds

- the implementation direction is **Rust-first**
- v1 is **runtime composition**, not code generation
- v1 is **resources-only**
- the open-source baseline is **Apache-2.0**
- the pinned MCP spec baseline is **2025-11-25**

## What Elegy MCP is for

The project aims to provide:

- a reusable core library for loading, validating, and composing MCP resource definitions
- a thin CLI for validation, inspection, packaging, and server startup
- deterministic validation and audit-friendly outputs for local use and CI
- safe defaults around trust boundaries, path handling, and outbound access

## v1 scope

The initial release is intentionally narrow.

### In scope

- runtime-composed MCP servers
- MCP resource listing and reading
- filesystem/static resources
- plain constrained HTTP-backed resources within declared policy bounds
- OpenAPI-backed resources as a staged capability, with bootstrap work currently limited to descriptor/policy scaffolding
- validation, policy enforcement, and audit-oriented design

### Out of scope for v1

- MCP tools, prompts, and sampling
- build-time generation flows
- write or mutation operations
- plugin marketplaces or general third-party extension APIs
- hosted control-plane features
- broad security claims such as malware detection or total assurance

## Bootstrap-stage quickstart

To get started with the bootstrap slice:

1. Run the workspace checks:

   ```bash
   cargo fmt --all --check
   cargo clippy --workspace --all-targets --all-features -- -D warnings
   cargo test --workspace --all-targets --all-features
   ```

2. Try the implemented filesystem/static path against [`examples/fs-static-minimal`](examples/fs-static-minimal/):

   ```bash
   cargo run -p elegy-cli -- --project examples/fs-static-minimal validate config
   cargo run -p elegy-cli -- --project examples/fs-static-minimal validate runtime
    cargo run -p elegy-cli -- --project examples/fs-static-minimal inspect resources
    cargo run -p elegy-cli -- --project examples/fs-static-minimal run --dry-run
    cargo run -p elegy-cli -- --project examples/fs-static-minimal run
    ```

3. Try the implemented plain constrained HTTP path against [`examples/http-minimal`](examples/http-minimal/):

   ```bash
   cargo run -p elegy-cli -- --project examples/http-minimal validate config
   cargo run -p elegy-cli -- --project examples/http-minimal validate runtime
    cargo run -p elegy-cli -- --project examples/http-minimal inspect resources
    cargo run -p elegy-cli -- --project examples/http-minimal run --dry-run
    cargo run -p elegy-cli -- --project examples/http-minimal run
    ```

    In live mode, `run` starts the stdio MCP host and serves the currently supported resources-only surface. The HTTP path is intentionally narrow. It is meant to prove bounded HTTP-backed resource composition under the declared bootstrap policy model, not a broad HTTP integration surface.

4. Use the OpenAPI example as a scope marker, not as a fully working end-to-end flow:

   ```bash
   cargo run -p elegy-cli -- --project examples/http-openapi-minimal validate config
   ```

   That example is useful for contributor review and schema alignment. It should not be read as proof that OpenAPI runtime composition is complete in the bootstrap slice.

5. Read the [architecture overview](docs/architecture/README.md).
6. Read the [spec baseline](docs/spec-baseline.md).
7. Review [contribution guidance](CONTRIBUTING.md).
8. Review [security reporting guidance](SECURITY.md).
9. Follow progress through the [changelog](CHANGELOG.md).

## Current repository shape

The first implemented workspace currently looks like this:

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

Audit crates are still planned, but they are intentionally deferred until the foundational runtime and host slice are stable.

## CLI surface

The accepted v1 CLI contract centers on a single binary named `elegy`.

Implemented today:

- `validate config`
- `validate runtime`
- `inspect resources`
- `run`
- `run --dry-run`

Still planned:

- `init`
- `audit`
- `package`

The bootstrap CLI is intentionally thin. It currently validates configuration, composes the supported filesystem/static/plain-HTTP runtime slice, prints deterministic JSON/text output for inspection flows, starts a stdio MCP host in live `run` mode, and keeps `run --dry-run` as the transport-free validation path.

## Documentation map

- [Architecture overview](docs/architecture/README.md)
- [MCP spec baseline](docs/spec-baseline.md)
- [Examples overview](examples/README.md)
- [Contributing](CONTRIBUTING.md)
- [Security policy](SECURITY.md)
- [Code of conduct](CODE_OF_CONDUCT.md)

## Contributing

Contributions are welcome, but please keep changes aligned with the current stage of the repository:

- prefer clarity over aspirational claims
- keep docs honest about what exists today
- keep HTTP and OpenAPI language conservative unless the code and example outputs prove a broader claim
- avoid widening v1 scope without an explicit decision
- open an issue or draft PR when a change would affect architecture, CLI contract, or security posture

See [CONTRIBUTING.md](CONTRIBUTING.md) for details.

## License

This project is licensed under the Apache License, Version 2.0. See [LICENSE](LICENSE) and [NOTICE](NOTICE).
