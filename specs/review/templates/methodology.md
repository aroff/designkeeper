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

## Focus area sub-rubrics

When a focus area is specified, apply the matching sub-rubric in addition to the standard dimension grades.

### security

Examine: authentication/authorisation checks, input validation, output encoding, secrets management, dependency vulnerabilities, error message leakage, path traversal, injection (SQL, shell, template), CSRF, SSRF.

- **9–10**: All attack surfaces hardened; secrets handled via vault/env; inputs validated at boundary; no known CVEs in added deps.
- **7–8**: No obvious vulnerabilities; one or two low-severity hygiene issues (e.g. missing rate limit on non-critical endpoint).
- **5–6**: At least one medium-severity gap (e.g. user-controlled path concatenation without sanitisation).
- **3–4**: High-severity issue present (e.g. SQL injection, hardcoded secret, unauthenticated privileged endpoint).
- **0–2**: Critical exploitable vulnerability or deliberate security regression.

### concurrency

Examine: shared mutable state, lock ordering, deadlock potential, race conditions, atomics misuse, async/await cancellation safety, worker pool sizing, idempotency under retries.

- **9–10**: All shared state properly synchronised; no TOCTOU; cancellation safe; idempotency documented.
- **7–8**: Sound synchronisation; minor concerns (e.g. coarse lock could be narrowed).
- **5–6**: Potential race under realistic load (e.g. check-then-act on shared resource without lock).
- **3–4**: Data race or deadlock plausible in production scenarios.
- **0–2**: Obvious race condition or deadlock that will manifest under normal concurrent use.

### accessibility

Examine: ARIA roles and labels, keyboard navigability, focus management, colour contrast, screen-reader text, form error association, skip links, semantic HTML.

- **9–10**: All interactive elements keyboard-accessible; ARIA used correctly; contrast meets WCAG AA; error messages associated.
- **7–8**: Mostly accessible; one minor gap (e.g. missing `aria-label` on icon button).
- **5–6**: Meaningful gap affecting a user group (e.g. modal traps keyboard, missing skip link).
- **3–4**: Multiple barriers; significant portion of UI unreachable without mouse or with screen reader.
- **0–2**: Core user flow inaccessible by keyboard or screen reader.

### internationalization

Examine: hard-coded strings vs. i18n key lookup, locale-aware formatting (dates, numbers, currency), RTL layout support, plural forms, character encoding, collation, timezone handling.

- **9–10**: All user-visible strings externalised; locale-aware formatting throughout; RTL tested or explicitly deferred with ticket.
- **7–8**: Strings externalised; minor formatting gap (e.g. `Date.toLocaleString` not used in one place).
- **5–6**: Several hard-coded strings or locale-unaware formatting in user-facing paths.
- **3–4**: Core user-visible text hard-coded; no i18n framework usage in changed files.
- **0–2**: Active regression (e.g. removes i18n support, breaks encoding).

### privacy

Examine: PII collection minimisation, consent gates, data retention, logging of sensitive fields, encryption at rest/transit, third-party data sharing, GDPR/CCPA compliance signals.

- **9–10**: PII scoped to minimum; encrypted in transit and at rest; not logged; retention policy referenced.
- **7–8**: Sound privacy posture; one minor logging or retention gap.
- **5–6**: PII logged or stored longer than necessary without documented justification.
- **3–4**: Sensitive data exposed in logs, error messages, or unencrypted storage.
- **0–2**: Privacy regression (e.g. removes consent gate, adds PII to public endpoint response).

### performance

Examine: algorithmic complexity, N+1 queries, cache invalidation, large allocations in hot paths, blocking I/O on async threads, missing indexes, payload sizes, lazy vs. eager loading.

- **9–10**: Hot paths profiled or evidently O(n) or better; queries indexed; payloads bounded.
- **7–8**: No obvious hotspots; one minor concern (e.g. unnecessary clone in loop).
- **5–6**: Likely performance issue under realistic load (e.g. N+1 query in list endpoint).
- **3–4**: Clear algorithmic or I/O problem that will degrade under production traffic.
- **0–2**: Introduces a known severe regression (e.g. synchronous HTTP call on UI thread, unbounded loop over table scan).

### api_design

Examine: naming consistency, backward compatibility, versioning, error contract, idempotency, pagination, rate-limit headers, HTTP method/status semantics, SDK ergonomics.

- **9–10**: Consistent naming; backward-compatible; errors machine-readable; idempotency documented; follows project API style.
- **7–8**: Sound design; minor naming inconsistency or missing rate-limit header.
- **5–6**: Breaking change without version bump, or error contract unclear.
- **3–4**: Multiple breaking changes, or API shape conflicts with project conventions.
- **0–2**: Fundamentally unusable or removes a public contract without migration path.

### ui

Examine: layout correctness across breakpoints, loading/error/empty states, accessibility (see sub-rubric), animation jank, form validation UX, responsive images, touch targets.

- **9–10**: All states handled; responsive; touch targets ≥ 44 px; animations respect `prefers-reduced-motion`.
- **7–8**: Correct on target breakpoints; one minor visual gap (e.g. missing empty state).
- **5–6**: Layout breaks on a common viewport or a state (loading/error) is unhandled.
- **3–4**: Multiple layout regressions or missing critical states affecting user comprehension.
- **0–2**: Core UI flow broken or completely unusable on a primary device class.
