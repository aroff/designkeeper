---
name: designkeeper
description: >-
  DesignKeeper (`dk`) — a structured, agent-driven code-review CLI and its
  `dk-core` SDK. Use when working with the `dk` binary or `dk-core` crate:
  running or scripting `dk review`/`check`/`init`/`doctor`/`mcp serve`, editing
  `dk.toml` or `.dk/` template packs, wiring an agent, exposing review over MCP,
  or calling the review pipeline from Rust.
license: Apache-2.0
metadata:
  version: "0.1.0"
---

# DesignKeeper (`dk`)

`dk` runs structured, agent-driven design/architecture reviews. It renders a
methodology prompt, shells out to an AI agent (`claude` by default), validates
the agent's JSON against a schema (retrying on failure), and emits a scored
report or a pass/fail exit code. CodeScene inspires the UX; the domain is
design/architecture compliance.

Two layers, both covered here:

- **`dk` CLI** — the binary. Commands: `review`, `check`, `init`, `doctor`,
  `mcp serve`, plus the built-in `spec` / `completion` / `version`.
- **`dk-core` SDK** — the Rust crate holding all domain logic (config, file
  discovery, template packs, the render→agent→validate pipeline, review/check
  orchestration, init scaffolding). Framework-free; reused by the CLI and any
  future `serve` / `mcp` front-end.

Keep this file high-level; **each topic has a detailed reference** under
`references/` (paths below are relative to this skill directory).

## When to use

Load this skill when the user or codebase involves:

- The `dk` binary — running, scripting, or debugging any subcommand.
- `dk-core` — calling `run_review` / `run_check`, the `Pipeline`, or writing a
  custom `AgentRunner`.
- `dk.toml` configuration or `.dk/` template packs (methodology, schemas, report
  layout).
- Exposing `dk` over MCP, or wiring/troubleshooting the agent subprocess.

## Install

```sh
cargo install --path crates/dk      # installs `dk` into ~/.cargo/bin
# or: cargo build -p dk             # binary at target/debug/dk
```

Prerequisites: a stable Rust toolchain, and (for real reviews) an agent CLI on
`PATH` — `claude` by default. `dk doctor` verifies the environment.

## Command surface (one-liners)

| Command | Purpose | Reference |
|---------|---------|-----------|
| `dk init` | Scaffold `.dk/` + write `dk.toml` (interactive/iterative) | [references/cli-init.md](references/cli-init.md) |
| `dk review [<path>]` | Structured review → scored report (md/json) | [references/cli-review.md](references/cli-review.md) |
| `dk check [<path>]` | Review collapsed to a pass/fail exit code | [references/cli-check.md](references/cli-check.md) |
| `dk doctor` | Diagnose config, pack, installed/reachable agents | [references/cli-doctor.md](references/cli-doctor.md) |
| `dk mcp serve` | Expose `dk.review` as an MCP tool (http/stdio) | [references/cli-mcp-serve.md](references/cli-mcp-serve.md) |
| `dk spec` / `completion` / `version` | Framework built-ins | [references/cli-builtins.md](references/cli-builtins.md) |

## SDK & cross-cutting references

- **`dk-core` library API** — modules, key types, entry points, how to inject a
  custom agent: [references/sdk-dk-core.md](references/sdk-dk-core.md)
- **Configuration & template packs** — `dk.toml` schema, `.dk/` layout, prompt
  slot assembly, file discovery: [references/configuration.md](references/configuration.md)
- **Agent invocation** — how agents are spawned, per-agent flags (e.g. claude's
  `--dangerously-skip-permissions`), supported agents:
  [references/agent-invocation.md](references/agent-invocation.md)

## Quick start

```sh
cd your-project
dk init --agent claude          # scaffold .dk/ + dk.toml
dk review src/                  # scored markdown report on stdout
dk check && echo "design OK"    # gate: exit 0 = approve, 1 = changes/reject
```

## Smoke driver

`scripts/smoke.sh` builds `dk` and exercises every command's safe surface
(asserting exit codes/output) without making a live agent call:

```sh
bash skill/scripts/smoke.sh     # prints "Result: N passed, M failed"; exit 0 if all pass
```

It drives the review pipeline up to agent invocation with a deliberately
missing agent (asserting `DK_AGENT_NOT_FOUND`), so it needs only a Rust
toolchain. See [references/testing.md](references/testing.md).

## Key facts to remember

- **The agent reads files itself.** `dk` sends a target list + methodology +
  schema, not file contents.
- **`dk init` writes into the CWD** — run it in the target project, not the `dk`
  repo.
- **Reviews are slow and buffered** — no stdout until the agent finishes; a TTY
  spinner shows progress on stderr.
- **`mcp serve` exposes only `review`** as a tool (`dk.review`).
