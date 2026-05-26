# `dk doctor`

Diagnose the runtime environment: which configuration is in effect, the
template-pack status, which agents are installed, and whether the configured
agent is reachable. Useful as a first step (and as a CI gate).

## Synopsis

```
dk doctor [-j --json] [-c --check <id>]
```

| Flag | Meaning |
|------|---------|
| `-j, --json` | Emit structured JSON (findings + summary) instead of the text table. |
| `-c, --check <id>` | Run only the check with this id. |

## Checks

| id | Title | What it reports |
|----|-------|-----------------|
| `config` | Effective configuration | Resolves `dk.toml` (walking up). Prints the file in effect (or "built-in defaults") and the effective agent/model/output/pack. **Error** if `dk.toml` is unparseable. |
| `template-pack` | Template pack | Finds a `.dk/` (walking up) containing `templates/review.md`. **Ok** if installed, **Warning** if absent (using embedded defaults). |
| `installed-agents` | Installed agents | Scans `PATH` for known agent CLIs (`claude`, `codex`, `gemini`, `cursor-agent`, `copilot`, `opencode`). **Ok** if ≥1 found, **Warning** if none. |
| `agent-reachability` | Configured agent reachability | Resolves the configured agent and checks its binary is on `PATH`. **Ok** if reachable, **Error** if not, **Skipped** if `dk.toml` can't be parsed. |

## Exit code

Exits **non-zero when any check is an error** (e.g. configured agent
unreachable) — so `doctor` can gate CI. Warnings do not fail.

## Examples

```sh
dk doctor          # text table with [ok]/[warn]/[error] + remediation lines
dk doctor --json   # { "findings": [...], "summary": { ok, warnings, errors, skipped } }
dk doctor -c agent-reachability
```

Text sample:

```
[ok]    config               | Effective configuration | Using /path/dk.toml
[warn]  template-pack        | Template pack           | No .dk/ template pack found; using embedded defaults
         → Run `dk init` to install an editable template pack.
[ok]    installed-agents     | Installed agents        | Detected: claude, codex
[error] agent-reachability   | ...                     | Configured agent 'x' not found on PATH
```

## Gotchas

- Checks honor the **current working directory** (config and pack are resolved
  by walking up from it).
- "installed-agents" only knows a fixed set of well-known agents; a custom agent
  won't be listed even if it's on `PATH`.
- Reachability tests that the agent's file is present on `PATH`, not that it's
  executable.
