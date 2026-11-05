#!/bin/sh
# Quiet, parallel run of the unchanged six-command gate (G01-G06).
# Prints one PASS/FAIL line per command; full logs go to $GATE_LOG_DIR, and only failures show their tail.
cd "$(dirname "$0")/.." || exit 2
D="${GATE_LOG_DIR:-${TMPDIR:-/tmp}/narihito-gate}"
mkdir -p "$D" && rm -f "$D"/*.log "$D"/*.rc
M=src-tauri/Cargo.toml

run() { # name, command...
  n=$1; shift
  "$@" >"$D/$n.log" 2>&1
  echo $? >"$D/$n.rc"
}

# Rust lane is sequential: clippy and test share cargo's build-directory lock anyway.
(
  run G01 cargo fmt --manifest-path $M --check
  run G02 cargo clippy --manifest-path $M --all-targets --quiet -- -D warnings
  run G03 cargo test --manifest-path $M --quiet
) &
run G04 npx prettier --check . --log-level warn &
run G05 npx eslint . --max-warnings 0 &
run G06 npx vitest run --reporter=dot &
wait

fail=0
for n in G01 G02 G03 G04 G05 G06; do
  if [ "$(cat "$D/$n.rc")" = 0 ]; then
    echo "$n PASS"
  else
    echo "$n FAIL (log: $D/$n.log)"; tail -n 15 "$D/$n.log"; fail=1
  fi
done
exit $fail
