# Built-in commands: `spec`, `completion`, `version`

Utility commands that ship with `dk` and appear in `dk --help`.

## `dk spec`

Export the CLI command surface (the registered commands, args, metadata) as a
machine-readable document — useful for tooling, docs, or LLM command resolution.

```
dk spec [--format json|yaml|markdown] [--output <path>] [--include-hidden]
```

| Flag | Default | Meaning |
|------|---------|---------|
| `--format` | `json` | `json`, `yaml`, or `markdown`. |
| `--output <path>` | stdout | Write to a file instead. |
| `--include-hidden` | off | Include commands marked `hidden: true`. |

```sh
dk spec --format json | jq '.commands[].id'
dk spec --format markdown --output COMMANDS.md
```

The JSON has a `schemaVersion`, an `app` block (`name`, `version`), and a
`commands` array (each with `path`, `id`, `summary`, `syntax`, `category`,
`args`, …).

## `dk completion`

Emit a shell completion stub for the top-level subcommands.

```
dk completion <shell>
```

`<shell>` is one of `bash`, `zsh`, `fish`, `powershell`, `pwsh`.

```sh
dk completion bash > /etc/bash_completion.d/dk     # or source it
dk completion zsh  > "${fpath[1]}/_dk"
```

The bash stub defines `_dk()` completing `check completion doctor init mcp
review spec` and registers `complete -F _dk dk`.

## `dk version`

```sh
dk version        # -> "dk 0.1.0"
dk --version      # same
```

## `dk mcp install` / `dk mcp register`

Register `dk` as an MCP server in a supported agent's config file.
`dk mcp register` is an alias for `dk mcp install` with identical flags.

```
dk mcp install [--agent <AGENT>] [--scope <SCOPE>] [--name <NAME>]
               [--project <PATH>] [--stdio | --url <URL>]
               [--host <HOST>] [--port <PORT>] [--path <PATH>]
               [--arg <ARG>]... [--env <KEY=VALUE>]...
               [--header <KEY:VALUE>]... [--overwrite] [--dry-run]
```

| Flag | Default | Meaning |
|------|---------|---------|
| `--agent` | `claude` | Target agent: `claude`, `cursor`, `gemini`, `copilot`, `vscode`, `opencode`, `codex`. |
| `--scope` | `project` | `project` (CWD config file) or `global` (user home config file). |
| `--name` | `dk` | Server name key in the config file. |
| `--project` | CWD | Project root for project-scope config resolution. |
| `--stdio` | — | Register as a stdio server (recommended). |
| `--url` | — | Register as an HTTP server at the given URL. |
| `--host` | `127.0.0.1` | HTTP host (ignored with `--url` or `--stdio`). |
| `--port` | `8080` | HTTP port (ignored with `--url` or `--stdio`). |
| `--path` | `/mcp` | HTTP path (ignored with `--url` or `--stdio`). |
| `--arg` | — | Override default stdio args (repeatable). |
| `--env` | — | Pass env vars to stdio server as `KEY=VALUE` (repeatable). |
| `--header` | — | HTTP auth headers as `KEY:VALUE` (repeatable; ignored with `--stdio`). |
| `--overwrite` | off | Replace an existing entry with the same `--name`. |
| `--dry-run` | off | Print the intended config to stdout; do not write any file. |

### Examples

```sh
# Cursor — project scope (.cursor/mcp.json in CWD)
dk mcp install --agent cursor --stdio

# Cursor — global scope (~/.cursor/mcp.json)
dk mcp install --agent cursor --stdio --scope global --overwrite

# Claude Code — project scope (.mcp.json in CWD)
dk mcp install --agent claude --stdio

# Gemini, Copilot/VS Code, OpenCode, Codex
dk mcp install --agent gemini   --stdio
dk mcp install --agent copilot  --stdio
dk mcp install --agent opencode --stdio
dk mcp install --agent codex    --stdio

# HTTP mode
dk mcp install --agent cursor --url http://127.0.0.1:8080/mcp

# Preview without writing
dk mcp install --agent cursor --stdio --dry-run
```

### Error codes

| Code | Trigger |
|------|---------|
| `[E010]` | `current_exe()` cannot be resolved (stdio mode). |
| `[E011]` | Write failed, server name already exists without `--overwrite`, or malformed `--header`/`--env`. |

---

## `dk mcp list`

Print a table of all supported MCP agent targets and their config file paths.

```
dk mcp list
```

No flags. Output is plain text — one row per supported agent:

```
AGENT          NAME     PROJECT PATH
claude         dk       .mcp.json
cursor-agent   dk       .cursor/mcp.json
gemini         dk       .gemini/settings.json
copilot        dk       .vscode/mcp.json
opencode       dk       opencode.json
codex          dk       .codex/config.toml
```

---

## `dk help`

`dk help` / `dk <cmd> --help` print usage from the registered `CommandSpec`s.
