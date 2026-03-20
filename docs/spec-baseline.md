---
created: 2026-03-17
updated: 2026-03-17
category: architecture
status: active
doc_kind: reference
---

# MCP Spec Baseline

> [!IMPORTANT]
> The authoritative protocol-baseline doc now lives in the main [`Elegy`](../../Elegy) monorepo at `docs/spec-baseline.md`.
> This file remains only to support closeout verification of the former standalone MCP repo.

## Purpose

Record the protocol baseline Elegy MCP is targeting during bootstrap and make upgrade expectations explicit.

## Context

The repository is greenfield, so implicit version drift would create unnecessary churn. The project needs a stable protocol target before the first implementation slice grows beyond basic scaffolding.

## Baseline

Elegy MCP is pinned to the **Model Context Protocol specification dated `2025-11-25`** for the initial implementation baseline.

This means:

- documentation should refer to `2025-11-25` when describing supported MCP behavior
- future implementation work should not silently target `latest`
- resource behavior should be aligned to the `2025-11-25` contract

The first implementation slice will still be intentionally narrower than the full spec:

- resources only
- listing and reading behavior first
- no implied support for tools, prompts, or other MCP surfaces in v1

## Upgrade policy

Spec upgrades are **explicit decisions**, not routine dependency drift.

Before changing the declared MCP baseline:

1. review the upstream MCP release and changelog
2. confirm the change is worth the migration cost
3. verify the Rust SDK and project implementation still match the required feature set
4. update docs, tests, and compatibility notes together
5. record the new baseline deliberately rather than treating it as an incidental dependency bump

Until that happens, the repository baseline remains `2025-11-25`.

## Related baselines

- Implementation direction: Rust-first
- Runtime model: runtime composition
- v1 scope: resources-only
- first resource families: HTTP/OpenAPI-backed and filesystem/static
- OSS license baseline: Apache-2.0

## References

- [Architecture overview](architecture/README.md)
- [Repository README](../README.md)