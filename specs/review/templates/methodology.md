# Code review grading methodology

Based on [Google engineering practices — code review](https://github.com/google/eng-practices) (`looking-for.md`, `standard.md`, `cl-descriptions.md`, `small-cls.md`, `navigate.md`).

## North star

Grade whether the change **improves overall code health**. Approve mentally when it does, even if imperfect. Score **0** only when the change clearly **degrades** health or is the wrong change entirely.

Principles:

- Technical facts over personal preference.
- Project style guide wins where it exists; otherwise match the repo.
- Design tradeoffs use engineering principles, not taste.
- Optional polish is **nit** severity; do not block merge for nits alone.

## Review sequence

1. Read change context (title, description). Wrong direction → low `overall_code_health` / `design`; stop if pointless.
2. Inspect main files for design first.
3. Walk remaining files; use tests to infer intent when helpful.
4. Grade every in-scope dimension; cite file/symbol evidence.

## Dimensions

| Key | Focus |
|-----|--------|
| `overall_code_health` | Net effect on maintainability and readability of the system |
| `cl_description` | First line + body explain **what** and **why**; searchable history |
| `change_scope` | One self-contained change; reviewable size; tests in same change |
| `design` | Interactions sensible; right place in codebase and time |
| `functionality` | Intended behavior; good for users; edge cases and concurrency |
| `complexity` | Understandable quickly; no speculative over-engineering |
| `tests` | Appropriate automated tests; would fail if code breaks |
| `naming` | Names communicate purpose without excess length |
| `comments` | Explain **why**; code explains **what** |
| `style` | Matches style guide; drive-by reformat not mixed with logic |
| `consistency` | Consistent with guide and surrounding code |
| `documentation` | README/API/docs updated when behavior or build changes |
| `context_and_review_depth` | Change fits file and system; touched logic is understandable |

## Score anchors (0–10)

- **9–10**: Exemplary; at most trivial nits.
- **7–8**: Good; improves health; minor follow-ups optional.
- **5–6**: Acceptable with meaningful follow-ups before merge.
- **3–4**: Serious issues; merge risky without fixes.
- **0–2**: Reject-level for that dimension.

## Findings (mandatory quality bar)

Each finding must let another developer act **without asking you**:

- **observation**: specific (file, function, line range, or PR text).
- **why_it_matters**: tied to the rubric row above.
- **recommended_action**: imperative steps (split CL, add test, rename, extract, update doc section).

Bad: "Code is complex."  
Good: "In `pkg/foo/handler.go:88-120`, `ProcessOrder` mixes validation, persistence, and email; split into `validateOrder`, `persistOrder`, `notifyOrder`."

Cap at **{{max_findings}}** findings; prefer blockers and majors.

## Verdict mapping

- **reject**: overall mean &lt; 4 OR any blocker on `design`, `functionality`, or `overall_code_health`
- **request_changes**: mean 4–6 OR any blocker
- **approve_with_comments**: mean 6–8; only minor/nit findings
- **approve**: mean ≥ 8; no blockers

`overall_score` = rounded mean of evaluated dimensions (exclude `not_evaluated`).
