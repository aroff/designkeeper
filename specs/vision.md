# DesignKeeper (`dk`) — Vision

An architecture and design compliance CLI tool built on `cli-framework` and `aikit-sdk`. CodeScene is the UX inspiration (commands, output style), not the domain. Implementations MUST rely on the latest versions of `cli-framework` (command registration, argument parsing, MCP server) and `aikit-sdk` (structured pipeline: template rendering, agent invocation, schema validation, retry, report rendering) for as much of the work as possible.

## Core pipeline

```
command + args → template render → agent → structured output → report
```

`dk` is a thin orchestration layer. Template rendering, agent invocation, JSON schema validation, retry, and report rendering are delegated to aikit-sdk's structured pipeline feature. See `specs/aikit-sdk-structured-pipeline.md` for the handover spec.

## Crates

| Crate | Role |
|-------|------|
| `dk` (CLI) | Thin `cli-framework` shell: command registration, argument parsing, I/O formatting |
| `dk-core` | Domain layer: file discovery, config resolution (`dk.toml` walk-up), init scaffolding, command orchestration |

## Commands

```
dk init  [-a --agent <agent>] [-m --model <model>] [--template-pack <url-or-folder>]
dk review [<path>]           [-a --agent] [-m --model] [--output-format] [--output-file]
                              [--title <text>] [--description <file|text>] [--base-ref <ref>] [--head-ref <ref>]
                              [--focus <area>]... [--max-findings <n>]
dk drift  [<path>] [--since <ref>] [-a --agent] [-m --model] [--output-format] [--output-file]
dk check  [<path>]           [-a --agent] [-m --model] [--output-format] [--output-file] [--verbose]
dk doctor
dk serve  --with-api [--host <host>] [--port <port>]
dk mcp    [<flags>]
```

### `dk init`

Interactive setup. Prompts for each parameter not provided on the command line. Uses aikit-sdk agent detection to present a closed list of installed agents. Fetches template pack from a GitHub URL (default: dk repo) or local folder into `.dk/`. Iterative — re-running updates values in place.

### `dk review [<path>]`

Evaluates code against a structured review rubric (default: Google eng-practices, 13 dimensions, 0–10 scores). Produces per-dimension scores, a verdict (`approve`, `approve_with_comments`, `request_changes`, `reject`), actionable findings, and suggested next steps. Accepts PR/CL change context (`--title`, `--description`, `--base-ref`, `--head-ref`) and optional focus areas. The rubric methodology ships as a default template (`templates/methodology.md`) that users can edit or replace via the template pack. Works on any directory; no git required. Spec pack: `specs/review/` (schemas, prompt/report templates, examples).

### `dk drift [<path>] [--since <ref>]`

Evaluates architectural trajectory over time. Detects degradation patterns, boundary erosion, coupling growth. The agent compares states using git commands on the working directory. Default `--since` is `HEAD~1` (previous commit). Requires a git repository.

### `dk check [<path>]`

Pass/fail gate for CI and pre-commit hooks. Runs `dk review` internally and maps the verdict to exit codes: `approve` or `approve_with_comments` → exit 0, `request_changes` or `reject` → exit 1. Always outputs a findings summary on failure. `--verbose` produces the full scored report.

### `dk doctor`

Reports on the runtime environment: installed agents, effective configuration file (after walking up directories), template pack status, agent reachability.

### `dk serve --with-api`

HTTP server exposing a JSON API (`POST /review`, `POST /drift`, `POST /check`). Options: `--host` (default: `127.0.0.1`), `--port` (default: `8080`).

### `dk mcp`

MCP server exposing `dk` commands as MCP tools. Uses cli-framework's `mcp-server` feature. Future: stdio transport mode.

## Agent model

`dk` invokes a full-featured coding agent (Claude Code, Opencode, Cursor, etc.) via `aikit-sdk`. The agent has filesystem access and works freely on the working directory. `dk` constructs a task directive from the template pack; the agent reads files, explores context, and produces a structured response independently. The agent's own LLM dependencies are managed by the agent runtime, not by `dk`.

### Agent / model resolution

1. CLI flags `-a`/`-m` (highest priority)
2. `dk.toml` `[agent]` section
3. Built-in defaults

## Template packs

Installed by `dk init` into `.dk/`. Structure:

```
.dk/
├── templates/
│   ├── review.md          # prompt template with {{slots}}
│   ├── methodology.md     # review rubric (default: Google eng-practices); user-editable
│   └── drift.md
├── schemas/
│   ├── review-input.json  # JSON Schema for CLI/API input
│   ├── review.json        # JSON Schema for agent output
│   └── drift.json
├── reports/
│   ├── review.md          # report layout template
│   └── drift.md
└── dk.toml                # control file
```

Users may edit any file to customize behavior. Alternative packs can be fetched from different GitHub URLs via `dk init --template-pack`.

### Prompt assembly

`dk` constructs a task directive using named slots (e.g. `{{methodology}}`, `{{target}}`, `{{output_schema}}`). The template pack defines the layout; `dk` fills the slots programmatically. The agent reads source files and project documentation from the filesystem itself.

## Control file (`dk.toml`)

Optional. `dk` walks up directories to find it. Created by `dk init` or manually.

```toml
[scan]
extensions = [".rs", ".ts"]
ignore_patterns = ["vendor/", "generated/"]

[output]
format = "markdown"

[agent]
agent = "claude"
model = ""

[templates]
pack = "default"
```

## File discovery

`dk` uses the `ignore` crate (respects .gitignore) and `globset` for pattern matching. Used to determine default analysis targets when the user doesn't specify a path. The agent reads actual file content independently.

Default recognized extensions: `.rs`, `.ts`, `.tsx`, `.js`, `.jsx`, `.py`, `.go`, `.java`, `.kt`, `.c`, `.cpp`, `.h`, `.hpp`, `.rb`, `.ex`, `.exs`, `.scala`, `.swift`, `.cs`. Extendable via control file.

## Response pipeline

Delegated to aikit-sdk's structured pipeline:

1. Render template with slots → prompt
2. Send prompt to agent → raw response
3. Extract JSON from first ```` ```json ```` code block in agent response
4. Validate JSON against schema from template pack
5. On validation failure: retry (up to 2 attempts) with augmented prompt including error details
6. Render report: markdown (via report template) or JSON (`--output-format`)
7. Output: stdout by default, `--output-file <path>` to write to disk

## Git requirements

`review` and `check` work on any directory. Only `drift` requires a git repository.

## Inspiration: CodeScene CLI reference

These CodeScene command examples informed the UX design:

```
$ cs delta                       # Analyse all non-committed changes
$ cs delta main                  # Analyse changes against the main branch
$ cs delta main~30 main          # Analyse the latest 30 commits on main

$ cs review test.c                       # Check the file test.c
$ cs review test.c --output-format json  # JSON output
$ cs review master:./test.c              # Check on the master branch

$ cs check test.c                        # Lint-style pass/fail
```
