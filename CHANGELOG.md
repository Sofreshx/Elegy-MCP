# Changelog



All notable changes to this project will be documented in this file.



The format is based on Keep a Changelog principles, adapted for the bootstrap stage.



## [Unreleased]



### Added



- bootstrap-stage Rust workspace with foundational crates: `elegy-descriptor`, `elegy-policy`, `elegy-runtime`, `elegy-core`, and `elegy-cli`

- bootstrap CLI support for `validate config`, `validate runtime`, `inspect resources`, and `run --dry-run`

- `elegy-host-mcp` as the first live MCP host crate

- live stdio MCP hosting for the current resources-only surface via `elegy run`

- filesystem/static bootstrap example project with expected catalog output

- plain constrained HTTP bootstrap runtime support within the current policy model

- plain constrained HTTP bootstrap example project with expected catalog output

- OpenAPI placeholder example project for descriptor and scope alignment

- bootstrap-stage project README with honest scope and status

- contributor guide for the pre-implementation repository phase

- security policy with reporting guidance and audit-boundary language

- Apache-2.0 licensing baseline files

- changelog, code of conduct, issue templates, PR template, and CODEOWNERS stub

- architecture and spec-baseline documentation entrypoints

- examples overview describing how examples should function as acceptance inputs



### Changed

- repository entered closeout verification; authoritative OSS posture now lives in the main Elegy monorepo


- refreshed repository docs to describe the implemented bootstrap workspace instead of a purely planned shape

- clarified that filesystem/static is the working bootstrap runtime path today

- clarified that plain constrained HTTP is now part of the working bootstrap runtime slice

- clarified that OpenAPI remains represented by descriptor, policy, and example scaffolding rather than a finished runtime composition flow

- clarified that live stdio hosting now exists while audit crates and broader transport support remain deferred

- tightened bootstrap documentation language around contributor expectations and the still-deferred audit/OpenAPI slices

