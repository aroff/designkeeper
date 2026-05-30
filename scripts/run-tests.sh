#!/usr/bin/env bash
# Run all tests: fmt, clippy, cargo test, then CLI smoke tests.
# Usage: bash scripts/run-tests.sh
# Exit:  0 = all passed, non-zero = something failed.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

# ---------------------------------------------------------------------------
# 1. Rust suite
# ---------------------------------------------------------------------------
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features "$@"

# ---------------------------------------------------------------------------
# 2. CLI smoke tests
# Builds the binary and exercises every command's safe surface without making
# a live agent call. Agent-dependent paths are driven up to agent invocation
# with a deliberately missing binary and assert the DK_AGENT_NOT_FOUND path.
# ---------------------------------------------------------------------------
BIN="$ROOT/target/debug/dk"

PASS=0
FAIL=0
note() { printf '\n\033[1m== %s\033[0m\n' "$*"; }
ok()   { PASS=$((PASS+1)); printf '  \033[32mPASS\033[0m %s\n' "$*"; }
bad()  { FAIL=$((FAIL+1)); printf '  \033[31mFAIL\033[0m %s\n' "$*"; }

assert_rc() {
  local want="$1" desc="$2"; shift 2; [ "$1" = "--" ] && shift
  "$@" >/tmp/dk_smoke.out 2>&1
  local got=$?
  if [ "$got" = "$want" ]; then ok "$desc (rc=$got)"; else
    bad "$desc (want rc=$want, got $got)"; sed 's/^/      | /' /tmp/dk_smoke.out; fi
}

assert_contains() {
  local needle="$1" desc="$2"; shift 2; [ "$1" = "--" ] && shift
  local out; out="$("$@" 2>&1)"
  if printf '%s' "$out" | grep -qF "$needle"; then ok "$desc"; else
    bad "$desc (missing: $needle)"; printf '%s\n' "$out" | sed 's/^/      | /'; fi
}

note "Top-level surface"
assert_rc 0 "version"                               -- "$BIN" version
assert_contains "dk 0"     "version prints semver"  -- "$BIN" version
assert_contains "review"   "--help lists review"    -- "$BIN" --help
assert_contains "install"  "--help lists install"   -- "$BIN" --help
assert_rc 0 "spec --format json"                    -- "$BIN" spec --format json
assert_contains '"name": "dk"' "spec emits surface" -- "$BIN" spec --format json
assert_rc 0 "completion bash"                       -- "$BIN" completion bash
assert_contains "complete -F _dk dk" "completion emits bash stub" -- "$BIN" completion bash

note "init + install + doctor"
WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT

assert_rc 0 "init --agent codex" \
  -- bash -c 'cd "$1" && "$2" init --agent codex --model gpt-5' _ "$WORK" "$BIN"
[ -f "$WORK/dk.toml" ] \
  && ok "init wrote dk.toml" || bad "init wrote dk.toml"
grep -q 'agent = "codex"' "$WORK/dk.toml" \
  && ok "dk.toml records agent" || bad "dk.toml records agent"

assert_rc 0 "install (embedded fallback)" \
  -- bash -c 'cd "$1" && "$2" install' _ "$WORK" "$BIN"
[ -d "$WORK/.dk/packs/default" ] \
  && ok "install created .dk/packs/default" || bad "install created .dk/packs/default"
[ -f "$WORK/.dk/packs/default/templates/review.md" ] \
  && ok "default pack has review.md" || bad "default pack has review.md"
[ -d "$WORK/.dk/packs/structural" ] \
  && ok "install created .dk/packs/structural" || bad "install created .dk/packs/structural"

( cd "$WORK" && "$BIN" doctor --json ) >/tmp/dk_doctor.json 2>&1
grep -q '"check_id": "config"'            /tmp/dk_doctor.json \
  && ok "doctor includes config check"            || bad "doctor includes config check"
grep -q '"check_id": "installed-packs"'   /tmp/dk_doctor.json \
  && ok "doctor includes installed-packs check"   || bad "doctor includes installed-packs check"
grep -q '"check_id": "agent-reachability"' /tmp/dk_doctor.json \
  && ok "doctor includes reachability check"      || bad "doctor includes reachability check"

note "--template required"
assert_rc 1 "review without --template fails" -- "$BIN" review "$WORK"
assert_contains "DK_INPUT_VALIDATION" "review missing --template emits DK_INPUT_VALIDATION" \
  -- "$BIN" review "$WORK"
assert_rc 1 "check without --template fails"  -- "$BIN" check "$WORK"

note "review / check help"
assert_rc 0 "review --help" -- "$BIN" review --help
assert_rc 0 "check --help"  -- "$BIN" check --help

note "pipeline error path (no live agent)"
echo "fn main() {}" > "$WORK/a.rs"
assert_rc 1 "review --template default with missing agent fails" \
  -- bash -c 'cd "$1" && "$2" review --template default --agent dk-no-such-agent-xyz .' _ "$WORK" "$BIN"
assert_contains "DK_AGENT_NOT_FOUND" "review surfaces agent-not-found code" \
  -- bash -c 'cd "$1" && "$2" review --template default --agent dk-no-such-agent-xyz . 2>&1' _ "$WORK" "$BIN"
assert_rc 1 "review --template structural with missing agent fails" \
  -- bash -c 'cd "$1" && "$2" review --template structural --agent dk-no-such-agent-xyz .' _ "$WORK" "$BIN"

note "pack-not-found"
assert_rc 1 "review with unknown template fails" \
  -- bash -c 'cd "$1" && "$2" review --template nonexistent-pack .' _ "$WORK" "$BIN"
assert_contains "DK_PACK_NOT_FOUND" "review unknown template emits DK_PACK_NOT_FOUND" \
  -- bash -c 'cd "$1" && "$2" review --template nonexistent-pack . 2>&1' _ "$WORK" "$BIN"

note "mcp serve (stdio) exposes review as a tool"
TOOLS="$(printf '%s\n' \
  '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"smoke","version":"1"}}}' \
  '{"jsonrpc":"2.0","method":"notifications/initialized"}' \
  '{"jsonrpc":"2.0","id":2,"method":"tools/list"}' \
  | timeout 8 "$BIN" mcp serve --transport stdio 2>/dev/null)"
if printf '%s' "$TOOLS" | grep -q '"name":"dk.review"'; then
  ok "mcp tools/list exposes dk.review"
else
  bad "mcp tools/list exposes dk.review"
  printf '%s\n' "$TOOLS" | sed 's/^/      | /'
fi

note "Smoke result: $PASS passed, $FAIL failed"
[ "$FAIL" -eq 0 ] || exit 1
