# `maintainability` — DesignKeeper review pack

A strict, ambition-first maintainability audit for `dk`. It is a faithful rendering of the **Thermo-Nuclear Code Quality Review**: it does not just check that code works — it pushes hard for the simpler design, hunting for **"code judo"** moves that delete whole categories of complexity rather than rearrange them.

Use it as a pre-merge quality gate when you want a demanding reviewer focused on structure, abstraction quality, and codebase health.

## What it grades

Seven flat dimensions, each scored **0–10**:

| Dimension | Asks |
|---|---|
| `simplification_ambition` | Was the obvious simplification taken? Could branches/helpers/modes disappear entirely? |
| `file_decomposition` | Do files stay under healthy size? (A PR pushing a file past ~1000 lines is a presumptive blocker.) |
| `branching_discipline` | Are new conditionals in the right place, or is spaghetti growing? |
| `abstraction_economy` | Do abstractions earn their keep, or are they thin wrappers / magic? |
| `boundary_clarity` | Are types and boundaries explicit, or hidden behind casts / needless optionality? |
| `canonical_placement` | Is logic in the layer that owns it, reusing canonical helpers? |
| `orchestration_atomicity` | Is async/sequential flow justified, and are related updates atomic? |

The **overall score** is the mean of the evaluated dimensions, with a penalty when any dimension scores below 4. The **verdict** is one of `approve` / `approve_with_comments` / `request_changes` / `reject`.

## Install

```sh
dk packs install aroff/designkeeper/templates/maintainability
```

Or pull it in alongside the other official packs:

```sh
dk packs init        # installs all official packs, including maintainability
dk packs list        # confirm it is installed
```

Install user-wide instead of per-project with `--global`.

## Use

```sh
# Review the current tree and print a scored report
dk review --template maintainability src/

# Use as a CI gate: exit 0 = approve / approve_with_comments, exit 1 = request_changes / reject
dk check --template maintainability
```

Review a pull request with context, or derive it from git:

```sh
dk review --template maintainability --title "Refactor sync" --base-ref main --head-ref HEAD
dk check  --template maintainability --from-git main
```

Other useful flags (all standard `dk` flags work):

```sh
--focus performance --focus concurrency      # nudge attention to specific areas
--max-findings 15                             # cap the number of findings
--include-dimensions simplification_ambition,branching_discipline   # grade a subset
--output-format json --output-file out.json   # machine-readable report
--sarif dk-review.sarif                        # also emit SARIF for code scanning
```

### CI example

```yaml
- run: dk check --template maintainability --from-git ${{ github.event.pull_request.base.ref }}
```

## What you get

A markdown report with the verdict and overall score, a per-dimension grade table with rationales, prioritized findings (each with a location, why it matters, and a recommended action), plus good practices, limitations, and ordered next steps. With `--output-format json` you get the same content as a structured document.

## Tuning it for your team

After installing, the rubric lives at `.dk/packs/maintainability/templates/methodology.md` (or `~/.dk/packs/...` if installed globally). Edit it to adjust the file-size threshold, soften or sharpen the approval bar, or reweight what matters for your codebase — `dk` reads the dimension keys from the pack, so your changes take effect immediately.

## Which pack should I use?

- **`maintainability`** — strict, ambition-first audit; best when you want a refactor pushed toward its simplest form before merge.
- **`structural`** — the same framework organized as 9 sub-dimensions across Structure / Complexity / Expressiveness groups; good for architecture changes and large new modules.
- **`default`** — Google engineering-practices rubric; best for general feature PRs and bug fixes.
