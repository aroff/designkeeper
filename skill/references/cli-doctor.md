# `dk doctor`

Diagnose the runtime environment. Auto-registered by `cli-framework`'s `doctor`
feature once `dk` registers its checks.

Checks defined in `crates/dk/src/doctor.rs` (`checks()` returns the four below);
wired via `DoctorModule::new(doctor::checks())` in `crates/dk/src/main.rs`.

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
| `agent-reachability` | Configured agent reachability | Resolves the configured agent key, maps it to a binary, checks `PATH`. **Ok** if reachable, **Error** if not, **Skipped** if `dk.toml` can't be parsed. |

`KNOWN_AGENTS` (key→binary) and the `which` / `find_up` helpers live in
`doctor.rs`.

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

- Checks honor the **current working directory** (config/pack walk-up).
- "installed-agents" only knows the binaries in `KNOWN_AGENTS`; a custom agent
  won't be listed even if on `PATH`.
- The `which` helper checks `is_file()`, not the executable bit.
