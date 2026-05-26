# CONTEXT.md — DesignKeeper

## Glossary

### DesignKeeper (`dk`)

An architecture and design compliance CLI tool. Core pipeline: prompt → template-based render → agent → structured output. CodeScene is the UX inspiration (commands, output style), not the domain. `review` and `check` work on any directory; only `drift` requires a git repository. The CLI is built on `cli-framework` (command registration, argument parsing, MCP server) and delegates the structured pipeline to `aikit-sdk` (template rendering, agent invocation, schema validation, retry, report rendering).

### Core Crate (`dk-core`)

The domain layer: file discovery (for default targets), config resolution (walk up directories, dk.toml parsing), init scaffolding (interactive prompts, template pack fetch), and command orchestration. The prompt→agent→validation→report pipeline is delegated to aikit-sdk's structured pipeline feature. Reused by the CLI, `serve`, and `mcp` without going through CLI dispatch.

### CLI Crate (`dk`)

A thin `cli-framework` binary shell: command registration, argument parsing, I/O formatting. Delegates all logic to the Core Crate.

### Agent

A full-featured coding agent (e.g. Claude Code, Opencode, Cursor) invoked via `aikit-sdk` (Rust library). The agent has filesystem access and works freely on the working directory. `dk` provides a task directive; the agent reads files, explores context, and produces a structured response independently. The agent's own LLM dependencies (API keys, providers) are managed by the agent runtime, not by `dk` directly.

### Template Pack

A set of files fetched from a GitHub URL into the project's `.dk/` directory by `dk init`. Organized as: `templates/` (prompt templates per command), `schemas/` (JSON output schemas per command), `reports/` (report layout templates per command). The default pack ships from the `dk` repo; alternative sources may be specified via `dk init` arguments. Users may edit installed files to customize behavior, including the review methodology (`templates/methodology.md`).

### Prompt Assembly

`dk` constructs a task directive from the template pack, not a full context payload. The prompt contains: methodology, target path(s), and expected output schema. Named slots (e.g. `{{methodology}}`, `{{target}}`, `{{output_schema}}`) are filled programmatically. The agent reads source files and project documentation from the filesystem itself.

### Response Pipeline

Delegated to aikit-sdk's structured pipeline. aikit-sdk validates the agent's response against the template pack's output schema, retries (up to 2 attempts) with augmented prompts including validation error details, and renders the final report. dk provides template paths, slot values, agent config, and output format. Default output format is markdown; override with `--output-format json`. Default output is stdout; `--output-file <path>` writes to disk.

### Control File

An optional `dk.toml` in the project root (or parent directories), created by `dk init` or manually. `dk` walks up directories to find it. Fields:

```toml
[scan]
extensions = [".rs", ".ts"]        # override default file types
ignore_patterns = ["vendor/", "generated/"]

[output]
format = "markdown"                # or "json"

[agent]
agent = "claude"                   # default agent key
model = ""                         # optional model override

[templates]
pack = "default"                   # GitHub URL or local folder path
```

When absent, `dk` uses built-in defaults.

### File Discovery

`dk` uses the `ignore` crate for fast recursive file walking (respects .gitignore) and `globset` for pattern matching. Used to determine default analysis targets when the user doesn't specify a path, and to enumerate files for `dk check`. The agent reads the actual file content independently. Default recognized extensions: `.rs`, `.ts`, `.tsx`, `.js`, `.jsx`, `.py`, `.go`, `.java`, `.kt`, `.c`, `.cpp`, `.h`, `.hpp`, `.rb`, `.ex`, `.exs`, `.scala`, `.swift`, `.cs`. Extendable via control file.

### Review

Evaluates a code snapshot or change against a structured review rubric (default: Google eng-practices, 13 dimensions, 0–10 scores). Produces per-dimension scores, a verdict (`approve`, `approve_with_comments`, `request_changes`, `reject`), actionable findings, and suggested next steps. Accepts PR/CL change context (`--title`, `--description`, `--base-ref`, `--head-ref`) and optional focus areas (`--focus`). The rubric methodology ships as a default template (`templates/methodology.md`) that users can edit or replace via the template pack. Works on any directory; git refs optional. Spec pack: `specs/review/`.

### Drift

Evaluates architectural trajectory over time. Detects degradation patterns, boundary erosion, and coupling growth. Answers: "are we getting worse?" Accepts `--since <ref>` to scope the temporal window (default: previous commit, `HEAD~1`). The agent compares states using git commands on the working directory.

### Check

A strict pass/fail gate for automation (CI, pre-commit hooks). Runs `review` internally and maps the verdict to exit codes: `approve` or `approve_with_comments` → exit 0, `request_changes` or `reject` → exit 1. Always outputs a findings summary on failure. `--verbose` produces the full scored report.

### Doctor

A diagnostic command that reports on the runtime environment: installed agents, the effective configuration file (after walking up directories), template pack status, and agent reachability.

### `.dk/` Directory

The per-project directory (at repository root, or overridable) created by `dk init`. Structure:

```
.dk/
├── templates/
│   ├── review.md
│   ├── methodology.md
│   └── drift.md
├── schemas/
│   ├── review-input.json
│   ├── review.json
│   └── drift.json
├── reports/
│   ├── review.md
│   └── drift.md
└── dk.toml              # control file
```

Users may edit any file to customize behavior.

### Serve

Starts an HTTP server exposing a JSON API (`POST /review`, `POST /drift`, `POST /check`) for programmatic access to `dk` commands. Required flag: `--with-api`. Options: `--host <host>` (default: `127.0.0.1`), `--port <port>` (default: `8080`).

### MCP

A separate command (`dk mcp`) that starts an MCP server exposing `dk` commands as MCP tools. Uses cli-framework's `mcp-server` feature. Future: stdio transport mode (not currently supported in cli-framework).

### JSON Extraction

The agent returns freeform text containing a ```` ```json ```` code block. aikit-sdk extracts the JSON from this block for schema validation.

### Check Output

`dk check` always outputs a findings summary on failure (violations found). `--verbose` produces the full scored report (equivalent to `dk review` output but with pass/fail exit code semantics).

## Command Interface

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

Agent and model flags on review/drift/check override defaults from the control file. When absent, values from `dk.toml` `[agent]` section apply.

### Init Flow

`dk init` is interactive when arguments are not provided on the command line. It prompts for each parameter sequentially. For agent selection, `dk` uses aikit-sdk to detect installed agents and presents a closed list to choose from. Running `dk init` on an existing `.dk/` directory updates values in place — it is iterative, not one-shot.
