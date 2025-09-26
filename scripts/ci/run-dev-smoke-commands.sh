#!/usr/bin/env bash
set -euo pipefail

FILE="${1:-scripts/ci/dev-smoke-commands.txt}"

if [[ ! -f "$FILE" ]]; then
  echo "Command list not found: $FILE" >&2
  exit 2
fi

if [[ -n "${BIN_DIR:-}" && -d "$BIN_DIR" ]]; then
  export PATH="$BIN_DIR:$PATH"
fi

have_timeout=0
if command -v timeout >/dev/null 2>&1; then have_timeout=1; fi

fail=0
lineno=0

cleanup_orphans() {
  pkill -f 'mojave-node'       >/dev/null 2>&1 || true
  pkill -f 'mojave-sequencer'  >/dev/null 2>&1 || true
  pkill -f 'mojave-prover'     >/dev/null 2>&1 || true
  pkill -f '^rex(\s|$)'        >/dev/null 2>&1 || pkill -f '/rex ' >/dev/null 2>&1 || true
}

run_one() {
  local cmd="$1" expect="$2" to_sec="$3"
  local rc=0 out tmp
  tmp="$(mktemp)"

  if [[ "$have_timeout" -eq 1 ]]; then
    set +e
    timeout --foreground -k 1s "${to_sec}s" bash -lc "$cmd" >"$tmp" 2>&1
    rc=$?
    set -e
  else
    set +e
    bash -lc "$cmd" >"$tmp" 2>&1 &
    cmd_pid=$!
    secs=0
    while kill -0 "$cmd_pid" 2>/dev/null && [ $secs -lt "$to_sec" ]; do
      sleep 1; secs=$((secs+1))
    done
    if kill -0 "$cmd_pid" 2>/dev/null; then
      rc=124
      kill -TERM -"$cmd_pid" 2>/dev/null || kill -TERM "$cmd_pid" 2>/dev/null || true
      sleep 2
      kill -KILL -"$cmd_pid" 2>/dev/null || kill -KILL "$cmd_pid" 2>/dev/null || true
    else
      wait "$cmd_pid"; rc=$?
    fi
    set -e
  fi

  out="$(cat "$tmp")"; rm -f "$tmp"
  echo "$out"

  if [[ $rc -eq 124 || $rc -eq 137 ]]; then
    echo "⏱ Timed out after ${to_sec}s. Cleaning up stray processes..."
    cleanup_orphans
    echo "✗ FAIL (timeout)"
    return 1
  elif [[ $rc -ne 0 ]]; then
    echo "✗ FAIL (exit=$rc)"
    return 1
  fi

  if [[ -n "$expect" ]]; then
    expect="${expect%\"}"; expect="${expect#\"}"
    expect="${expect%\'}"; expect="${expect#\'}"
    if printf "%s" "$out" | grep -Eiq -- "$expect"; then
      echo "✓ PASS (matched: $expect)"
    else
      echo "✗ FAIL (no match: $expect)"
      return 1
    fi
  else
    echo "✓ PASS (exit=0)"
  fi

  cleanup_orphans
  return 0
}

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
  run_one "$cmd" "$expect" "$to_sec"
  rc=$?
  set -e
  if [[ $rc -ne 0 ]]; then
    fail=1
  fi
done < "$FILE"

exit $fail
