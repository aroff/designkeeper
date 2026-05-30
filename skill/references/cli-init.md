# `dk init` and `dk install`

## `dk init`

Install all official template packs and write/update `dk.toml` in the
**current directory**. Interactive when flags are omitted; iterative on re-run.

### Synopsis

```
dk init [-a --agent <agent>] [-m --model <model>]
```

| Flag | Meaning |
|------|---------|
| `-a, --agent <agent>` | Default agent key (e.g. `claude`, `codex`). |
| `-m, --model <model>` | Default model override (blank = none). |

### What it does

1. Reads the `dk-templates.toml` manifest (walking up from CWD, or uses the
   embedded default).
2. Fetches and installs every listed pack into `.dk/packs/{name}/`. If a remote
   fetch fails, falls back to the embedded copy for built-in packs (`default`,
   `structural`).
3. Writes `dk.toml` (or updates `[agent]` in an existing one, preserving
   `[scan]` / `[output]`).

### Interactive flow

Any flag not passed is prompted **only when stdin is a TTY**; non-interactive
invocations (pipes, CI) silently take the defaults.

```
$ dk init
Agent [claude]:
Model (blank for none) []: sonnet
Created /path/dk.toml
✓ installed default → /path/.dk/packs/default
✓ installed structural → /path/.dk/packs/structural
```

### Errors / exit

`DK_IO_ERROR` (filesystem), `DK_CONFIG_PARSE` (existing `dk.toml` unparseable).
Exit 0 on success.

---

## `dk install`

Install template packs from GitHub, a URL, or a local path. Can be run
independently of `dk init`.

### Synopsis

```
dk install [-g --global] [<source>]
```

| Flag | Meaning |
|------|---------|
| `-g, --global` | Install to `~/.dk/packs/` (user-wide) instead of `.dk/packs/`. |
| `<source>` (positional) | Pack source (see below). Omit to install all official packs from `dk-templates.toml`. |

### Source formats

| Format | Example |
|--------|---------|
| GitHub shorthand | `owner/repo` |
| GitHub with version | `owner/repo@v1.2.0` |
| GitHub full URL | `https://github.com/owner/repo` |
| Direct zip URL | `https://example.com/pack.zip` |
| Local directory | `./my-pack` |

GitHub fetches use `GITHUB_TOKEN` or `GH_TOKEN` env vars when present.

### Pack layout expected in source

Each pack must contain an `aikit.toml` manifest with at least `[package] name`
and `version`, plus the standard layout:

```
templates/review.md
templates/methodology.md
reports/review.md
schemas/review-input.json
schemas/review.json
```

### Examples

```sh
dk install                               # install all official packs to .dk/packs/
dk install --global                      # install all to ~/.dk/packs/
dk install owner/repo                    # single pack from GitHub
dk install owner/repo@v2.0.0 --global   # versioned, user-global
dk install ./local-template-dir          # local path
```

### Errors / exit

`DK_REMOTE_INVALID_SOURCE`, `DK_REMOTE_FETCH_FAILED`, `DK_IO_ERROR`.
Per-pack failures are reported individually; built-in packs fall back to the
embedded copy.

---

## Pack resolution order

When `dk review --template <name>` is invoked, packs are resolved in this order:

1. `.dk/packs/{name}/` (walking up from CWD — project-local)
2. `~/.dk/packs/{name}/` (user-global)
3. Embedded fallback for `default` and `structural`
4. `DK_PACK_NOT_FOUND` error for unknown pack names

## Gotchas

- **Run `dk init` in the target project**, not the `dk` source repo.
- `dk install` is idempotent — re-installing a pack overwrites the previous version.
- Not exposed over MCP.
