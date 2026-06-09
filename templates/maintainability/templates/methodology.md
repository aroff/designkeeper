# Maintainability audit methodology

A faithful rendering of the **Thermo-Nuclear Code Quality Review** — an unusually strict review of implementation quality, abstraction quality, and codebase health. It treats maintainability as a first-class constraint, not a style concern.

## North star

Above all, be **ambitious** about code structure. Do not merely identify local cleanup. Actively hunt for **"code judo"** moves: restructurings that preserve behavior while making the implementation dramatically simpler, smaller, more direct, and more elegant.

Grade whether the change makes the codebase **easier to understand, extend, and delete** than before. A working but structurally regressive change is a failing change.

Principles:

- Maintainability is objective. "It works" is not sufficient and never earns approval on its own.
- Prefer the solution that makes the code feel **inevitable in hindsight**.
- Prefer **deleting** complexity over rearranging it. A refactor that merely spreads the same complexity around is not a win.
- Prefer direct, boring, maintainable code over hacky or magical code.
- Do not soften major maintainability issues into mild suggestions. Be direct, serious, and demanding — but not rude.

## The seven dimensions

Each dimension corresponds to a non-negotiable standard from the framework. They are **flat** (no group scores) so the dimension list maps one-to-one onto a grading rubric's scores.

| Key | Standard | What to grade |
|-----|----------|---------------|
| `simplification_ambition` | Be ambitious; clean the design | Was the available "code judo" move taken? Could whole branches, helpers, modes, layers, or conditionals disappear entirely instead of being polished or centralized? Penalize preserving incidental complexity when a plausible reframing would delete it. This is the north-star dimension. |
| `file_decomposition` | File-size discipline | Files stay under healthy size boundaries. A PR that pushes a file from **under 1000 lines to over 1000 lines** is a presumptive blocker unless there is a compelling structural reason and the result is still clearly organized. Prefer extracting helpers, subcomponents, or modules. |
| `branching_discipline` | No spaghetti growth | New conditionals live in the right place. Ad-hoc special-case branches, one-off booleans, nullable modes, or flags must not be bolted onto unrelated flows. Repeated conditionals signal a missing model, helper, state machine, or dispatcher. |
| `abstraction_economy` | Abstractions earn their keep; direct over magic | Reject thin wrappers, identity pass-throughs, and indirection that does not buy clarity. Be skeptical of generic "magic" mechanisms that hide simple data-shape assumptions. Indirection must pay for itself. |
| `boundary_clarity` | Type and boundary cleanliness | Boundaries are explicit. Question unnecessary `any`, `unknown`, optionality, or cast-heavy code where a clearer typed contract could exist. A branch relying on silent fallback to paper over an unclear invariant should instead make the boundary explicit. |
| `canonical_placement` | Right layer; reuse canonical helpers | Logic lives in the layer/package that owns the concept. Feature logic must not leak into shared paths; implementation details must not cross API boundaries. Prefer existing canonical utilities over bespoke near-duplicates. |
| `orchestration_atomicity` | Justified orchestration; atomic updates | Sequential/async flow is justified. Independent work serialized for no reason should run in parallel when that simplifies the structure. Related updates that can leave state half-applied should be made more atomic. Do not over-index on micro-optimizations. |

## Review sequence

1. Identify changed files and their owners. Wrong structural direction → flag immediately and grade low.
2. For every meaningful change, ask the primary questions below.
3. Grade each in-scope dimension 0–10 with a short rationale.
4. Emit specific, actionable findings — prefer a small number of high-conviction comments over a long list of cosmetic nits.
5. Compute `overall_score` and apply the critical penalty rule.

### Primary review questions

- Is there a "code judo" move that would make this dramatically simpler?
- Can this be reframed so fewer concepts, branches, or helper layers are needed?
- Did the diff add branching where a better abstraction should exist?
- Did a cohesive module become more coupled, more stateful, or harder to scan?
- Did this enlarge a file past a healthy size boundary?
- Is this abstraction earning its keep, or is it just a wrapper?
- Did the diff introduce casts, optionality, or ad-hoc shapes that obscure the real invariant?
- Is this logic in the canonical layer, or did detail leak across a boundary?
- Is this orchestration more sequential or less atomic than it needs to be?

## Score anchors (0–10)

| Range | Label | Meaning |
|-------|-------|---------|
| **9–10** | Good | Exemplary; at most trivial observations. |
| **8** | Low | Minor concern; non-blocking follow-up welcome. |
| **6–7** | Medium | Meaningful issue; should be addressed before or shortly after merge. |
| **4–5** | High | Serious maintainability problem; merge is risky without a fix. |
| **0–3** | **Critical** | Maintainability regression; blocks merge; adds a quality penalty. |

A score below **4** on any dimension is a **critical** finding and acts as a quality penalty — it degrades `overall_score` beyond its arithmetic contribution.

## Findings (mandatory quality bar)

Each finding must let another developer act **without asking you**:

- **observation**: specific (file, function, line range, or symbol).
- **why_it_matters**: tied to the dimension and the principle above.
- **recommended_action**: imperative (delete the layer, reframe the state model, extract the helper, move the logic, make the boundary explicit, parallelize the work).

Bad: "Code is tangled."
Good: "In `src/orders/processor.rs:45-110`, `process()` mixes HTTP dispatch, retry scheduling, and persistence. Extract `HttpDispatcher` and `RetryScheduler`; keep `process()` as thin orchestration."

A finding's `dimension` must be one you actually scored — never a `not_evaluated` dimension. Cap at **{{max_findings}}** findings; prefer critical and high.

## Overall score computation

- `overall_score` = mean of all evaluated dimensions (exclude any `not_evaluated`).
- **Critical penalty rule**: for each dimension scored below 4, subtract 0.5 from `overall_score` (floor at 0). Document applied penalties in `summary.one_paragraph`.

## Verdict mapping

- **reject**: `overall_score` < 4 OR any dimension scored 0–3
- **request_changes**: `overall_score` 4–5 OR any dimension scored 4–5
- **approve_with_comments**: `overall_score` 6–7; only medium/low findings
- **approve**: `overall_score` ≥ 8; no critical or high findings

The approval bar is high: do not approve merely because behavior is correct. Block on a preserved code-judo opportunity, an unjustified file-size explosion, ad-hoc spaghetti branching, a hacky/magical abstraction, needless wrapper/cast/optionality churn, or a canonical-helper duplication / wrong-layer placement.

## Newton compatibility (assessment-v1)

This pack's output is designed to project mechanically onto Newton's `assessment-v1` wire contract when `dk` runs as a Newton **command Grader**. The projection is purely structural — no re-grading:

| dk output | Newton assessment-v1 | Transform |
|-----------|----------------------|-----------|
| — | `schema_version` | constant `"1"` |
| pack name | `grader` | e.g. `"dk:maintainability"` |
| invocation context | `scope` / `scope_id` | supplied by the grading operator |
| run time | `evaluated_at` | RFC 3339 timestamp |
| `summary.overall_score` (0–10) | `overall_score` (0–100) | `round(score * 10)` |
| `summary.verdict` | `verdict` | passthrough (identical enum) |
| `summary.one_paragraph` | `summary` | passthrough |
| each evaluated `grades[dim]` | one `scores[]` entry | `{dimension, score: score*10, rationale}` |
| each `findings[]` | one `observations[]` entry | `{dimension, severity, observation, why_it_matters, recommended_action}` + `location` parsed `"path:range"` → `{path, range}` + `evidence` → `evidence[]` |

Two design choices keep the projection lossless and the **Newton invariant** ("every `observations[].dimension` MUST appear in `scores[].dimension`") automatically satisfied:

1. **Flat dimensions** — no group scores. Newton's `scores[]` is flat, so each dk dimension becomes exactly one Newton score with no aggregation to invent.
2. **Findings only reference scored dimensions** — the schema forbids emitting a finding for a `not_evaluated` dimension, so every observation's dimension is guaranteed to exist in `scores[]`.

The `verdict` and `severity` enums are deliberately identical to Newton's, and both scales are linear, so the mapping is reversible and carries no semantic loss. `good_things`, `limitations`, and `suggested_next_steps` have no Newton home and are dropped on projection (or carried in `extensions` if the grading operator opts in).
