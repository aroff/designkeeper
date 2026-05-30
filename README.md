# DesignKeeper (`dk`)

Agent-driven code review with a scored rubric and a pass/fail exit code.

`dk` builds a structured prompt from your chosen template, runs it through an AI agent (Claude, Codex, Gemini, …), validates the JSON response against a schema, and emits a scored report or CI exit code. Multiple review templates are supported — each targets a different quality lens.

## Install

**macOS**
```sh
brew install aroff/cli/designkeeper
```

**Windows**
```sh
scoop bucket add aroff https://github.com/aroff/scoop-bucket
scoop install designkeeper
```

**Cargo**
```sh
cargo install --git https://github.com/aroff/designkeeper --bin dk
```

Requires an agent CLI on your `PATH` (`claude` by default). Run `dk doctor` to verify.

## Quick start

```sh
cd your-project
dk init                                    # install template packs and write dk.toml
dk review --template default src/          # Google eng-practices review
dk review --template structural src/       # structural quality review
dk check  --template default src/          # exit 0 = approve, 1 = request_changes/reject
```

**CI gate**
```yaml
- run: dk check --template default
```

**PR review with context**
```sh
dk review --template default --title "Add auth" --base-ref main --head-ref feature/auth
```

**Focus on a specific area**
```sh
dk review --template default --focus security --focus concurrency
```

## Template packs

A template pack defines the review rubric, prompt, output schema, and report format. You must specify `--template <name>` on every `dk review` and `dk check` invocation.

### Built-in packs

| Pack | Rubric | Best for |
|---|---|---|
| `default` | [Google Engineering Practices](https://github.com/google/eng-practices) — 13 dimensions (design, tests, naming, complexity, …) | Feature PRs, bug fixes, general code review |
| `structural` | Structure · Complexity · Expressiveness — 9 sub-dimensions | Architecture changes, refactors, large new modules |

### Installing packs

`dk init` installs all official packs automatically from `dk-templates.toml`. To install packs manually:

```sh
dk install                          # install all official packs to .dk/packs/
dk install --global                 # install to ~/.dk/packs/ (user-wide)
dk install owner/repo               # install a single pack from GitHub
dk install owner/repo@v1.2.0        # specific version
dk install https://example.com/pack.zip
dk install ./local-template-dir
```

Packs are resolved in this order: project-local (`.dk/packs/{name}/`) → global (`~/.dk/packs/{name}/`) → embedded fallback for `default` and `structural`.

### Customizing a pack

After `dk init`, edit `.dk/packs/default/templates/methodology.md` to tune the rubric for your team. The original sources live in [`templates/`](templates/).

### Pack directory layout

```
.dk/packs/{name}/
├── templates/
│   ├── review.md        # prompt template ({{slots}})
│   └── methodology.md   # rubric (editable)
├── reports/
│   └── review.md        # report layout
└── schemas/
    ├── review-input.json
    └── review.json
```

## Commands

| Command | Description |
|---|---|
| `dk init` | Install template packs from `dk-templates.toml` and write `dk.toml`. Re-running is safe. |
| `dk install [--global] [<source>]` | Install packs from GitHub, a URL, or a local path. |
| `dk review --template <name> [<path>]` | Run a review; emit scored markdown or JSON report. |
| `dk check --template <name> [<path>]` | Same pipeline, exit 0/1. Prints nothing by default; `-v` for the full report. |
| `dk doctor` | Check config, installed packs, and agent availability. |
| `dk mcp serve` | Expose `dk` as an MCP tool (HTTP or stdio). |

### Verdicts and exit codes

| Verdict | `dk check` |
|---|---|
| `approve`, `approve_with_comments` | exit 0 |
| `request_changes`, `reject` | exit 1 |
| operational error (config, I/O, agent) | exit 2 |

### `dk review` flags

```
--template/-t <name>       template pack to use (required)
--agent/-a <key>           agent to use (default: claude)
--model/-m <model>         model override
--output-format <fmt>      markdown (default) or json
--output-file <path>       write report to file
--title <text>             changelist title (for PR context)
--description <file|text>  changelist description
--base-ref / --head-ref    git refs for diff context
--from-git <base-ref>      derive PR context from git (title, diff, changed files)
--focus <area>             security, concurrency, accessibility,
                           internationalization, privacy,
                           performance, api_design, ui
--max-findings <1-50>      cap findings (default: 25)
--include-dimensions <...> comma-separated dimensions to grade
```

## Configuration

`dk.toml` (walks up parent directories; all fields optional):

```toml
[scan]
extensions      = [".rs", ".ts"]
ignore_patterns = ["vendor/", "generated/"]

[output]
format = "markdown"   # or "json"

[agent]
agent = "claude"
model = ""            # optional override
```

## License

Apache-2.0. The `default` rubric derives from [google/eng-practices](https://github.com/google/eng-practices) (CC BY 3.0) — see [LICENSE](LICENSE).
