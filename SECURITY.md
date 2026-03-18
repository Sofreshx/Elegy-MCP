# Security Policy

## Supported state

Elegy MCP is currently in a **bootstrap stage with a live stdio host slice**.

At this stage:

- the repository baseline is established but still evolving
- the current usable slice exposes resources-only MCP hosting over stdio
- interfaces may still change before the first stable release
- security-related docs describe the current implementation boundary and reporting process, not a completed security feature set

## Reporting a vulnerability

Please do **not** open a public issue for a suspected vulnerability until maintainers have had a chance to assess it privately.

Preferred reporting path:

1. Use GitHub's private vulnerability reporting flow for this repository if it is enabled.
2. If private reporting is not available, contact the maintainers privately through GitHub before opening a public issue.

When reporting, include:

- a clear description of the issue
- affected files or components
- reproduction steps if available
- expected impact
- any proof-of-concept artifacts needed to understand the report

Please avoid sending real secrets, production credentials, or sensitive customer data.

## Response expectations

During bootstrap, response times are best-effort. Maintainers will try to:

- acknowledge receipt promptly
- confirm whether the report is in scope
- communicate remediation plans when a valid issue is confirmed

## Security posture for v1

The accepted v1 design includes a dedicated security-audit capability focused on high-confidence checks such as:

- secret leakage
- auth and identity leakage
- unsafe capability widening
- path traversal and descriptor abuse
- dangerous defaults in descriptor, runtime, and CLI-visible behavior

The currently implemented surface already applies part of that posture through conservative runtime policy:

- bounded filesystem roots and size limits
- symlink-deny defaults for filesystem resources
- allowlisted outbound HTTP targets
- credential-bearing HTTP URL rejection
- live hosting limited to resources/list-read over stdio

## Important limitation

The planned security audit is **not general malware detection**.

It is also **not**:

- endpoint protection
- a full DLP system
- a substitute for code review, dependency review, or host hardening
- proof that a descriptor, adapter, or upstream system is fully secure

The project may eventually flag malicious-looking or exfiltration-friendly patterns when they overlap with explicit audit rules, but it must not claim to detect arbitrary malware or all malicious behavior.

## Out-of-scope reports

The following are generally out of scope unless they demonstrate a concrete Elegy MCP issue:

- theoretical concerns with no plausible exploit path
- vulnerabilities only present in unsupported forks or local modifications
- reports that require disabled safeguards or unsupported future features
- general malware-detection expectations unrelated to the repository's explicit audit rules

## Coordinated disclosure

Please allow maintainers reasonable time to investigate and fix validated issues before public disclosure.
