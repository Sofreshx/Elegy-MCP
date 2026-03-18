---
created: 2026-03-17
updated: 2026-03-17
category: architecture
status: proposed
doc_kind: implementation_slice
---

# HTTP Runtime Slice Design

## Goal

Define the smallest honest next implementation slice for HTTP/OpenAPI runtime support without rewriting the bootstrap architecture.

## Decision

Implement **plain constrained HTTP descriptors first** and keep **OpenAPI-derived resources scaffold-only** for one more slice.

This means the next slice should land:

- runtime catalog composition for `kind = "http"` resources
- a minimal bounded outbound GET execution path for those resources
- hermetic tests proving policy enforcement, response limits, and normalized failures

This slice should **not** land OpenAPI execution, parameter binding, or schema-driven request generation yet.

## Why this is the right slice

The repository already has the contracts needed to parse and normalize plain HTTP descriptors:

- `HttpResource` already has `base_url` and `path`
- HTTP policy is already normalized with allowlist, timeout, and max-response settings
- the runtime already rejects unsupported families explicitly instead of silently widening scope

OpenAPI is materially larger because it adds:

- document loading and validation
- operation resolution
- parameter location handling
- coercion and encoding rules
- more complicated trust-boundary review

Plain HTTP lets the project convert the existing scaffold into a real runtime path while keeping the current bootstrap posture conservative.

## Slice boundary

### In scope now

1. Support `NormalizedResource::Http` in runtime composition.
2. Add a deterministic catalog representation for HTTP resources.
3. Add a minimal read/execution primitive for HTTP resources only:
   - method is fixed to `GET`
   - target URL is `base_url + path`
   - no runtime parameters
   - no request body
   - no custom headers
   - no redirects
4. Enforce existing HTTP policy values at runtime:
   - `allowed_targets`
   - `allow_plaintext_http`
   - `timeout_ms`
   - `max_response_size_bytes`
5. Normalize outbound failures into stable runtime diagnostics/errors.
6. Add one working HTTP example and hermetic tests.

### Explicitly deferred

- `NormalizedResource::OpenApi` runtime composition beyond scaffold diagnostics
- operation lookup from OpenAPI documents
- path/query/header/cookie parameter mapping
- schema coercion or response-shape validation
- auth, secrets, bearer tokens, API keys, or custom headers
- redirects, retries, caching, cookies, and content negotiation
- mutation methods (`POST`, `PUT`, `PATCH`, `DELETE`)
- CLI or MCP-hosted live invocation flows

## Required model and runtime changes

### `elegy-descriptor`

Keep the HTTP descriptor model intentionally small. Do **not** add new descriptor fields in this slice.

Required adjustments:

1. Keep `HttpResource { base_url, path }` as the only execution inputs.
2. Tighten HTTP path normalization so first-slice HTTP resources stay literal and non-templated:
   - still must start with `/`
   - reject fragments
   - reject full absolute URLs in `path`
   - reject path templates such as `{id}`
   - reject embedded query strings for now
3. Leave `OpenApiResource` untouched.

This keeps parameter mapping out of scope by construction.

### `elegy-policy`

No schema rewrite is needed.

Reuse the existing policy surface exactly as-is:

- `allowed_targets`
- `allow_plaintext_http`
- `timeout_ms`
- `max_response_size_bytes`

Any policy growth beyond that should stay deferred.

### `elegy-runtime`

This crate gets the real implementation work.

Add:

1. `CatalogSource::Http { descriptor, base_url, path, method }`
2. HTTP catalog composition for `NormalizedResource::Http`
3. A small execution abstraction, for example:
   - `HttpClient` trait
   - `execute_http_resource(...) -> Result<HttpReadOutput, HttpReadError>`
4. A normalized response type carrying:
   - final requested URL
   - status code
   - content type if present
   - bounded body bytes
5. Stable error categories/codes for:
   - policy denial
   - invalid target join
   - timeout
   - transport failure
   - redirect denied
   - upstream non-success status
   - response too large
   - invalid UTF-8 only when a caller explicitly asks for text

Keep existing catalog composition entrypoints in place. Do not replace `compose_catalog`; extend it.

### `elegy-core`

Keep the current thin-facade behavior.

Required changes should stay additive:

- `compose_runtime()` should stop rejecting plain HTTP resources once runtime composition supports them
- the existing CLI commands should automatically benefit from the new catalog support
- no new CLI verbs are required in this slice

## Trust-boundary rules for the first slice

### Allowed targets

The runtime should validate the outbound target twice:

1. at composition time, after joining `base_url` and `path`
2. again immediately before execution

The joined URL must:

- remain `http` or `https`
- match the configured allowlist using the existing policy logic
- preserve the fail-closed behavior when no allowlist is configured

### Parameter mapping

There should be **no runtime parameter mapping** in this slice.

That is the tightest boundary and the smallest honest step. A resource either maps to one exact GET URL or it is deferred.

Concretely:

- no URI-template expansion
- no query parameter injection
- no header injection
- no body construction

If a future use case needs parameters, that belongs to the next slice after the plain HTTP path is stable.

### Error normalization

Normalize all HTTP execution failures into stable, non-leaky categories.

Recommended first-slice codes:

- `RUNTIME-HTTP-001` target URL could not be formed from `base_url` + `path`
- `RUNTIME-HTTP-002` target URL is outside policy
- `RUNTIME-HTTP-003` request timed out
- `RUNTIME-HTTP-004` transport request failed
- `RUNTIME-HTTP-005` redirect refused
- `RUNTIME-HTTP-006` upstream returned non-success status
- `RUNTIME-HTTP-007` response exceeded `max_response_size_bytes`

Messages should be clear and operator-facing, but should not dump arbitrary upstream bodies into diagnostics.

### Response limits

The first slice must treat response bounding as a hard requirement, not a later enhancement.

Rules:

1. Enforce `max_response_size_bytes` while reading, not only from `Content-Length`.
2. If `Content-Length` is present and already exceeds the limit, fail before reading the body.
3. If streamed bytes cross the limit, abort and return `RUNTIME-HTTP-007`.
4. Disable redirects so an allowlisted URL cannot silently bounce to a non-allowlisted host.
5. Apply `timeout_ms` as a full request deadline for the minimal client.

## Acceptance example

Add a new working example rather than overloading the current scaffold placeholder:

- `examples/http-minimal/`

Recommended contents:

- one `http` resource
- policy allowlisting for exactly one local or example target scope
- expected catalog snapshot showing the resource as runtime-supported

Example descriptor shape:

- `base_url = "https://api.example.com"`
- `path = "/status"`

The example should prove catalog support, not live remote network access in CI.

Keep `examples/http-openapi-minimal/` as the explicit scaffold-only OpenAPI marker for now.

## First tests

### Catalog tests

Add deterministic tests proving that:

1. a plain HTTP resource now composes into the runtime catalog
2. its catalog source is `kind = "http"`
3. its limit uses `policy.http.max_response_size_bytes`
4. `open_api` resources are still rejected with explicit scaffold diagnostics

### Execution tests

Add hermetic runtime tests with an in-process local HTTP server or a stubbed client behind `HttpClient`.

The first test set should prove:

1. **success path**
   - allowlisted URL
   - `GET`
   - bounded body returned
2. **allowlist denial**
   - target outside `allowed_targets`
   - stable normalized error
3. **plaintext denial**
   - `http://` target rejected when `allow_plaintext_http = false`
4. **oversize response**
   - response larger than `max_response_size_bytes`
   - read aborted with normalized error
5. **non-success upstream**
   - e.g. `404`
   - normalized non-success error without embedding full body text
6. **redirect refusal**
   - `302` response produces redirect-denied error

## Implementation map

### Files to modify

- `crates/elegy-descriptor/src/lib.rs`
  - tighten `normalize_http_path`
- `crates/elegy-runtime/src/lib.rs`
  - add HTTP catalog composition
  - add HTTP source variant
  - add minimal execution primitive
  - add normalized HTTP runtime errors
- `crates/elegy-runtime/Cargo.toml`
  - add the minimal blocking HTTP client dependency selected for the slice
- `crates/elegy-core/tests/bootstrap_slice.rs`
  - add catalog acceptance coverage for working HTTP resources
  - preserve explicit OpenAPI scaffold-only coverage

### Files to add

- `examples/http-minimal/elegy.toml`
- `examples/http-minimal/elegy.resources.d/http.toml`
- `examples/http-minimal/expected-resources.json`
- `crates/elegy-runtime/tests/http_runtime.rs` or inline crate tests
  - execution-path coverage with hermetic client/server setup

## Data flow for the slice

1. `elegy-descriptor` loads and normalizes the project.
2. `elegy-core` converts raw policy into `PolicyConfig`.
3. `elegy-runtime::compose_catalog`:
   - deduplicates IDs and URIs
   - joins `base_url` + `path`
   - validates target against policy
   - emits a `CatalogResource` with HTTP source metadata
4. Future caller asks runtime to execute the HTTP resource.
5. Runtime re-validates the target, issues a bounded GET, and reads at most `max_response_size_bytes`.
6. Runtime returns normalized output or a stable normalized error.

## What remains scaffold-only after this slice

After this slice, these should still remain scaffold-only:

- `kind = "open_api"` catalog/runtime support
- OpenAPI document parsing beyond descriptor shape validation
- operation parameter mapping and schema coercion
- auth and secret resolution
- any live MCP server integration
- any broad claim that HTTP/OpenAPI is fully complete

## Build sequence

1. Tighten descriptor validation for literal-only HTTP paths.
2. Extend runtime catalog types with HTTP source support.
3. Compose plain HTTP resources into the catalog.
4. Add minimal HTTP execution primitive with injected client.
5. Add hermetic execution tests for success and failure boundaries.
6. Add `examples/http-minimal` and expected catalog snapshot.
7. Update docs/README language only where needed to reflect that plain HTTP is now implemented while OpenAPI remains deferred.

## Done criteria

The slice is done when all of the following are true:

- `validate runtime` succeeds for `examples/http-minimal`
- `inspect resources` shows the HTTP resource in deterministic catalog output
- runtime tests prove allowlist enforcement, timeout/transport normalization, redirect refusal, and size limits
- `examples/http-openapi-minimal` still documents deferred OpenAPI behavior honestly
- repository docs say plain HTTP runtime support exists, while OpenAPI runtime support does not yet
