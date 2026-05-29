# DesignKeeper (`dk`)

Agent-driven code review with a scored rubric and a pass/fail exit code.

`dk` builds a structured prompt from your methodology, runs it through an AI agent (Claude, Codex, Gemini, …), validates the JSON response against a schema, and emits a scored report or CI exit code. The rubric is based on [Google Engineering Practices](https://github.com/google/eng-practices) and scores 13 dimensions on a 0–10 scale.

## Install

**macOS**
```sh
brew install aroff/cli/dk
```

**Windows**
```sh
scoop bucket add aroff https://github.com/aroff/scoop-bucket
scoop install dk
```

**Cargo**
```sh
cargo install --git https://github.com/aroff/designkeeper --bin dk
```

Requires an agent CLI on your `PATH` (`claude` by default). Run `dk doctor` to verify.

## Quick start

```sh
cd your-project
dk init                    # scaffold .dk/ and dk.toml
dk review src/             # scored markdown report to stdout
dk check src/              # exit 0 = approve, 1 = request_changes/reject
```

**CI gate**
```yaml
- run: dk check
```

**PR review**
```sh
dk review --title "Add auth" --base-ref main --head-ref feature/auth
```

**Focus on a specific area**
```sh
dk review --focus security --focus concurrency
```

## Commands

| Command | Description |
|---|---|
| `dk init` | Scaffold `.dk/` template pack and `dk.toml`. Re-running updates in place. |
| `dk review [<path>]` | Run a review; emit scored markdown or JSON report. |
| `dk check [<path>]` | Same pipeline, exit 0/1. Prints nothing by default; `-v` for the full report. |
| `dk doctor` | Check config, template pack, agent availability. |
| `dk mcp serve` | Expose `dk` as an MCP tool (HTTP or stdio). |

### Verdicts and exit codes

| Verdict | `dk check` |
|---|---|
| `approve`, `approve_with_comments` | exit 0 |
| `request_changes`, `reject` | exit 1 |
| operational error (config, I/O, agent) | exit 2 |

### `dk review` flags

```
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
```

## Configuration

`dk.toml` (walks up parent directories; all fields optional):

```toml
[scan]
extensions      = [".rs", ".ts"]
ignore_patterns = ["vendor/", "generated/"]

[output]
format = "markdown"          # or "json"

[agent]
agent = "claude"
model = ""                   # optional override

[templates]
pack = "default"             # "default" or path to a custom pack
```

`.dk/` is the editable template pack written by `dk init`. Edit `.dk/templates/methodology.md` to customize the rubric for your team. The built-in defaults are in [`templates/default/`](templates/default/).

## License

Apache-2.0. The default rubric derives from [google/eng-practices](https://github.com/google/eng-practices) (CC BY 3.0) — see [LICENSE](LICENSE).
