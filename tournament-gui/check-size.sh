#!/bin/sh
# Fails when the wasm of the GUI's release build, as served with gzip, exceeds the budget. Run
# after `dx build -p tournament-gui --release --debug-symbols false` (the `wasm-size` hook does
# both). The budget catches a change that makes the page much larger; raise it on purpose when
# that is worth it, and say so in AGENTS.md and DESIGN.md.
set -eu

budget=1500000

dir=target/dx/tournament-gui/release/web/public
wasm=$(find "$dir" -name '*.wasm')
if [ "$(printf '%s\n' "$wasm" | grep -c .)" -ne 1 ]; then
    echo "expected one wasm in $dir, found: $wasm" >&2
    exit 1
fi

raw=$(wc -c < "$wasm")
gzipped=$(gzip -9c "$wasm" | wc -c)
report="wasm of the GUI: $raw bytes, $gzipped with gzip (budget $budget)"
echo "$report"
if [ -n "${GITHUB_STEP_SUMMARY:-}" ]; then
    echo "$report" >> "$GITHUB_STEP_SUMMARY"
fi

if [ "$gzipped" -gt "$budget" ]; then
    echo "the wasm exceeds its budget by $((gzipped - budget)) bytes with gzip" >&2
    exit 1
fi
