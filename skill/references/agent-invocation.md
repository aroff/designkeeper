# Agent invocation

How `dk review` / `dk check` turn a prompt into an agent call, and the
per-agent flags that matter — useful when troubleshooting a hang or
`DK_AGENT_NOT_FOUND`.

## The model

`dk` does **not** stream a context payload. It builds a task directive (the
prompt) and runs the configured agent binary with that prompt, using the
working dir as the agent's cwd; the agent reads the repository files itself.
`dk` captures the agent's stdout and gives it no interactive stdin. A missing
binary surfaces as `DK_AGENT_NOT_FOUND`; a non-zero agent exit as
`DK_PIPELINE_ERROR`.

## What `dk` runs

```
<agent> [--model <m>] [--dangerously-skip-permissions (claude only)] -p <prompt>
```

- **`--model` (long form)**: `claude` rejects `-m`; `codex`/`gemini` accept both
  `-m` and `--model`. Using `--model` works across all of them. (A `-m` here was
  the original bug that broke `claude`.)
- **`--dangerously-skip-permissions` (claude only)**: added only for `claude`.
  In `-p` (print) mode with no TTY, claude otherwise **blocks forever** on
  tool-permission approval the moment the agent tries to read a file — so a
  review hangs. Bypassing permissions lets it run unattended. Other agents
  don't get (or need) this flag.
- **`-p <prompt>`**: print / non-interactive mode; the prompt is the positional
  argument.

A configured agent given as a full path (e.g. `/usr/local/bin/claude`) is still
recognized as `claude` by its file name, so the claude-specific flag applies.

## Supported agents

`dk` knows these agents: `claude`, `codex`, `gemini`, `cursor-agent`,
`copilot`, `opencode`. Any agent works as long as its binary is on `PATH` and
accepts `--model` / `-p`; an unknown agent key is used directly as the binary
name (without the claude-specific flag).

## Progress

During the agent call `dk` shows progress on **stderr**: a spinner with elapsed
seconds and the attempt number (e.g. `attempt 1/3`) on a TTY, or plain stage
lines when stderr is piped. There are up to 3 attempts (2 retries after a
validation failure). SDK consumers can subscribe to the same events — see
[sdk-dk-core.md](sdk-dk-core.md).

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
