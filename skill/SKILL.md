---
name: designkeeper
description: >-
  DesignKeeper (`dk`) — a structured, agent-driven code-review CLI. Use when
  running or scripting `dk review`, `dk check`, `dk init`, `dk install`,
  `dk doctor`, or `dk mcp serve`; managing template packs; editing `dk.toml`;
  or wiring an agent.
license: Apache-2.0
metadata:
  version: "0.2.0"
---

# DesignKeeper (`dk`)

`dk` runs structured, agent-driven code reviews. It renders a methodology
prompt from a **template pack**, shells out to an AI agent (`claude` by
default), validates the agent's JSON against a schema (retrying on failure),
and emits a scored report or a pass/fail exit code.

## When to use

Load this skill when the user involves:

- Running, scripting, or debugging any `dk` subcommand.
- Installing, switching, or customizing template packs.
- Configuring `dk.toml` or the `.dk/packs/` directory.
- Exposing `dk` over MCP or troubleshooting the agent.

## Install

```sh
# macOS
brew install aroff/cli/designkeeper

# Windows
scoop bucket add aroff https://github.com/aroff/scoop-bucket
scoop install designkeeper

# Cargo
cargo install --git https://github.com/aroff/designkeeper --bin dk
```

Requires an agent CLI on `PATH` (`claude` by default). Run `dk doctor` to verify.

## Command surface

| Command | Purpose | Reference |
|---------|---------|-----------|
| `dk init` | Install packs from `dk-templates.toml` + write `dk.toml` | [references/cli-init.md](references/cli-init.md) |
| `dk install [--global] [<source>]` | Install a pack from GitHub, URL, or local path | [references/cli-init.md](references/cli-init.md) |
| `dk review --template <name> [<path>]` | Structured review → scored report (md/json) | [references/cli-review.md](references/cli-review.md) |
| `dk check --template <name> [<path>]` | Review collapsed to a pass/fail exit code | [references/cli-check.md](references/cli-check.md) |
| `dk doctor` | Diagnose config, installed packs, agent availability | [references/cli-doctor.md](references/cli-doctor.md) |
| `dk mcp serve` | Expose `dk.review` as an MCP tool (http/stdio) | [references/cli-mcp-serve.md](references/cli-mcp-serve.md) |
| `dk spec` / `completion` / `version` | Built-in framework commands | [references/cli-builtins.md](references/cli-builtins.md) |

## Cross-cutting references

- **Configuration & template packs** — `dk.toml`, `.dk/packs/` layout, pack
  resolution order, prompt slots, file discovery:
  [references/configuration.md](references/configuration.md)
- **Agent invocation** — how agents are spawned, supported agents, per-agent flags:
  [references/agent-invocation.md](references/agent-invocation.md)

## Quick start

```sh
cd your-project
dk init --agent claude                      # install packs + write dk.toml
dk review --template default src/           # Google eng-practices review
dk review --template structural src/        # structural quality review
dk check --template default && echo "OK"    # gate: exit 0 = approve
```

## Template packs

`--template <name>` is **required** on every `dk review` and `dk check` call.
Built-in packs (`default`, `structural`) work out of the box without `dk install`.

| Pack | Rubric | Dimensions |
|------|--------|-----------|
| `default` | Google Engineering Practices | 13 (design, tests, naming, complexity, …) |
| `structural` | Structure · Complexity · Expressiveness | 9 sub-dimensions |

Pack resolution order: `.dk/packs/{name}/` (project-local) →
`~/.dk/packs/{name}/` (global) → embedded fallback.

See [references/configuration.md](references/configuration.md) for layout and
customization details.

## Key facts

- **`--template` is required** — omitting it on `review` or `check` is an error.
- **The agent reads files itself.** `dk` sends a target list + methodology + schema, not file contents.
- **`dk init` writes into the CWD** — run it in the target project, not the `dk` repo.
- **Reviews are slow and buffered** — no stdout until the agent finishes; a TTY spinner shows progress on stderr.
- **`mcp serve` exposes only `review`** as a tool (`dk.review`).
