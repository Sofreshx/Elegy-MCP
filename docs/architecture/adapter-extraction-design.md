---
created: 2026-03-17
updated: 2026-03-17
category: architecture
status: proposed
doc_kind: implementation_slice
---

# Adapter Extraction Design

## Goal

Define the smallest safe extraction wave that moves family-specific runtime behavior out of `elegy-runtime` without changing the validated bootstrap behavior or pulling host/OpenAPI concerns forward.

## Current patterns and constraints

The repository already establishes the architectural shape this extraction needs to preserve:

- `elegy-runtime` is the family-neutral orchestration point for composition, catalog ordering, duplicate detection, and runtime state assembly.
- family-specific behavior is currently colocated in `elegy-runtime`, especially:
  - static/filesystem composition and read logic
  - plain constrained HTTP composition and read logic
- `elegy-core` is intentionally thin and should not learn family-specific execution rules.
- the accepted package shape already plans `elegy-adapter-fs` and `elegy-adapter-http` as later crates, but not a broader plugin system yet.
- OpenAPI remains scaffold-only at runtime and must stay that way in this wave.

Concrete evidence in the current codebase:

- planned adapter crates are already documented in the architecture overview
  - `docs/architecture/README.md:52`
- runtime composition currently mixes all supported families in one loop
  - `crates/elegy-runtime/src/lib.rs:629`
- filesystem/static composition is currently runtime-owned
  - `crates/elegy-runtime/src/lib.rs:807`
  - `crates/elegy-runtime/src/lib.rs:827`
- filesystem read-time safety checks are currently runtime-owned
  - `crates/elegy-runtime/src/lib.rs:404`
- plain constrained HTTP composition and execution are currently runtime-owned
  - `crates/elegy-runtime/src/lib.rs:519`
  - `crates/elegy-runtime/src/lib.rs:974`
- OpenAPI is still intentionally rejected during runtime composition
  - `crates/elegy-runtime/src/lib.rs:688`
- the core facade remains a narrow orchestrator and should stay that way
  - `crates/elegy-core/src/lib.rs:65`
  - `crates/elegy-core/src/lib.rs:86`
- deterministic catalog outputs are already acceptance-checked and must not change
  - `crates/elegy-core/tests/bootstrap_slice.rs:29`
  - `crates/elegy-core/tests/bootstrap_slice.rs:46`

## Architecture decision

Implement a **direct crate extraction**, not a plugin framework and not a runtime rewrite.

That means:

1. create `crates/elegy-adapter-fs`
2. create `crates/elegy-adapter-http`
3. keep `elegy-runtime` as the public runtime facade and orchestration layer
4. move family-specific compose/read logic into the adapter crates
5. keep descriptor loading, policy modeling, catalog shape, runtime state shape, and CLI/core entrypoints intact

This is the smallest safe wave because it:

- aligns with the already-accepted package direction
- preserves existing public behavior
- keeps `elegy-core` and `elegy-cli` unchanged or nearly unchanged
- prepares for MCP host integration without introducing a generic extension API too early
- avoids entangling adapter extraction with OpenAPI execution design

### Explicit non-decision

Do **not** introduce any of the following in this wave:

- a dynamic adapter registry
- runtime-loaded plugins
- a new public extension API for third parties
- OpenAPI runtime composition
- host/protocol abstractions
- auth, secrets, retries, caching, or richer HTTP request modeling

Those are all larger decisions and should stay deferred.

## Smallest safe extraction wave

The safe wave now is:

1. extract local-content behavior into `elegy-adapter-fs`
2. extract plain constrained HTTP behavior into `elegy-adapter-http`
3. leave `elegy-runtime` as the single caller-facing runtime crate
4. preserve all current catalog JSON, diagnostics, and read behavior

This wave should be treated as **boundary extraction**, not feature expansion.

## What should move into `elegy-adapter-fs`

`elegy-adapter-fs` should own the current local-content families:

- `static`
- `filesystem`

That grouping is slightly broader than the crate name, but it is the right small step because both families are local, non-network, and already share the same size-bound and read-path trust boundary.

### Move these responsibilities

1. **Static resource composition**
   - current source: `crates/elegy-runtime/src/lib.rs:807`
   - move the logic that:
     - applies the default text mime type
     - binds inline descriptor source metadata
     - applies the current max-size limit contract

2. **Filesystem resource composition**
   - current source: `crates/elegy-runtime/src/lib.rs:827`
   - move the logic that:
     - resolves the configured root under the project root
     - validates the root against the allowlist
     - checks symlink policy
     - canonicalizes the candidate file path
     - prevents root escape
     - verifies file type
     - verifies size bound
     - infers default mime type

3. **Filesystem policy preparation**
   - current source: `crates/elegy-runtime/src/lib.rs:745`
   - move canonical allowed-root resolution into the FS adapter because it is family-specific preflight work

4. **Static read behavior**
   - current source: `crates/elegy-runtime/src/lib.rs:393`
   - move the logic that returns inline bytes for static resources

5. **Filesystem read-time revalidation**
   - current source: `crates/elegy-runtime/src/lib.rs:404`
   - move the logic that re-checks:
     - root existence
     - allowlisted containment
     - symlink policy
     - canonical containment
     - file type
     - size bound
     - bounded file reading

6. **Filesystem/local helpers**
   - current source: `crates/elegy-runtime/src/lib.rs:1046`
   - current source: `crates/elegy-runtime/src/lib.rs:1071`
   - move adapter-private helpers such as:
     - bounded read loop
     - mime inference for filesystem paths

### Keep the adapter interface narrow

`elegy-adapter-fs` should expose runtime-consumable data and functions only. It should **not** expose a general-purpose file API.

Recommended public surface:

```rust
pub struct FsResolvedStaticResource { /* adapter-owned resolved data */ }
pub struct FsResolvedFilesystemResource { /* adapter-owned resolved data */ }

pub enum FsReadError {
    AccessDenied { uri: String, message: String },
    InvalidResourceState { uri: String, message: String },
    Io { uri: String, message: String },
}

pub fn resolve_allowed_roots(
    project_root: &Path,
    policy: &FilesystemPolicy,
) -> Result<Vec<PathBuf>, Vec<Diagnostic>>;

pub fn compose_static_resource(
    resource: &StaticResource,
    policy: &FilesystemPolicy,
) -> FsResolvedStaticResource;

pub fn compose_filesystem_resource(
    project_root: &Path,
    allowed_roots: &[PathBuf],
    policy: &FilesystemPolicy,
    resource: &FilesystemResource,
) -> Result<FsResolvedFilesystemResource, Vec<Diagnostic>>;

pub fn read_static_resource(
    resource: &FsResolvedStaticResource,
) -> Vec<u8>;

pub fn read_filesystem_resource(
    project_root: &Path,
    allowed_roots: &[PathBuf],
    policy: &FilesystemPolicy,
    resource: &FsResolvedFilesystemResource,
) -> Result<Vec<u8>, FsReadError>;
```

The exact shape can vary, but the contract should remain **function-based and data-based**, not trait-heavy.

## What should move into `elegy-adapter-http`

`elegy-adapter-http` should own the current plain constrained HTTP family only.

### Move these responsibilities

1. **HTTP policy preflight**
   - current source: `crates/elegy-runtime/src/lib.rs:776`
   - move validation of configured `allowed_targets` into the HTTP adapter because it is family-specific validation

2. **HTTP resource composition**
   - current source: `crates/elegy-runtime/src/lib.rs:974`
   - move the logic that:
     - joins `base_url + path`
     - validates the joined target against policy
     - builds the current GET-only source metadata
     - applies the response size limit to the catalog-facing resolved state

3. **HTTP execution**
   - current source: `crates/elegy-runtime/src/lib.rs:519`
   - move the logic that:
     - re-joins the target URL
     - re-validates policy immediately before execution
     - executes a bounded GET
     - refuses redirects
     - normalizes timeout/transport/status/size failures
     - defaults mime type when upstream omits `Content-Type`

4. **HTTP client seam**
   - current source: `crates/elegy-runtime/src/lib.rs:295`
   - move:
     - `HttpRequest`
     - `HttpResponse`
     - `HttpClientError`
     - `HttpClient`
     - `ReqwestHttpClient`

5. **HTTP-specific errors**
   - current source: `crates/elegy-runtime/src/lib.rs:173`
   - move `HttpReadError` into the HTTP adapter and re-export it from `elegy-runtime` to preserve the current public API shape

6. **HTTP-local helpers**
   - current source: `crates/elegy-runtime/src/lib.rs:1025`
   - move:
     - URL join helper
     - bounded response read helper

### Keep the HTTP adapter narrow

The extracted crate is for the **implemented plain constrained HTTP slice only**.

Recommended public surface:

```rust
pub struct HttpResolvedResource { /* adapter-owned resolved data */ }

pub struct HttpRequest {
    pub target: Url,
    pub timeout_ms: u64,
}

pub struct HttpResponse {
    pub status_code: u16,
    pub content_type: Option<String>,
    pub content_length: Option<u64>,
    pub location: Option<String>,
    pub body: Box<dyn Read + Send>,
}

pub enum HttpClientError { /* unchanged */ }
pub trait HttpClient { /* unchanged */ }
pub struct ReqwestHttpClient;
pub enum HttpReadError { /* unchanged codes and messages */ }

pub fn validate_http_policy(policy: &HttpPolicy) -> Result<(), Vec<Diagnostic>>;

pub fn compose_http_resource(
    policy: &HttpPolicy,
    resource: &HttpResource,
) -> Result<HttpResolvedResource, Vec<Diagnostic>>;

pub fn read_http_resource<C: HttpClient>(
    policy: &HttpPolicy,
    resource: &HttpResolvedResource,
    client: &C,
) -> Result<HttpAdapterReadResult, HttpReadError>;
```

Again, this should stay function-based. No registry, no strategy graph, and no broader request model.

## What must remain in `elegy-runtime`

`elegy-runtime` should stay the caller-facing runtime layer.

### Keep these responsibilities in runtime

1. **Public runtime entrypoints**
   - `compose_catalog`
   - `compose_runtime_state`
   - `RuntimeState`
   - `ResourceReadResult`
   - `ReadResourceError`
   - `HttpReadMetadata`

2. **Cross-family orchestration**
   - current source: `crates/elegy-runtime/src/lib.rs:629`
   - keep:
     - iteration across normalized resources
     - duplicate ID detection
     - duplicate URI detection
     - aggregation of diagnostics across families
     - deterministic sorting of resolved entries

3. **Catalog ownership**
   - current source: `crates/elegy-runtime/src/lib.rs:49`
   - keep:
     - `Catalog`
     - `CatalogPolicySummary`
     - `CatalogResource`
     - `CatalogSource`
     - `ResourceLimits`

4. **Runtime-state ownership**
   - current source: `crates/elegy-runtime/src/lib.rs:362`
   - keep:
     - resource URI index
     - catalog assembly
     - public dispatch methods
     - storage of resolved adapter entries

5. **Cross-family error normalization**
   - keep `ReadResourceError` as the outer public error enum
   - keep `ReadResourceError::Http(HttpReadError)` so current behavior does not change
   - map FS adapter read errors into the existing generic variants

6. **Unsupported family handling**
   - current source: `crates/elegy-runtime/src/lib.rs:1087`
   - keep OpenAPI scaffold diagnostics in runtime because unsupported-family policy is a runtime-level concern, not an adapter concern

7. **Spec baseline constant**
   - current source: `crates/elegy-runtime/src/lib.rs:18`
   - keep `MCP_SPEC_BASELINE` in runtime because it is already part of the public runtime/core boundary used by higher layers

### Important restraint

Runtime should depend on both adapter crates directly and call them explicitly.

Do **not** add:

- dynamic loading
- trait-object family registries
- adapter discovery by resource family string
- a new public plugin interface

The current resource-family match in runtime is simple and honest. Keep it simple.

## Public and internal interfaces after extraction

## Public workspace interfaces

### `elegy-runtime`

Preserve the current primary runtime API:

```rust
pub fn compose_catalog(
    project: &LoadedProject,
    policy: &PolicyConfig,
) -> Result<Catalog, CompositionError>;

pub fn compose_runtime_state(
    project: &LoadedProject,
    policy: &PolicyConfig,
) -> Result<RuntimeState, CompositionError>;

impl RuntimeState {
    pub fn catalog(&self) -> &Catalog;
    pub fn read_resource(&self, uri: &str) -> Result<ResourceReadResult, ReadResourceError>;
    pub fn read_resource_with_http_client<C: HttpClient>(
        &self,
        uri: &str,
        client: &C,
    ) -> Result<ResourceReadResult, ReadResourceError>;
}
```

Also re-export the HTTP test seam from `elegy-adapter-http`:

- `HttpRequest`
- `HttpResponse`
- `HttpClient`
- `HttpClientError`
- `ReqwestHttpClient`
- `HttpReadError`

That keeps existing tests and future host callers from needing an immediate API migration.

### `elegy-core`

No new boundary is required for this extraction wave.

`elegy-core` should continue to depend only on `elegy-runtime` and preserve:

- `compose_runtime(locator)`
- `validate_descriptor_set(locator)`

If implementation needs minor import changes, keep them internal. Do not expand the public core surface for adapter extraction alone.

### Adapter crates

The adapter crates are public Cargo packages because workspace crates must compile against them, but they should be treated as **workspace-internal building blocks**, not as the primary user entrypoint.

That means:

- public enough for `elegy-runtime` to depend on them
- documented conservatively
- no promise of stable third-party extension APIs yet

## Internal runtime-to-adapter interfaces

Prefer **plain functions plus adapter-owned resolved structs**.

Recommended runtime storage model:

```text
RuntimeState
├─ catalog: Catalog
├─ project_root: PathBuf
├─ fs_allowed_roots: Vec<PathBuf>
├─ filesystem_policy: FilesystemPolicy
├─ http_policy: HttpPolicy
├─ entries: Vec<ResolvedResource>
└─ uri_index: BTreeMap<String, usize>

ResolvedResource
├─ Static(FsResolvedStaticResource)
├─ Filesystem(FsResolvedFilesystemResource)
└─ Http(HttpResolvedResource)
```

This keeps:

- runtime as the single composition/read dispatcher
- adapter state family-specific
- cross-family runtime indexing and ordering unchanged

## File-by-file implementation map

### Create

- `crates/elegy-adapter-fs/Cargo.toml`
- `crates/elegy-adapter-fs/src/lib.rs`
- `crates/elegy-adapter-http/Cargo.toml`
- `crates/elegy-adapter-http/src/lib.rs`

Recommended test files:

- `crates/elegy-adapter-fs/tests/fs_adapter.rs`
- `crates/elegy-adapter-http/tests/http_adapter.rs`

### Modify

- `Cargo.toml`
  - add the two new crates to workspace members
- `crates/elegy-runtime/Cargo.toml`
  - add dependencies on `elegy-adapter-fs` and `elegy-adapter-http`
  - remove direct `reqwest` ownership from runtime and let HTTP transport live in the HTTP adapter crate
- `crates/elegy-runtime/src/lib.rs`
  - retain public runtime types and entrypoints
  - replace in-file family logic with adapter calls
  - re-export selected HTTP adapter types
  - keep cross-family diagnostics, catalog assembly, and dispatch
- `crates/elegy-core/tests/bootstrap_slice.rs`
  - keep existing acceptance assertions unchanged to prove extraction preserved behavior
- `README.md`
  - after implementation, update the workspace shape list so it includes the new adapter crates
- `docs/architecture/README.md`
  - after implementation, update the package-shape section from “planned later crates” to “implemented bootstrap crates” for the adapter layer

## Data flow after extraction

1. `elegy-descriptor` loads and normalizes descriptors exactly as it does today.
2. `elegy-core` converts raw policy into `PolicyConfig` exactly as it does today.
3. `elegy-runtime::compose_runtime_state(...)`:
   - asks `elegy-adapter-fs` to resolve allowed roots
   - asks `elegy-adapter-http` to validate HTTP policy
   - performs duplicate ID/URI checks
   - dispatches each normalized resource by family
   - asks the relevant adapter to compose family-specific resolved state
   - builds the stable `Catalog`
   - stores adapter-owned resolved entries behind runtime-owned indexing
4. `RuntimeState::read_resource(...)`:
   - resolves the resource by URI using the runtime index
   - dispatches to the relevant adapter
   - maps adapter output into `ResourceReadResult`
   - maps adapter failures into the existing public error model
5. Higher layers (`elegy-core`, future host crate, CLI dry-run flow) continue to talk only to runtime.

## Implementation order

Implement the extraction in this order:

- [ ] 1. Create `elegy-adapter-fs` and copy static/filesystem composition and read logic into it with behavior-preserving tests
- [ ] 2. Refactor `elegy-runtime` to consume `elegy-adapter-fs` while preserving the current catalog/read API and example outputs
- [ ] 3. Create `elegy-adapter-http` and move HTTP policy validation, composition, client seam, and read logic into it with behavior-preserving tests
- [ ] 4. Refactor `elegy-runtime` to consume `elegy-adapter-http` and re-export the current HTTP public types from the adapter crate
- [ ] 5. Re-run existing acceptance coverage from `crates/elegy-core/tests/bootstrap_slice.rs` to prove catalog outputs and scaffold-only OpenAPI behavior remain unchanged
- [ ] 6. Update top-level docs only after code and tests prove the new crate layout

### Why this order

Extracting FS first is safer because:

- it has no transport dependency
- it is already the accepted baseline for the future host read/list wave
- it proves the runtime-to-adapter boundary with the simpler family first

HTTP extraction should come second because:

- it carries the reqwest dependency
- it exposes the only current adapter-specific public test seam
- it is easier to preserve behavior once the runtime already consumes one adapter crate successfully

## Critical details

### Behavior preservation

The extraction wave is only complete if these stay the same:

- current catalog JSON snapshots
- current diagnostic codes/messages
- current OpenAPI scaffold-only runtime rejection
- current HTTP error normalization
- current runtime ordering and duplicate handling

If any of those change, the wave is too broad.

### Error handling

- adapters should return diagnostics or adapter-local errors
- runtime should remain the place where cross-family failures become `CompositionError` and `ReadResourceError`
- HTTP-specific errors may stay adapter-owned and be re-exported by runtime because that preserves the existing public surface cleanly

### Testing split

After extraction:

- adapter-fs tests should own filesystem/static family behavior
- adapter-http tests should own plain constrained HTTP behavior
- runtime tests should keep only:
  - duplicate ID/URI handling
  - deterministic ordering
  - unsupported-family handling
  - end-to-end composition wiring
- core tests should continue to assert example acceptance outputs

### Avoid a premature shared utility crate

Do not create an adapter-common crate in this wave.

If both adapters each need a tiny bounded-read helper, small duplication is acceptable for now. A shared adapter base crate would be a larger architectural commitment than this extraction requires.

### Security posture

Keep all current fail-closed behavior:

- filesystem root allowlisting
- symlink restrictions
- file-size limits
- HTTP allowlist enforcement
- plaintext HTTP denial by policy
- redirect refusal
- response-size limits
- unsupported family rejection

The extraction must not weaken any of those checks.

## What remains deferred until host integration or OpenAPI support

Still defer all of the following:

- `elegy-host-mcp`
- MCP request/response mapping
- stdio transport startup
- any host-facing runtime API expansion driven only by protocol needs
- OpenAPI runtime composition
- OpenAPI document loading/execution
- parameter mapping for HTTP or OpenAPI resources
- auth headers, secrets, bearer tokens, API keys, or custom header injection
- mutation methods
- retries, redirects, caching, cookies, or streaming policies
- generic adapter/plugin architecture

## Summary

The smallest safe wave is:

- add `elegy-adapter-fs` for static/filesystem behavior
- add `elegy-adapter-http` for plain constrained HTTP behavior
- keep `elegy-runtime` as the public orchestrator and compatibility boundary
- preserve all existing behavior and defer host/OpenAPI/general-plugin decisions

That is the cleanest next step before MCP host integration and the safest way to avoid locking the project into a premature extension model.
