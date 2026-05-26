# Configuration & template packs

Two optional, user-editable inputs; both default sensibly when absent. `dk`
walks up parent directories to find each.

## `dk.toml` (control file)

Resolved by `dk_core::config::resolve_config` (`crates/dk-core/src/config.rs`):
walk up from the working dir to the first `dk.toml`; absent → built-in
defaults. Parsing uses `deny_unknown_fields`, so unknown keys are a hard error
(`DK_CONFIG_PARSE`).

```toml
[scan]
extensions = [".rs", ".ts"]          # file types auto-discovery considers
ignore_patterns = ["vendor/", "generated/"]

[output]
format = "markdown"                  # or "json"

[agent]
agent = "claude"                     # default agent key
model = ""                           # optional model override ("" = unset)

[templates]
pack = "default"                     # "default" or a local folder
```

All sections/fields are optional and merged over defaults. An empty `model =
""` is treated as unset.

**Defaults** (`default_config`): `extensions` = ~19 languages (`.rs .ts .tsx .js
.jsx .py .go .java .kt .c .cpp .h .hpp .rb .ex .exs .scala .swift .cs`),
`ignore_patterns` empty, `format` markdown, `agent` `claude`, `model` none,
`pack` `default`.

**Precedence**: CLI flags (`--agent`/`--model`/`--output-format`) > `dk.toml` >
built-in defaults.

## `.dk/` (template pack)

Created by `dk init`; layout (`crates/dk-core/src/pack.rs`):

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

`dk` resolves the pack by walking up for a `.dk/` containing
`templates/review.md` (`ensure_template_dir`, `crates/dk/src/main.rs`). If none
is found, the **embedded** default pack (compiled into the binary via
`include_str!` from `specs/review/`) is materialized to a temp dir — so reviews
work with no `.dk/` present. Edit any file to customize behavior.

## Prompt slot assembly

`slots::build_prompt_slots` fills the template's `{{slots}}`:

| Slot | Source |
|------|--------|
| `{{working_dir}}` | Canonicalized working dir. |
| `{{target}}` | The `<path>` arg, or newline-joined discovered files, or `"entire repository"`. |
| `{{change_context}}` | `--title/--description/--base-ref/--head-ref`, or "No PR/CL metadata supplied." |
| `{{focus}}` | `--focus` areas. |
| `{{project_hints}}` | Optional hints. |
| `{{methodology}}` | `templates/methodology.md`. |
| `{{max_findings}}` | `--max-findings` (default 25). |
| `{{output_schema}}` | Minified `schemas/review.json`. |

The prompt is a **task directive** — methodology + target + schema — not a
context payload. The agent reads source files from disk itself.

## File discovery (`discovery::discover_paths`)

Used only when `dk review` is run without a `<path>`. Walks the working dir with
the `ignore` crate (`require_git(false)`, so `.gitignore` applies even outside
git; hidden files/dirs skipped) and keeps files that:

1. match an extension in `[scan].extensions`,
2. are not gitignored,
3. are not under a `[scan].ignore_patterns` entry (a trailing `/` like
   `vendor/` matches everything under it).

Returns repo-relative, forward-slashed, sorted paths.
