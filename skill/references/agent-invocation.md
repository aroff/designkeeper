# Agent invocation

How `dk` turns a rendered prompt into an agent call, and the per-agent flags
that matter. Source: `crates/dk-core/src/pipeline.rs` (`SubprocessAgent`,
`agent_args`, `agent_basename`).

## The model

`dk` does **not** stream a context payload. It builds a task directive (prompt)
and runs the configured agent binary with that prompt; the agent reads the
repository files itself, in the working dir as its cwd.

```rust
pub struct SubprocessAgent { pub agent: String, pub model: Option<String> }
```

`run()` does `Command::new(agent).current_dir(working_dir).args(agent_args(...))`,
captures stdout via `cmd.output()` (stdin is null — no interactive input), and
returns stdout. A non-zero exit becomes `AgentFailed`; a missing binary becomes
`AgentNotFound`.

## Argv construction (`agent_args`)

```
<agent> [--model <m>] [--dangerously-skip-permissions (claude only)] -p <prompt>
```

- **`--model` (long form)**: `claude` rejects `-m`; `codex`/`gemini` accept both
  `-m` and `--model`. Using `--model` works across all of them. (A `-m` here was
  the original bug that broke `claude`.)
- **`--dangerously-skip-permissions` (claude only)**: gated on
  `agent_basename(agent) == "claude"`. In `-p` (print) mode with no TTY, claude
  otherwise **blocks forever** on tool-permission approval the moment the agent
  tries to read a file — so a review hangs. Bypassing permissions lets it run
  unattended. Other agents don't get (or need) this flag.
- **`-p <prompt>`**: print/non-interactive mode; the prompt is the positional
  argument.

`agent_basename` drops any directory path, so `/usr/local/bin/claude` still
matches `claude`.

## Supported agents

`KNOWN_AGENTS` (in `crates/dk/src/doctor.rs`) maps keys → binaries: `claude`,
`codex`, `gemini`, `cursor-agent`, `copilot`, `opencode`. Any key works as long
as the binary is on `PATH` and accepts `--model`/`-p`; unknown keys fall back to
the key as the binary name (no claude-specific flag).

## Progress events

`Pipeline::run` emits `dk_core::pipeline::Progress` on the calling thread:

```rust
pub enum Progress {
    AgentRunning { attempt: u32, total: u32 },     // before each agent call
    Validating   { attempt: u32, total: u32 },     // agent responded; validating
    Retrying     { attempt: u32, total: u32, errors: usize }, // about to retry
}
```

`total` is `max_retries + 1` (3). The CLI renders these as a TTY spinner with
elapsed time, or plain stage lines when stderr is piped.

## Custom agents (SDK)

Implement `AgentRunner` and pass it to `run_review_with_agent` /
`run_check_with_agent` — handy for tests (canned/recorded responses) or wrapping
a non-CLI agent. See [sdk-dk-core.md](sdk-dk-core.md).

```rust
pub trait AgentRunner {
    fn run(&self, prompt: &str, working_dir: &Path) -> Result<String, PipelineError>;
}
```

## Errors

- `DK_AGENT_NOT_FOUND` — binary not on `PATH`. Fix: install it, or set a
  different agent (`-a` / `dk.toml [agent].agent`). Check with `dk doctor`.
- `DK_PIPELINE_ERROR` — agent exited non-zero, or output failed validation after
  all retries (e.g. wrong/missing ```json block, schema violations).

## Gotchas

- A real call is slow (often 1–2 min) and buffered (no streaming).
- The agent must emit a single ```json block validating against
  `schemas/review.json`; the pipeline extracts the first such block.
- The skip-permissions behavior is claude-specific and intentional — read-only
  reviews in a trusted local repo. Other agents are invoked without it.
