# ADR 0001: Delegate structured pipeline to aikit-sdk

## Status

Accepted

## Context

`dk` (DesignKeeper) needs a core pipeline: render a prompt template, invoke a coding agent, validate the structured response against a JSON schema, retry on failure, and render a report.

This pipeline is not dk-specific. Any tool that orchestrates "template → agent → structured output" needs the same capability. aikit-sdk already handles agent invocation, detection, and subprocess management. The question is where the template rendering, schema validation, and retry logic live.

## Decision

Delegate the entire pipeline to aikit-sdk as a new structured pipeline feature. `dk` provides template pack paths, slot values, agent configuration, and output format. aikit-sdk handles template rendering, agent invocation, JSON extraction, schema validation, retry with error feedback, and report rendering.

`dk-core` retains only: file discovery (for default targets), config resolution (dk.toml walk-up), init scaffolding (interactive prompts, template pack fetch), and command orchestration.

Implementation MUST rely on the latest versions of `cli-framework` (command registration, argument parsing, MCP server, HTTP serve infrastructure) and `aikit-sdk` (structured pipeline, agent detection) for as much of the work as possible. Custom logic outside these crates is limited to domain-specific concerns.

## Consequences

**Positive:**
- Other tools can reuse the pipeline without rebuilding it
- dk becomes genuinely thin — template packs + config + CLI shell
- Single place to fix retry logic, schema validation, and report rendering
- dk-core may not need to exist as a separate crate (potentially just a module in the CLI crate)

**Negative:**
- aikit-sdk becomes a heavier dependency
- dk is coupled to aikit-sdk's release cycle for pipeline improvements
- Template rendering and JSON Schema validation are new dependencies for aikit-sdk (`jsonschema` crate)
