#!/bin/zsh
# Usage: run-final-gates.sh <worktree>
# The gates that need a cargo build, one after the other (they share one target folder), each in an isolated HOME/TMPDIR.
set -u
WT="$1"
E="$WT/plans/reports/v062-fix-1068-evidence"
RUN="$E/scripts/run-isolated.sh"
"$RUN" "$WT" "$E/gate-clippy-workspace-ci.log" clippy --workspace --all-targets --locked
"$RUN" "$WT" "$E/gate-test-mcp.log" test -p designos-mcp --locked
"$RUN" "$WT" "$E/gate-test-eval.log" test -p designos-eval --locked
"$RUN" "$WT" "$E/gate-test-app.log" test -p designos-app --bin designos --locked
echo "all gates ran"
