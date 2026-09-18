#!/usr/bin/env bash
set -euo pipefail

assert_contains() {
  local haystack="$1"
  local needle="$2"
  local context="$3"
  if [[ "$haystack" != *"$needle"* ]]; then
    echo "$context missing required marker: $needle" >&2
    exit 1
  fi
}

echo "==> Phase 6 release checklist (Unix)"

cargo build --workspace --all-targets --locked
cargo test --workspace --all --release --locked
cargo build --release --locked

sort_out="$(./target/release/pasta tests/phase5_sort_smoke.ps)"
assert_contains "$sort_out" "=== PHASE5 SORT SMOKE PASS ===" "phase5_sort_smoke.ps"

full_out="$(./target/release/pasta tests/10_full_suite.ps)"
assert_contains "$full_out" "=== ALL 50 TESTS COMPLETE ===" "10_full_suite.ps"

big_out="$(./target/release/pasta tests/09_big_test.ps)"
assert_contains "$big_out" "=== ALL TESTS COMPLETE ===" "09_big_test.ps"

echo "RELEASE_CHECKLIST_OK"
