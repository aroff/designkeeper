---
name: run-designkeeper
description: Build, run, and smoke-test the DesignKeeper `dk` CLI — a structured, agent-driven code-review tool. Use when asked to run, build, start, test, screenshot, or drive `dk`, or to exercise its commands (init, review, check, doctor, mcp serve, spec, completion).
---

# Run DesignKeeper (`dk`)

`dk` is a CLI (Rust workspace; the `dk` binary is a thin shell over `dk-core`).
It runs structured, agent-driven design/architecture reviews: it renders a
methodology prompt, shells out to an AI agent (`claude` by default), validates
the agent's JSON against a schema, and emits a report or a pass/fail exit code.

There is no GUI. The way to drive it is the **smoke driver**, which builds the
binary and exercises every command's safe surface (asserting exit codes and
output) without making a live/paid agent call.

> Paths below are relative to the repo root (the dir with `Cargo.toml`).
> The driver lives at `skill/smoke.sh`.

## Prerequisites

A Rust toolchain (stable) is all the driver needs:

```sh
cargo --version    # any recent stable
```

To run a *real* `dk review` / `dk check` you additionally need an agent CLI on
`PATH` — `claude` by default. The driver does **not** require one (it tests the
pipeline with a deliberately-missing agent). Verify your environment with
`dk doctor`.

## Build

```sh
cargo build -p dk            # binary at target/debug/dk
```

The driver uses `target/debug/dk` directly. The bare `dk` in the command
reference below assumes it's on `PATH` — install the current build with:

```sh
cargo install --path crates/dk   # puts `dk` in ~/.cargo/bin
```

## Run — the driver (agent path)

```sh
bash skill/smoke.sh
```

Builds `dk`, then runs 19 assertions across all commands and prints
`Result: N passed, M failed`. Exits 0 only if everything passed. `init` is run
inside a temp dir (it writes `dk.toml` + `.dk/` into the CWD), so the driver
never pollutes the repo. Expected tail:

```
== Result: 19 passed, 0 failed
```

Add a command to `skill/smoke.sh` whenever you add a `dk` subcommand.

## Command reference

Each command verified this session. `dk --help` lists them all.

### `dk init` — scaffold a project
Writes `dk.toml` and a `.dk/` template pack into the **current directory**;
interactive when flags are omitted, iterative on re-run. Handler:
`crates/dk/src/main.rs` (`run_init_cmd`) → `crates/dk-core/src/init.rs`.

```sh
dk init --agent codex --model gpt-5 --template-pack default
```

### `dk review [<path>]` — structured review
Runs the review pipeline and prints a scored report (markdown default; `--output-format json`).
With no `<path>` it auto-discovers source files; with a path it reviews exactly that.
Handler: `crates/dk-core/src/review.rs`. Flags: `-a/--agent`, `-m/--model`,
`--output-format`, `--output-file`, `--title`, `--description`, `--base-ref`,
`--head-ref`, `--focus <area>` (repeatable), `--max-findings` (1–50, default 25).
During the (slow) agent call it shows a progress spinner on a TTY, or stage
lines (`dk: Reviewing with claude…`) when stderr is piped.

```sh
dk review --help
# Pipeline up to agent invocation, no live agent (verified error path):
dk review --agent dk-no-such-agent-xyz .   # -> error [DK_AGENT_NOT_FOUND], exit 1
```
With an installed agent, `dk review src/` produces the report.

### `dk check [<path>]` — pass/fail gate
Same pipeline, collapsed to an exit code: `approve`/`approve_with_comments` → 0,
`request_changes`/`reject` or pipeline error → 1. Silent on stdout when passing;
`-v/--verbose` prints the full report. Handler: `crates/dk-core/src/check.rs`.

```sh
dk check --help
```

### `dk doctor` — environment diagnostics
Four checks: effective config, template-pack status, installed agents, configured-agent
reachability. `--json` for machine output; exits non-zero if any check errors.
Handler: `crates/dk/src/doctor.rs`.

```sh
dk doctor          # text table
dk doctor --json   # structured findings + summary
```

### `dk mcp serve` — expose `dk` over MCP
Auto-registered by `cli-framework`'s `mcp-server` feature. HTTP (`--transport http`,
default) or stdio. Only commands flagged `expose_mcp` are surfaced — currently just
`review` (tool name `dk.review`).

```sh
# List the exposed tools over stdio (prints a tools/list response containing dk.review):
printf '%s\n' \
  '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"x","version":"1"}}}' \
  '{"jsonrpc":"2.0","method":"notifications/initialized"}' \
  '{"jsonrpc":"2.0","id":2,"method":"tools/list"}' \
  | timeout 8 dk mcp serve --transport stdio
```

### `dk spec` / `dk completion` — framework built-ins
`spec` exports the command surface (`--format json|yaml|markdown`); `completion`
emits a shell completion stub. Both come from `cli-framework`.

```sh
dk spec --format json
dk completion bash
```

## Gotchas

- **`dk init` writes into the CWD**, not a fixed location — `dk.toml` + `.dk/`
  land wherever you run it. Run it in the *target project*, never in the `dk`
  tool repo. The driver always uses a temp dir for this reason.
- **`review`/`check` shell out to an agent** as `<agent> [--model m] -p <prompt>`
  (default `claude`; `crates/dk-core/src/pipeline.rs`). For `claude` it adds
  `--dangerously-skip-permissions`, because in `-p` mode with no TTY claude
  otherwise blocks forever on tool-permission approval the moment it reads a
  file. The agent reads the source files itself — `dk` only sends a target list
  + methodology + schema. No agent on `PATH` ⇒ `DK_AGENT_NOT_FOUND`, exit 1.
- **No `.dk/` needed to review.** The default template pack is embedded in the
  binary and materialized to a temp dir when `.dk/` is absent
  (`crates/dk/src/main.rs`, `ensure_template_dir`).
- **`mcp serve` exposes only `review`.** `init`/`doctor`/`check` are CLI-only by
  design (the app sets `McpToolExportPolicy::ExposeMcpOnly` and only `review` has
  `expose_mcp: true`).
- **`--template-pack` copies a *local folder*** verbatim; a remote URL is recorded
  in `dk.toml` but not fetched (embedded defaults are seeded instead).
- **`dk check` is quiet on success** — no stdout, exit 0. Failures print a
  findings summary to **stderr**.
- **A real review is slow and silent on stdout** — output is buffered until the
  agent finishes (often 1–2 min). Progress goes to stderr; the report appears
  all at once at the end.

## Troubleshooting

- `error [DK_AGENT_NOT_FOUND]: configured agent not found: <x>` — the agent CLI
  isn't on `PATH`. Install it, or pick another with `-a <agent>` / `[agent].agent`
  in `dk.toml`. Confirm with `dk doctor` (the `agent-reachability` check).
- `dk doctor` **exits 1** — at least one check is an error (e.g. configured agent
  unreachable). This is intentional so `doctor` can gate CI. `--json` shows which.
- `cargo build -p dk` pulls `cli-framework` + `aikit-sdk` (a git dependency) on
  first build, so the initial compile fetches crates and is slow; subsequent
  builds are fast.
