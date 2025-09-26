#!/usr/bin/env bash
set -euo pipefail

FILE="${1:-scripts/ci/dev-smoke-commands.txt}"

if [[ ! -f "$FILE" ]]; then
  echo "Command list not found: $FILE" >&2
  exit 2
fi

have_timeout=0
if command -v timeout >/dev/null 2>&1; then have_timeout=1; fi

fail=0
lineno=0

while IFS= read -r raw || [[ -n "$raw" ]]; do
  lineno=$((lineno+1))
  line="${raw%%#*}"
  line="$(echo "$line" | sed -e 's/^[[:space:]]*//' -e 's/[[:space:]]*$//')"
  [[ -z "$line" ]] && continue

  IFS='|' read -r cmd expect to_sec <<<"$(printf "%s" "$line")"
  cmd="$(echo "${cmd:-}" | xargs)"
  expect="$(echo "${expect:-}" | xargs || true)"
  to_sec="$(echo "${to_sec:-}" | xargs || true)"
  [[ -z "${to_sec:-}" ]] && to_sec="30"

  echo
  echo "────────────────────────────────────────────────────────"
  echo "[$lineno] RUN: $cmd"
  echo "      EXPECT: ${expect:-<none>}"
  echo "     TIMEOUT: ${to_sec}s"

  set +e
  if [[ "$have_timeout" -eq 1 ]]; then
    out="$(timeout "${to_sec}s" bash -lc "$cmd" 2>&1)"; rc=$?
  else
    out="$(bash -lc "$cmd" 2>&1)"; rc=$?
  fi
  set -e

  echo "$out"

  if [[ $rc -ne 0 ]]; then
    echo "✗ FAIL (exit=$rc)"; fail=1; continue
  fi

  if [[ -n "$expect" ]]; then
    if ! printf "%s" "$out" | grep -Eiq -- "$expect"; then
      echo "✗ FAIL (no match: $expect)"; fail=1
    else
      echo "✓ PASS (matched: $expect)"
    fi
  else
    echo "✓ PASS (exit=0)"
  fi
done < "$FILE"

exit $fail
