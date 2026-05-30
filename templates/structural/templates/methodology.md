# Structural code quality grading methodology

Based on the Thermo-Nuclear Code Quality Review framework — a strict, structural-first review that treats maintainability as a first-class constraint, not a style concern.

## North star

Grade whether the code **improves structural health**: is it easier to understand, extend, and delete than before? Do not merely check correctness. A working but structurally regressive change is a failing change.

Principles:

- Structural health is objective. "It works" is not sufficient.
- Prefer the solution that makes the code feel **inevitable in hindsight**.
- Actively look for "code judo" moves: restructurings that preserve behavior while deleting whole categories of complexity.
- Do not soften structural blockers into style nits.

## Review sequence

1. Identify changed files and their owners. Wrong structural direction → low `overall_structural_health`; flag immediately.
2. Assess the **Structure** group: are modules well-sized, is logic in the right layer, are helpers reused?
3. Assess the **Complexity** group: can branches/layers be deleted, is control flow tangled, is orchestration justified?
4. Assess the **Expressiveness** group: do abstractions earn their keep, are boundaries explicit, is the code direct?
5. Compute group scores (mean of sub-dimensions). Overall score = mean of all nine sub-dimensions.

## Dimension groups

### Group 1 — Structure
*Where things live at the macro level*

| Key | Focus |
|-----|-------|
| `file_decomposition` | Files stay under healthy size boundaries; PRs that push a file past ~1000 lines are presumptive blockers unless justified |
| `layer_integrity` | Logic lives in the right layer and package; feature logic does not leak into shared paths; implementation details do not cross API boundaries |
| `helper_reuse` | Canonical utilities are used; bespoke near-duplicates are not introduced; logic is placed where the concept already lives |

### Group 2 — Complexity
*How hard the code is to reason about locally*

| Key | Focus |
|-----|-------|
| `structural_simplicity` | Whole branches, helpers, modes, or layers can be deleted rather than rearranged; the "code judo" move was taken when available |
| `branching_complexity` | New conditionals live in the right place; ad-hoc special-case branches are not bolted onto unrelated flows; spaghetti does not grow |
| `orchestration_quality` | Async/sequential flow is justified; independent work is not serialized unnecessarily; related updates are atomic where that matters |

### Group 3 — Expressiveness
*How clearly the code communicates its intent*

| Key | Focus |
|-----|-------|
| `abstraction_quality` | Abstractions earn their keep; thin wrappers and identity pass-throughs are not introduced; indirection buys clarity |
| `type_contract_clarity` | Boundaries are explicit; unnecessary `any`, `unknown`, or optionality does not obscure real invariants; casts are minimized |
| `legibility` | Implementation is direct and boring; magic, implicit assumptions, and incidental control flow are avoided |

## Score anchors (0–10)

| Range | Label | Meaning |
|-------|-------|---------|
| **9–10** | Good | Exemplary; at most trivial observations. |
| **8** | Low | Minor concern; non-blocking follow-up welcome. |
| **6–7** | Medium | Meaningful issue; should be addressed before or shortly after merge. |
| **4–5** | High | Serious structural problem; merge is risky without a fix. |
| **0–3** | **Critical** | Structural regression; blocks merge; adds significant penalties to code quality. |

A score below **4** on any single dimension is a **critical** finding and acts as a quality penalty — it degrades the `overall_structural_health` score beyond its arithmetic contribution.

## Findings (mandatory quality bar)

Each finding must let another developer act **without asking you**:

- **observation**: specific (file, function, line range, or symbol).
- **why_it_matters**: tied to the dimension and structural principle above.
- **recommended_action**: imperative steps (split module, extract abstraction, move logic, delete wrapper, etc.).

Bad: "Code is tangled."
Good: "In `src/orders/processor.rs:45-110`, `process()` mixes HTTP dispatch, retry scheduling, and persistence. Extract `HttpDispatcher` and `RetryScheduler`; keep `process()` as thin orchestration."

Cap at **{{max_findings}}** findings; prefer critical and high findings.

## Group and overall score computation

- `structure_score` = mean(`file_decomposition`, `layer_integrity`, `helper_reuse`)
- `complexity_score` = mean(`structural_simplicity`, `branching_complexity`, `orchestration_quality`)
- `expressiveness_score` = mean(`abstraction_quality`, `type_contract_clarity`, `legibility`)
- `overall_score` = mean of all nine sub-dimensions (exclude `not_evaluated`)

**Critical penalty rule**: if any sub-dimension scores below 4, subtract 0.5 from `overall_score` per offending dimension (floor at 0). Document applied penalties in `summary.one_paragraph`.

## Verdict mapping

- **reject**: `overall_score` < 4 OR any sub-dimension scored 0–3
- **request_changes**: `overall_score` 4–5 OR any sub-dimension scored 4–5
- **approve_with_comments**: `overall_score` 6–7; only medium/low findings
- **approve**: `overall_score` ≥ 8; no critical or high findings
