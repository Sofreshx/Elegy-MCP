# Contributing to Elegy MCP

> [!IMPORTANT]
> The authoritative contributor guidance now lives in the main [`Elegy`](../Elegy) monorepo.
> Keep this repository only for closeout verification while remaining migration checks finish.

Thanks for your interest in contributing.

This repository is at the **bootstrap stage**, so the most valuable contributions right now are:

- keeping docs aligned with reality
- improving clarity around architecture and scope
- identifying gaps in contributor ergonomics
- hardening the first real Rust implementation slice

## First principles

Please keep these project rules in mind:

1. **Be honest about current status.** Do not document commands, examples, or capabilities that do not exist yet.
2. **Respect the accepted direction.** The locked baseline is Rust-first, runtime composition, resources-only v1, Apache-2.0, and MCP spec baseline `2025-11-25`.
3. **Keep v1 intentionally narrow.** The first resource families are HTTP/OpenAPI-backed and filesystem/static only.
4. **Prefer safe defaults.** Validation, policy, and audit concerns are core product behavior, not extras.
5. **Do not widen scope casually.** Changes that affect protocol scope, resource families, trust boundaries, or packaging direction should start with an issue or design discussion.

## Before you start

If you want to contribute code or structure changes, first review:

- [README.md](README.md)
- [docs/architecture/README.md](docs/architecture/README.md)
- [docs/spec-baseline.md](docs/spec-baseline.md)
- [SECURITY.md](SECURITY.md)

For larger changes, please open an issue or draft PR early so maintainers can confirm the work still matches the accepted bootstrap plan.

## What to work on now

Good contributions right now include:

- documentation corrections
- wording improvements that remove ambiguity
- cross-link fixes
- issue-template improvements
- contributor-experience suggestions that do not conflict with the locked direction

## Local verification

Contributors are expected to run the standard local checks before opening a PR:

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-targets --all-features
```

If you change behavior or public docs, make sure your verification scope still matches the current live surface.

## Pull request expectations

Please keep pull requests:

- small enough to review
- explicit about user-visible impact
- explicit about whether the change is documentation-only or implementation work
- updated with docs when behavior changes

Every PR should answer:

- What changed?
- Why is the change needed now?
- Does it change the accepted v1 scope or architecture?
- What follow-up work, if any, remains?

Use the PR template and fill it in completely.

## Scope guardrails

These items are intentionally out of scope for the first release unless the project direction is changed explicitly:

- tools, prompts, or other non-resource MCP surfaces
- build-time generation as the primary operating model
- write-capable adapters
- broad plugin/extensibility promises
- hosted platform features
- generalized malware-detection claims

## Security-related contributions

If you believe you found a security issue, please follow [SECURITY.md](SECURITY.md) instead of opening a public bug report first.

## Communication and conduct

Please follow [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md) in all project spaces.