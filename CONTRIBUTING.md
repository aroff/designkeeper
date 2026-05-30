# Contributing to DesignKeeper

## Architecture

Two crates in `crates/`:

- **`dk-core`** — domain layer. Config resolution, file discovery, template pack management (`pack`, `pack_store`, `remote`), prompt slot assembly, the review/check pipeline, init scaffolding. No CLI dependency; reusable by any front-end.
- **`dk`** — thin `cli-framework` binary shell. Command registration, argument parsing, I/O formatting. All logic delegates to `dk-core` and `aikit-sdk`.

External dependencies worth knowing:
- **`aikit-sdk`** — template rendering, agent subprocess invocation, schema validation, retry, report rendering, remote pack fetching (`TemplateSource`, `fetch_package_to_dir`).
- **`cli-framework`** — command registration, MCP server, doctor module.

## Glossary

**Template pack** — a directory with a fixed layout (`templates/review.md`, `templates/methodology.md`, `reports/review.md`, `schemas/review-input.json`, `schemas/review.json`) plus an `aikit.toml` manifest. Installed under `.dk/packs/{name}/` (project-local) or `~/.dk/packs/{name}/` (global). The two built-in packs (`default`, `structural`) are embedded in the binary as fallbacks.

**`dk-templates.toml`** — the official pack manifest at the repo root, embedded in the binary. Lists pack names and their GitHub sources. `dk init` and `dk install` (with no args) read this to know what to fetch.

**Pack resolution order** — project-local → global → embedded fallback (for `default`/`structural`) → `DK_PACK_NOT_FOUND`.

**`--template`** — required flag on `dk review` and `dk check`. No default; the user always names the pack explicitly.

**Response pipeline** — `dk` builds a prompt (methodology + target + schema slots), hands it to the agent, extracts the JSON block, validates against the pack's output schema, retries up to 2 times on failure.

**Control file** — `dk.toml`, walked up from CWD. Fields: `[scan]` (extensions, ignore_patterns), `[output]` (format), `[agent]` (agent, model). No `[templates]` section — pack selection is per-command.

## Current command interface

```
dk init    [-a --agent <a>] [-m --model <m>]
dk install [-g --global] [<source>]
dk review  --template <name> [<path>] [-a --agent] [-m --model]
           [--output-format] [--output-file]
           [--title] [--description] [--base-ref] [--head-ref] [--from-git]
           [--focus <area>]... [--max-findings <n>] [--include-dimensions]
dk check   --template <name> [<path>] [-a --agent] [-m --model]
           [--output-format] [--output-file] [--from-git] [-v --verbose]
dk doctor  [-j --json] [-c --check <id>]
dk mcp serve [--transport stdio|http]
```

## Template pack layout

Source packs live under `templates/{name}/` in this repo:

```
templates/{name}/
├── aikit.toml               # [package] name, version, description
├── templates/
│   ├── review.md            # prompt template ({{slots}})
│   └── methodology.md       # review rubric
├── reports/
│   └── review.md            # report layout
└── schemas/
    ├── review-input.json
    └── review.json          # output schema
```

File names must match exactly — `pack.rs` resolves them by convention, not config.

The `default` pack uses Google Engineering Practices (13 dimensions, severity `blocker/major/minor/nit`). The `structural` pack uses the Structure · Complexity · Expressiveness rubric (9 sub-dimensions, severity `critical/high/medium/low`, score < 4 triggers a quality penalty).

## Running tests

```sh
bash scripts/run-tests.sh
```

Runs in order: `cargo fmt --check`, `cargo clippy -D warnings`, `cargo test --all-features`, then CLI smoke tests. The smoke tests build the binary and exercise every command's safe surface without a live agent call — they need only a Rust toolchain.

Add a smoke assertion in `scripts/run-tests.sh` whenever you add a new subcommand or error code.

## Adding a template pack

1. Create `templates/{name}/` with the layout above.
2. Add an `aikit.toml` with `[package] name`, `version`, `description`.
3. Add an entry to `dk-templates.toml` at the repo root.
4. Add `include_str!` constants to `crates/dk-core/src/pack.rs` and a `write_{name}_pack` function.
5. Add the embedded fallback branch to `pack_store::write_embedded_pack_to_temp` and `init::write_embedded_fallback`.
6. Update smoke tests in `scripts/run-tests.sh`.
