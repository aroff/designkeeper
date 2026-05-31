# ADR 0002: Fetch-only template packs

## Status

Accepted

## Context

`dk` ships two official template packs (`default`, `structural`). Until now they were **embedded in the binary** via `include_str!` (`crates/dk-core/src/pack.rs`), and `install_pack_or_embedded_fallback` wrote that embedded copy whenever a remote fetch failed. Embedding was what let `dk review` run with **no `init` and no network** — a guarantee stated in `specs/vision.md` ("Works on any directory").

`dk-templates.toml` also carried placeholder sources (`TODO/dk-template-default`), so every `init`/`install` emitted an `HTTP 404` WARN before silently falling back to the embedded copy. The remote source did nothing but produce noise in front of content that was already canonical.

We want the template packs to live as the canonical source of truth **in the designkeeper repo** (`templates/`) and be **fetched on install**, rather than baked into the binary.

## Decision

Adopt a **fetch-only** pack model:

- Remove `include_str!` embedding and the embedded-fallback path.
- Official packs are fetched on `dk install` / `dk init` from sub-directories of the designkeeper repo (`aroff/designkeeper/templates/{default,structural}`).
- `dk review` / `dk check` **require** packs to be installed first; with none installed they fail fast (`DK_PACK_NOT_INSTALLED`, "run `dk install`") instead of falling back.

This depends on a new **aikit-sdk subpath capability** (`TemplateSource` addressing `owner/repo/subdir`), tracked in goaikit/aikit `specs/templatesource-subpath-support.md`. Until that lands, the change is blocked. Consistent with ADR 0001, the fetch capability lives in aikit-sdk, not in `dk`.

## Considered Options

- **Keep embedded as offline fallback** — preserves zero-config `dk review`, but keeps templates in the binary, which contradicts the goal of a single repo-hosted source of truth.
- **Per-pack repos** (`aroff/dk-template-default`, …) — works with aikit-sdk today (manifest at repo root), but scatters the templates out of the designkeeper repo.
- **Release-asset zips** — keeps templates in-repo and needs no aikit-sdk change, but requires a release-packaging step and a less direct source URL.

We chose repo-subdir fetch (via the aikit-sdk extension) because it keeps templates in this repo, versioned with the code, with the cleanest source syntax.

## Consequences

**Positive:**
- One canonical home for templates (this repo); no embedded/remote duplication; no `TODO/` 404 noise.
- Rubric/template content is editable without rebuilding the binary.

**Negative / to action:**
- The "works without init" guarantee is **dropped**; `specs/vision.md` and `pack.rs` docs must be updated. First use now needs network + an explicit `dk install`.
- `dk` is blocked on the aikit-sdk subpath change before this can ship.
