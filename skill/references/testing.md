# Testing & smoke driver

## Smoke driver — `scripts/smoke.sh`

Builds `dk` and exercises every command's **safe** surface, asserting exit
codes and output, without making a live/paid agent call.

```sh
bash skill/scripts/smoke.sh
```

It resolves the repo root from its own location, so it can be run from
anywhere. Prints `Result: N passed, M failed` and exits 0 only if all pass.

What it covers (19 assertions):

- `version`, `--help`, `spec --format json`, `completion bash`.
- `init` in a **temp dir** (never the repo): writes `dk.toml` + `.dk/`, records
  the agent.
- `doctor --json`: contains the `config` and `agent-reachability` checks.
- `review --help`, `check --help`.
- **Pipeline error path without a live agent**: `dk review --agent
  dk-no-such-agent-xyz <dir>` exits 1 with `DK_AGENT_NOT_FOUND` — this drives
  the pipeline up to agent invocation, so the driver needs only a Rust
  toolchain.
- `mcp serve --transport stdio`: a `tools/list` response exposes `dk.review`.

Add an assertion here whenever you add a `dk` subcommand.

## Unit + integration tests

```sh
cargo test                 # whole workspace
cargo test -p dk-core      # domain crate only
```

- `dk-core` unit tests cover config parsing/precedence, discovery, slot
  rendering, the pipeline (retry/exhaust), `agent_args` flag construction,
  init scaffolding, and the check verdict→exit mapping.
- Integration tests (`crates/dk-core/tests/fixtures.rs`) run the full review
  pipeline against **recorded** agent responses (no live agent), using fixtures
  under `specs/review/examples/`.

Tests inject agents via the `AgentRunner` trait (e.g. `CannedAgent` /
`RecordedAgent`) and pass a no-op progress callback `&|_| {}`.

## Lint

```sh
cargo clippy --all-targets
```

## Manual end-to-end (real agent, billed)

Needs an agent CLI on `PATH` (default `claude`). A real review takes ~1–2 min
and is buffered:

```sh
dk review crates/dk/src/main.rs            # scored report on stdout
dk review --output-format json -o out.json src/
```

Watch stderr for the progress spinner (TTY) or stage lines (piped).
