#!/usr/bin/env bash
#
# Smoke driver for the `dk` CLI.
#
# Builds the binary, then exercises every command's safe surface and asserts
# exit codes / output. Commands that would spawn a real agent subprocess
# (`dk review` / `dk check` against an installed agent) are NOT run for real —
# instead we drive the pipeline up to agent invocation with a deliberately
# missing agent and assert the DK_AGENT_NOT_FOUND error path.
#
# Usage:   bash skill/scripts/smoke.sh
# Exit:    0 = all checks passed, 1 = one or more failed.
#
# Resolves the repo root from its own location (skill/scripts/ -> repo root).

set -uo pipefail

REPO="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$REPO"

PASS=0
FAIL=0
note() { printf '\n\033[1m== %s\033[0m\n' "$*"; }
ok()   { PASS=$((PASS+1)); printf '  \033[32mPASS\033[0m %s\n' "$*"; }
bad()  { FAIL=$((FAIL+1)); printf '  \033[31mFAIL\033[0m %s\n' "$*"; }

# assert_rc <expected_rc> <description> -- <command...>
assert_rc() {
  local want="$1" desc="$2"; shift 2; [ "$1" = "--" ] && shift
  "$@" >/tmp/dk_smoke.out 2>&1
  local got=$?
  if [ "$got" = "$want" ]; then ok "$desc (rc=$got)"; else
    bad "$desc (want rc=$want, got $got)"; sed 's/^/      | /' /tmp/dk_smoke.out; fi
}

# assert_contains <needle> <description> -- <command...>
assert_contains() {
  local needle="$1" desc="$2"; shift 2; [ "$1" = "--" ] && shift
  local out; out="$("$@" 2>&1)"
  if printf '%s' "$out" | grep -qF "$needle"; then ok "$desc"; else
    bad "$desc (missing: $needle)"; printf '%s\n' "$out" | sed 's/^/      | /'; fi
}

note "Build"
if cargo build -p dk >/tmp/dk_build.log 2>&1; then ok "cargo build -p dk"; else
  bad "cargo build -p dk"; tail -20 /tmp/dk_build.log; exit 1; fi
BIN="$REPO/target/debug/dk"

note "Top-level surface"
assert_rc 0 "version"          -- "$BIN" version
assert_contains "dk 0"  "version prints name+semver" -- "$BIN" version
assert_contains "review" "--help lists commands"     -- "$BIN" --help
assert_rc 0 "spec --format json"   -- "$BIN" spec --format json
assert_contains '"name": "dk"' "spec emits command surface" -- "$BIN" spec --format json
assert_rc 0 "completion bash"      -- "$BIN" completion bash
assert_contains "complete -F _dk dk" "completion emits bash stub" -- "$BIN" completion bash

note "init + doctor (in a throwaway project dir)"
# init writes dk.toml / .dk into the CWD, so always run it inside a temp dir to
# avoid polluting the repo.
WORK="$(mktemp -d)"
assert_rc 0 "init --agent codex" -- bash -c 'cd "$1" && "$2" init --agent codex --model gpt-5 --template-pack default' _ "$WORK" "$BIN"
[ -f "$WORK/dk.toml" ] && ok "init wrote dk.toml" || bad "init wrote dk.toml"
[ -f "$WORK/.dk/templates/review.md" ] && ok "init wrote .dk/ pack" || bad "init wrote .dk/ pack"
grep -q 'agent = "codex"' "$WORK/dk.toml" && ok "dk.toml records agent" || bad "dk.toml records agent"
# doctor honors CWD; report runs even when checks warn/err (rc reflects errors).
( cd "$WORK" && "$BIN" doctor --json ) >/tmp/dk_doctor.json 2>&1
grep -q '"check_id": "config"' /tmp/dk_doctor.json && ok "doctor --json includes config check" || bad "doctor --json includes config check"
grep -q '"check_id": "agent-reachability"' /tmp/dk_doctor.json && ok "doctor --json includes reachability check" || bad "doctor --json includes reachability check"

note "review / check help + pipeline error path (no live agent)"
assert_rc 0 "review --help"  -- "$BIN" review --help
assert_rc 0 "check --help"   -- "$BIN" check --help
# Drive the review pipeline up to agent invocation with a missing agent.
echo "fn main() {}" > "$WORK/a.rs"
assert_rc 1 "review with missing agent fails" -- "$BIN" review --agent dk-no-such-agent-xyz "$WORK"
assert_contains "DK_AGENT_NOT_FOUND" "review surfaces agent-not-found code" -- "$BIN" review --agent dk-no-such-agent-xyz "$WORK"

note "mcp serve (stdio) exposes review as a tool"
TOOLS="$(printf '%s\n' \
  '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"smoke","version":"1"}}}' \
  '{"jsonrpc":"2.0","method":"notifications/initialized"}' \
  '{"jsonrpc":"2.0","id":2,"method":"tools/list"}' \
  | timeout 8 "$BIN" mcp serve --transport stdio 2>/dev/null)"
if printf '%s' "$TOOLS" | grep -q '"name":"dk.review"'; then ok "mcp tools/list exposes dk.review"; else
  bad "mcp tools/list exposes dk.review"; printf '%s\n' "$TOOLS" | sed 's/^/      | /'; fi

note "Result: $PASS passed, $FAIL failed"
rm -rf "$WORK"
[ "$FAIL" -eq 0 ]
