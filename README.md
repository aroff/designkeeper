# DesignKeeper (`dk`)

An architecture and design **compliance CLI** — a structured, agent-driven code
reviewer. `dk` turns an AI coding agent (Claude, Codex, Gemini, …) into a
repeatable review gate: it renders a methodology-based prompt, runs your agent
against a target, validates the agent's response against a JSON schema, and
emits a scored report or a pass/fail exit code.

CodeScene is the UX inspiration (commands, output style); the domain is design
and architecture review.

## Methodology

The default review rubric is based on the
[Google Engineering Practices](https://github.com/google/eng-practices) guide
(`looking-for.md`, `standard.md`, `cl-descriptions.md`, `small-cls.md`,
`navigate.md`). It scores 13 review dimensions on a 0–10 scale. The full
scoring anchors live in
[`templates/default/templates/methodology.md`](templates/default/templates/methodology.md)
and can be customized by editing `.dk/templates/methodology.md` after `dk init`.

## How it works

```
prompt (methodology + target + schema)  →  agent  →  structured JSON  →  validate/retry  →  report
```

`dk` builds a **task directive**, not a full context payload: the prompt
contains the review methodology, the target path(s), and the expected output
schema. The agent reads the source files from the filesystem itself. The
response is validated against the template pack's output schema and retried (up
to 2×) with the validation errors fed back in, then rendered as markdown or JSON.

## Install

**Prerequisites**

- A Rust toolchain (stable).
- An agent CLI on your `PATH` — `claude` by default. Run `dk doctor` to check
  what's installed and reachable.

**Build & install the binary**

```sh
git clone https://github.com/aroff/designkeeper
cd designkeeper
cargo install --path crates/dk      # installs `dk` into ~/.cargo/bin
```

Or build without installing:

```sh
cargo build --release               # binary at target/release/dk
```

## Quick start

```sh
cd your-project

# 1. Scaffold .dk/ (template pack) and dk.toml. Interactive when flags are
#    omitted; or pass them directly:
dk init --agent claude

# 2. Review the whole repo (or a subpath) and print a scored markdown report:
dk review
dk review src/

# 3. Use it as a pass/fail gate (exit 0 = approve, 1 = changes/reject):
dk check && echo "design OK"
```

## Commands

| Command | What it does |
| --- | --- |
| `dk init` | Scaffold `.dk/` and write `dk.toml`. Interactive when flags are omitted; re-running updates values in place. |
| `dk review [<path>]` | Run a structured review and emit a scored report (markdown or JSON). |
| `dk check [<path>]` | Run a review and map the verdict to an exit code — a CI/pre-commit gate. |
| `dk doctor` | Diagnose the environment: config file, template pack, installed agents, agent reachability. |
| `dk mcp serve` | Expose `dk` as an MCP server (HTTP or stdio) so agents can call `review` as a tool. |
| `dk spec` | Export the CLI command surface as JSON, YAML, or Markdown. |
| `dk completion` | Emit a shell completion stub. |

### `dk review`

```
dk review [<path>]
          [-a --agent <a>] [-m --model <m>]
          [--output-format markdown|json] [--output-file <path>]
          [--title <text>] [--description <file|text>]
          [--base-ref <ref>] [--head-ref <ref>]
          [--focus <area>]...        # security, concurrency, accessibility,
                                     # internationalization, privacy,
                                     # performance, api_design, ui
          [--max-findings <1-50>]    # default 25
```

Produces per-dimension scores, an overall verdict
(`approve` / `approve_with_comments` / `request_changes` / `reject`),
findings, and suggested next steps. Defaults to markdown on stdout; use
`--output-format json` and/or `--output-file` to change that.

### `dk check`

Same review pipeline, collapsed to an exit code:

- `approve` / `approve_with_comments` → **exit 0**
- `request_changes` / `reject`, or a pipeline error → **exit 1**

By default it prints nothing on stdout (a findings summary goes to stderr on
failure). Pass `-v/--verbose` to also print the full report.

## Main use cases

- **Local design review** — `dk review src/` for a scored report while you work.
- **CI / pre-commit gate** — `dk check` returns a non-zero exit code when the
  verdict is `request_changes` or `reject`, so it drops straight into a pipeline:
  `dk check || exit 1`.
- **PR / changelist review** — feed change context with
  `--title`, `--description`, `--base-ref`, `--head-ref` to focus the review on
  a diff.
- **As an MCP tool** — `dk mcp serve` lets an MCP-capable agent invoke `dk.review`
  directly (HTTP `--transport http` or `--transport stdio`).
- **Custom methodology** — edit `.dk/templates/methodology.md` (or point
  `--template-pack` at your own folder) to enforce your team's rubric.

## Configuration

`dk init` creates two things in your project; both are optional (built-in
defaults are used when absent), and `dk` walks up parent directories to find them.

**`dk.toml`** — the control file:

```toml
[scan]
extensions = [".rs", ".ts"]          # which file types auto-discovery considers
ignore_patterns = ["vendor/", "generated/"]

[output]
format = "markdown"                  # or "json"

[agent]
agent = "claude"                     # default agent key
model = ""                           # optional model override

[templates]
pack = "default"                     # "default" or a local folder
```

CLI `--agent` / `--model` / `--output-format` flags override `dk.toml`, which
overrides built-in defaults.

**`.dk/`** — the editable template pack (initialized from
[`templates/default/`](templates/default/)):

```
.dk/
├── templates/
│   ├── review.md        # prompt template with {{slots}}
│   └── methodology.md   # the review rubric — edit to customize
├── schemas/
│   ├── review-input.json
│   └── review.json      # output schema the agent must satisfy
└── reports/
    └── review.md        # report layout
```

### Which files get reviewed?

- `dk review <path>` reviews exactly that path/glob (passed to the agent as-is).
- `dk review` with no path auto-discovers source files under the working dir:
  matching `[scan].extensions`, honoring `.gitignore`, skipping hidden files,
  and excluding `[scan].ignore_patterns`.

## License

Apache-2.0
