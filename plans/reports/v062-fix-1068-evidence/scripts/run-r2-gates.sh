#!/bin/zsh
# Usage: run-r2-gates.sh <worktree>
# The round-2 gates that need a cargo build, one after the other (they share one target folder), each in an isolated HOME/TMPDIR.
set -u
WT="$1"
E="$WT/plans/reports/v062-fix-1068-evidence"
RUN="$E/scripts/run-isolated.sh"
"$RUN" "$WT" "$E/r2-gate-clippy-workspace.log" clippy --workspace --all-targets --locked -- -D warnings
"$RUN" "$WT" "$E/r2-gate-test-mcp.log" test -p designos-mcp --locked
"$RUN" "$WT" "$E/r2-gate-test-eval.log" test -p designos-eval --locked
"$RUN" "$WT" "$E/r2-gate-test-app.log" test -p designos-app --bin designos --locked
echo "all gates ran"
