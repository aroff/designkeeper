# `dk init`

Scaffold a `.dk/` template pack and write/update `dk.toml` in the **current
directory**. Interactive when flags are omitted; iterative on re-run.

## Synopsis

```
dk init [-a --agent <agent>] [-m --model <model>] [--template-pack <url-or-folder>]
```

| Flag | Meaning |
|------|---------|
| `-a, --agent <agent>` | Default agent key (e.g. `claude`, `codex`). |
| `-m, --model <model>` | Default model override (blank = none). |
| `--template-pack <ref>` | `default`, a local folder, or a URL (see below). |

## Interactive flow

Any flag not passed is prompted for **only when stdin is a TTY**;
non-interactive invocations (pipes, CI) silently take the default. Defaults are
seeded from an existing `dk.toml` (so re-running keeps prior values), falling
back to built-ins (`agent=claude`, model none, pack `default`).

```
$ dk init
Agent [claude]:
Model (blank for none) []: sonnet
Template pack [default]:
Created /path/dk.toml
Installed default template pack at /path/.dk
```

## What it writes

- **`dk.toml`** in the CWD: `[agent].agent`, `[agent].model`, `[templates].pack`.
  Re-running parses the existing file and overwrites only those fields,
  preserving any `[scan]` / `[output]` you added.
- **`.dk/`** template pack in the CWD: `templates/`, `schemas/`, `reports/`.

## `--template-pack` semantics

- `default` → installs the built-in default pack.
- A path to an **existing local directory** → copied verbatim into `.dk/`.
- Anything else (e.g. a remote URL) → recorded in `dk.toml [templates].pack`,
  but **not fetched**; `.dk/` is seeded with the default pack so `dk review`
  works immediately. (Remote fetch is not yet supported.)

## Examples

```sh
dk init                                         # interactive
dk init --agent claude                          # set agent, prompt for rest (TTY)
dk init --agent codex --model gpt-5 --template-pack default
dk init --template-pack ./my-pack               # copy a local pack
```

## Errors / exit

`DK_IO_ERROR` (filesystem), `DK_CONFIG_PARSE` (existing `dk.toml` unparseable).
Exit 0 on success; prints `Created`/`Updated <path>` and the pack source.

## Gotchas

- **Writes into the CWD** — run it in the *target project*, not the `dk` tool
  repo (otherwise `dk` would review itself with those settings).
- Iterative: safe to re-run to change the agent/model/pack.
- Not exposed over MCP (it's interactive).
