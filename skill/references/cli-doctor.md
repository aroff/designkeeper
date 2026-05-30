# `dk doctor`

Diagnose the runtime environment: which configuration is in effect, installed
template packs, which agents are installed, and whether the configured agent is
reachable. Useful as a first step and as a CI pre-flight.

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
| `config` | Effective configuration | Resolves `dk.toml` (walking up). Prints the file in effect (or "built-in defaults") and the effective agent/model/output. **Error** if `dk.toml` is unparseable. |
| `installed-packs` | Installed template packs | Lists all packs found in `.dk/packs/` (project-local) and `~/.dk/packs/` (global). **Warning** if none are installed (built-in embedded fallbacks for `default`/`structural` are still available). |
| `installed-agents` | Installed agents | Scans `PATH` for known agent CLIs (`claude`, `codex`, `gemini`, `cursor-agent`, `copilot`, `opencode`). **Ok** if ≥1 found, **Warning** if none. |
| `agent-reachability` | Configured agent reachability | Resolves the configured agent and checks its binary is on `PATH`. **Ok** if reachable, **Error** if not, **Skipped** if `dk.toml` can't be parsed. |

## Exit code

Exits **non-zero when any check is an error** (e.g. configured agent
unreachable) — so `doctor` can gate CI. Warnings do not fail.

## Examples

```sh
dk doctor          # text table with [ok]/[warn]/[error] + remediation lines
dk doctor --json   # { "findings": [...], "summary": { ok, warnings, errors, skipped } }
dk doctor -c installed-packs
dk doctor -c agent-reachability
```

Text sample:

```
[ok]    config            | Effective configuration    | Using /path/dk.toml
[warn]  installed-packs   | Installed template packs   | No template packs installed. Built-in fallbacks available.
         → Run `dk install` to fetch and install packs.
[ok]    installed-agents  | Installed agents           | Detected: claude, codex
[error] agent-reachability| Configured agent reachability | Configured agent 'x' not found on PATH
```

## Gotchas

- Checks honor the **current working directory** (config and packs are resolved
  by walking up from it).
- `installed-packs` shows scope per pack: `project` (`.dk/packs/`) or `global`
  (`~/.dk/packs/`), with the full path.
- "installed-agents" only knows a fixed set of well-known agents; a custom agent
  won't be listed even if it's on `PATH`.
