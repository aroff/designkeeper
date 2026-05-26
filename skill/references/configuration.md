# Configuration & template packs

Two optional, user-editable inputs; both default sensibly when absent. `dk`
walks up parent directories to find each.

## `dk.toml` (control file)

`dk` walks up from the working dir to the first `dk.toml`; if none is found it
uses built-in defaults. Unknown keys are a hard error (`DK_CONFIG_PARSE`).

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

**Defaults**: `extensions` = ~19 languages (`.rs .ts .tsx .js
.jsx .py .go .java .kt .c .cpp .h .hpp .rb .ex .exs .scala .swift .cs`),
`ignore_patterns` empty, `format` markdown, `agent` `claude`, `model` none,
`pack` `default`.

**Precedence**: CLI flags (`--agent`/`--model`/`--output-format`) > `dk.toml` >
built-in defaults.

## `.dk/` (template pack)

Created by `dk init`; layout:

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
`templates/review.md`. If none is found, the built-in default pack is used
(materialized to a temp dir) — so reviews work with no `.dk/` present. Edit any
file in `.dk/` to customize behavior.

## Prompt slot assembly

`dk` fills the template's `{{slots}}`:

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

## File discovery

Used only when `dk review` is run without a `<path>`. `dk` walks the working dir
— honoring `.gitignore` even outside a git repo, and skipping hidden files/dirs
— and keeps files that:

1. match an extension in `[scan].extensions`,
2. are not gitignored,
3. are not under a `[scan].ignore_patterns` entry (a trailing `/` like
   `vendor/` matches everything under it).

Returns repo-relative, forward-slashed, sorted paths.
