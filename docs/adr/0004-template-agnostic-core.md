# ADR 0004: Template-agnostic Pack pipeline in `dk-core`

## Status

Accepted

## Context

`dk-core` deserialized agent output into Rust types hardcoded for the **default**
Pack's rubric: `Dimension` (13 fixed variants), `Severity` (4 variants), and
`FocusArea` (8 variants). When `dk review --template structural` ran, aikit-sdk
validated the agent output against the structural Pack's `schemas/review.json`,
but the subsequent `serde_json::from_value::<ReviewOutput>` failed with
`unknown variant 'abstraction_quality'` because `Dimension` lacked structural
keys. The same failure hit any third-party Pack whose dimension/severity strings
differed from the default enum. `--focus`/`--include-dimensions` were parsed into
those enums, post-validation rules embedded default-rubric assumptions, and SARIF
hardcoded 13 dimension rules and a fixed severity map.

The Pack is meant to be the unit of rubric extensibility (ADR 0001/0002). Rubric
vocabulary baked into Rust enums contradicts that: every new rubric required a
core change, and a JSON-Schema-valid output could still fail the Rust enum parse.

## Decision

Make `dk-core` **template-agnostic**. No Pack identifier or rubric vocabulary
lives in Rust:

- Delete `Dimension`, `Severity`, and `FocusArea` enums. Keep `Verdict` — it is
  part of the core contract and drives `dk check` exit semantics.
- Replace the typed `ReviewOutput` with `ReviewDocument`, a lossless
  `serde_json::Value` wrapper with typed accessors for contract fields only.
  Pack-defined dimensions/severities are arbitrary strings.
- After the Pack output schema validates (by aikit-sdk), additionally validate
  against an embedded `dk-core-contract-v1.json` — the minimal shape every review
  document must satisfy. Failure → `DK_CONTRACT_VIOLATION` (a Pack authoring
  error).
- Validate CLI-built input against the Pack's `schemas/review-input.json` before
  the agent runs. Failure → `DK_INPUT_VALIDATION`. `--focus`/`--include-dimensions`
  pass through as `Vec<String>`; the Pack input schema is the validation gate.
- Report slots are Pack-driven: every key under `summary` is auto-exposed as a
  slot, plus a fixed set of computed slots (grades table, findings section, …).
- SARIF rules derive from the union of finding dimensions and grade keys;
  severity → level order is read from the Pack output schema.
- Remove the dead `[templates] pack` config field; `dk.toml` with that section now
  errors with `DK_CONFIG_PARSE`.
- `dk-core` tests use synthetic fixture Packs under
  `crates/dk-core/tests/fixtures/packs/` (`minimal`, `custom`), never the official
  `templates/`.

## Considered Options

- **Extend the Rust enums per Pack** — keeps strong typing but defeats Pack
  extensibility: every rubric needs a core release, and the schema/enum split
  keeps producing confusing `DK_PIPELINE_ERROR`s.
- **Typed partial struct mirroring the contract** — type-safe contract fields,
  but requires maintaining a Rust mirror of the contract schema and still drops
  Pack-added fields. `ReviewDocument` (Value wrapper) preserves everything.
- **Pack-owned `post-checks.json`** for re-encoding V1–V4 — deferred to v2; the
  core simply drops the default-rubric post-validation for now.

## Consequences

**Positive:**
- `dk review/check --template structural` and arbitrary third-party Packs work
  with no Rust change. Schema is the single contract.
- ~150 lines of fragile rubric enums and V1–V4 validation removed; the
  schema/enum mismatch failure mode is gone.
- Tests are hermetic (fixture Packs), not coupled to on-disk official templates.

**Negative / to action:**
- Breaking API: `ReviewOutput`, `Dimension`, `Severity`, `FocusArea`, `Summary`,
  `TemplatesConfig`, `validate_output` are removed. Callers migrate to
  `ReviewDocument` accessors.
- Default-rubric consistency checks (reject+high-score, blocker+pass-verdict) are
  no longer enforced by core; Packs that want them must encode them in
  `schemas/review.json`.
