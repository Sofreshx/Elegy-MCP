---
created: 2026-03-17
updated: 2026-03-18
category: architecture
status: active
doc_kind: design
---

# `elegy-host-mcp` Boundary Design

## Purpose

Document the implemented host boundary for live MCP serving without pulling protocol or transport concerns into `elegy-core` or `elegy-runtime`.

The current host slice is intentionally narrow:

- resources-only
- stdio-first hosting
- static, filesystem, and plain constrained HTTP runtime families
- pinned MCP baseline `2025-11-25`

## Implemented boundary

The repository now preserves these boundaries:

- `elegy-runtime` owns deterministic resource catalog composition and rejects unsupported runtime families rather than pretending broader support exists.
- `elegy-core` is a thin orchestration facade over descriptor loading and runtime composition.
- `elegy-host-mcp` owns MCP SDK integration, stdio transport startup, and protocol mapping.
- `elegy-cli` remains a thin caller of library code rather than a second implementation of server behavior.

This keeps the boundary clean:

- core/runtime stay protocol-agnostic
- the CLI stays thin
- host-specific churn is isolated to `elegy-host-mcp`

## Public surface

### `elegy-runtime`

`elegy-runtime` now exposes host-usable state for:

- deterministic catalog inspection
- protocol-agnostic resource reads
- typed read failures that do not require MCP types

### `elegy-core`

`elegy-core` now exposes `compose_runtime_state(ProjectLocator)` and re-exports the runtime types needed by callers.

It remains responsible for:

- project discovery
- descriptor loading
- policy normalization
- top-level composition entrypoints

### `elegy-host-mcp`

`elegy-host-mcp` currently owns:

- MCP SDK integration via `rmcp`
- stdio transport startup
- `resources/list`
- `resources/read`
- an intentionally empty `resources/templates` surface
- mapping between runtime results and MCP protocol content

It does **not** parse project config directly and does **not** implement validation or policy logic itself.

## `resources/list` mapping

`resources/list` maps directly from the runtime catalog:

- `uri` -> MCP `uri`
- `title.unwrap_or_else(|| id.clone())` -> MCP `name`
- `title` -> MCP `title`
- `description` -> MCP `description`
- `mime_type` -> MCP `mimeType`

Current rules:

- preserve runtime ordering; do not let the SDK reorder resources
- expose only runtime-composed resources from the current supported families
- do not invent host-only metadata fields unless the spec requires them

## `resources/read` mapping

`resources/read` resolves by `uri` against `RuntimeState::read_resource(uri)`.

All family-specific access safety stays in runtime and adapter code. The host only translates successful reads and typed failures into MCP-facing results.

### Content-kind rules

The host maps runtime bytes to MCP content using MIME semantics first:

- explicitly textual MIME types produce MCP `text` content when bytes decode as UTF-8
- non-textual MIME types produce MCP `blob` content even if the bytes happen to be UTF-8-compatible
- binary fallback stays base64-encoded at the protocol boundary

This keeps the host aligned with the declared resource MIME type instead of making the protocol shape depend on payload coincidence.

### Filesystem and static resources

Filesystem and static resources are read entirely through runtime-owned behavior, including:

- root containment
- symlink policy
- file-size bounds
- inline static size enforcement

### Plain constrained HTTP resources

Plain constrained HTTP resources should also flow through `RuntimeState::read_resource(uri)` rather than through host-owned request logic.

That preserves the current trust boundary:

- HTTP allowlist, credential rejection, redirect refusal, timeout handling, and bounded response size stay in runtime/adapter code
- the host only maps the already-bounded result into MCP content

### Read errors

The host currently maps runtime read failures to MCP-facing errors as follows:

- `UnknownResource` -> invalid params / resource not found error
- `AccessDenied` -> internal error with safe message
- `InvalidResourceState` -> internal error with safe message
- `Io` -> internal error with safe message
- `NotYetSupported` -> method-level failure describing unsupported family

The host should never leak raw filesystem paths unless the existing diagnostic policy intentionally allows it.

## Spec-version handling

Spec-version negotiation and SDK-specific behavior live inside `elegy-host-mcp`, not in the CLI and not in the runtime.

The current crate keeps that logic consolidated in its host implementation rather than splitting protocol and transport modules yet. If protocol churn or transport breadth increases later, that internal split can still happen without moving the boundary.

## MCP SDK integration boundary

The MCP SDK dependency lives only in `elegy-host-mcp`.

Do not add the SDK dependency to:

- `elegy-runtime`
- `elegy-core`
- `elegy-cli`

The SDK should stay wrapped behind small internal adapter functions so the rest of the crate continues to speak in terms of runtime-owned data and host-owned errors.

## CLI integration

`elegy-cli` now delegates live startup to `elegy-host-mcp::serve_stdio(...)`.

Current behavior:

- `elegy run` starts the stdio MCP host
- `elegy run --dry-run` performs transport-free validation only
- JSON output is intentionally rejected for live stdio mode because it would corrupt MCP traffic on stdout

The CLI still owns argument parsing, human/script-facing command UX, and process exit codes. Protocol behavior stays in the host crate.

## Current implementation checkpoint

At this checkpoint, "done" for the first host wave means:

- a caller can start an MCP server over stdio from an Elegy project path
- the server can list the current supported resources from `examples/fs-static-minimal` and `examples/http-minimal`
- the server can read static/filesystem resources and plain constrained HTTP resources through the shared runtime boundary
- unsupported runtime families still fail before the host pretends they are live
- host mapping is regression-tested for:
  - list/read behavior over duplex transport
  - MIME-aware text vs blob mapping

## Known follow-up

The current host uses `spawn_blocking(...)` for runtime reads to avoid blocking the async MCP handler path. That is correct for the current foundation, but it leaves cancellation/shutdown behavior less bounded than the final design should be, especially for in-flight HTTP reads. Tightening that lifecycle behavior is the next hardening follow-up for the host layer.

## Deferred on purpose

Not in this wave:

- tools
- prompts
- sampling
- subscriptions
- SSE/HTTP transport hosting
- HTTP/OpenAPI runtime execution
- hot reload
- caching and watchers
- packaging or deployment flows

## Summary

The current boundary is:

- `elegy-runtime`: protocol-agnostic list/read runtime state
- `elegy-core`: project-discovery facade for composing that state
- `elegy-host-mcp`: MCP SDK, spec negotiation, stdio transport, and runtime-to-protocol mapping
- `elegy-cli`: thin delegation only

That is the current implemented shape that enables real MCP hosting while keeping protocol concerns out of the already-stable bootstrap layers.
