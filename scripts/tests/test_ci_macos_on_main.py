"""The macOS CI jobs skip a push to main, and the `Test (macos-15)` aggregate reports that instead of failing.

The tests read `.github/workflows/ci.yml` as text (no PyYAML needed): they evaluate the `if:` of each macOS job for
every kind of event, run the aggregate job's shell script once per row of its truth table, and check the swiftc leg
of the native export compile matrix.
"""

import json
import os
import re
import subprocess
import textwrap
import unittest
from pathlib import Path
from types import SimpleNamespace

CI = Path(__file__).resolve().parents[2] / ".github" / "workflows" / "ci.yml"
MAIN_PUSH = "github.event_name == 'push' && github.ref == 'refs/heads/main'"

PR_REF = "refs/pull/1/merge"
MAIN_REF = "refs/heads/main"
TRAIN_REF = "refs/heads/train/20261112T000000Z"
TAG_REF = "refs/tags/v0.7.14"
QUEUE_REF = "refs/heads/gh-readonly-queue/main/pr-1-abc"


def job_block(name: str) -> str:
    """The text of one top-level job: from its `  name:` line to the next job (minus the next job's leading comment)."""
    lines = CI.read_text(encoding="utf-8").splitlines()
    start = lines.index(f"  {name}:")
    end = len(lines)
    for i in range(start + 1, len(lines)):
        if re.match(r"^  [A-Za-z0-9_-]+:\s*$", lines[i]):
            end = i
            break
    while end > start + 1 and (not lines[end - 1].strip() or lines[end - 1].startswith("  #")):
        end -= 1
    return "\n".join(lines[start:end])


def job_if(name: str) -> str:
    match = re.search(r"^    if: (.+)$", job_block(name), re.MULTILINE)
    assert match, f"job {name} has no `if:`"
    return match.group(1)


def job_runs(name: str, event: str, ref: str, draft: bool = False, rust: str = "true") -> bool:
    """Evaluate a job's `if:` the way GitHub does, for the few operators these expressions use."""
    expr = re.sub(r"!(?!=)", " not ", job_if(name)).replace("&&", " and ").replace("||", " or ")
    scope = {
        "github": SimpleNamespace(event_name=event, ref=ref, event=SimpleNamespace(pull_request=SimpleNamespace(draft=draft))),
        "needs": SimpleNamespace(changes=SimpleNamespace(outputs=SimpleNamespace(rust=rust))),
    }
    return bool(eval(expr, {"__builtins__": {}}, scope))  # noqa: S307 - the expression comes from our own workflow file


def aggregate_script() -> str:
    block = job_block("test-macos-required").splitlines()
    start = next(i for i, line in enumerate(block) if line.strip() == "run: |")
    body = []
    for line in block[start + 1 :]:
        if line.strip() and not line.startswith(" " * 10):
            break
        body.append(line)
    return textwrap.dedent("\n".join(body))


def run_aggregate(event: str, ref: str, shards: str, lint: str, changes: str = "success", rust: str = "true", draft: str = "") -> subprocess.CompletedProcess:
    env = {
        "PATH": os.environ["PATH"],
        "EVENT_NAME": event,
        "REF_NAME": ref,
        "PR_DRAFT": draft,
        "CHANGES_RESULT": changes,
        "RUST_CHANGED": rust,
        "SHARDS_RESULT": shards,
        "LINT_RESULT": lint,
    }
    return subprocess.run(["bash", "-c", aggregate_script()], env=env, capture_output=True, text=True, timeout=30)


class MacosJobsSkipPushToMain(unittest.TestCase):
    def test_shards_and_lint_budget_skip_only_a_push_to_main(self) -> None:
        for job in ("test-macos", "lint-budget"):
            self.assertIn(f"!({MAIN_PUSH})", job_if(job), f"{job} must carry the push-to-main exclusion")
            for event, ref, expected in (
                ("pull_request", PR_REF, True),
                ("push", MAIN_REF, False),
                ("push", TRAIN_REF, True),
                ("push", TAG_REF, True),
                ("merge_group", QUEUE_REF, True),
            ):
                self.assertEqual(job_runs(job, event, ref), expected, f"{job} on {event} {ref}")

    def test_existing_draft_and_docs_only_skips_still_apply(self) -> None:
        for job in ("test-macos", "lint-budget"):
            self.assertFalse(job_runs(job, "pull_request", PR_REF, draft=True), f"{job} on a draft pull request")
            self.assertFalse(job_runs(job, "pull_request", PR_REF, rust="false"), f"{job} on a docs-only pull request")
            self.assertFalse(job_runs(job, "push", TRAIN_REF, rust="false"), f"{job} on a docs-only train push")

    def test_swift_compile_check_leaves_the_matrix_on_main_and_nowhere_else(self) -> None:
        match = re.search(r"include: \$\{\{ fromJSON\(\((.+?)\) && '(\[.*?\])' \|\| '(\[.*?\])'\) \}\}", job_block("native-export-compile"))
        self.assertIsNotNone(match, "native-export-compile must pick its matrix with fromJSON on the push-to-main predicate")
        predicate, on_main, elsewhere = match.groups()
        self.assertEqual(predicate, MAIN_PUSH)
        on_main_legs, other_legs = json.loads(on_main), json.loads(elsewhere)
        self.assertEqual([leg["os"] for leg in on_main_legs], ["ubuntu-latest"])
        self.assertEqual([leg["os"] for leg in other_legs], ["macos-15", "ubuntu-latest"])
        self.assertEqual(on_main_legs[0], other_legs[1], "the Linux leg must be identical on main and elsewhere")
        self.assertEqual(other_legs[0]["label"], "swiftc · macos-15", "the check name on pull requests must not change")
        self.assertEqual(other_legs[0]["targets"], "--target swift")

    def test_linux_jobs_do_not_mention_main(self) -> None:
        for job in ("changes", "check", "test", "lint", "fmt", "release-sync-tests", "codegen-validators"):
            self.assertNotIn("refs/heads/main", job_block(job), f"{job} must run on main as before")

    def test_shards_compile_without_debug_info_through_job_env(self) -> None:
        block = job_block("test-macos")
        env = re.search(r"^    env:\n((?:      .+\n)+)", block + "\n", re.MULTILINE)
        self.assertIsNotNone(env, "test-macos needs a job-level env so rust-cache hashes it into its key")
        self.assertIn('CARGO_PROFILE_DEV_DEBUG: "0"', env.group(1))
        self.assertIn('CARGO_PROFILE_TEST_DEBUG: "0"', env.group(1))


class AggregateTruthTable(unittest.TestCase):
    """Rows: (event, ref, draft, changes, rust, shards, lint) -> exit code and a message fragment."""

    ROWS = (
        # draft pull request
        ("pull_request", PR_REF, "true", "success", "true", "skipped", "skipped", 0, "skipped: draft"),
        ("pull_request", PR_REF, "true", "success", "true", "failure", "skipped", 1, "Draft PR shard matrix"),
        # pull request, path filter broken
        ("pull_request", PR_REF, "false", "failure", "", "skipped", "skipped", 1, "Path filter job"),
        # docs-only pull request
        ("pull_request", PR_REF, "false", "success", "false", "skipped", "skipped", 0, "skipped: docs-only"),
        ("pull_request", PR_REF, "false", "success", "false", "failure", "skipped", 1, "Docs-only PR shard matrix"),
        # pull request with rust changes
        ("pull_request", PR_REF, "false", "success", "true", "success", "success", 0, "All 3 Test (macos-15) shards"),
        ("pull_request", PR_REF, "false", "success", "true", "failure", "success", 1, "shards did not all succeed"),
        ("pull_request", PR_REF, "false", "success", "true", "cancelled", "success", 1, "shards did not all succeed"),
        ("pull_request", PR_REF, "false", "success", "true", "skipped", "success", 1, "shards did not all succeed"),
        ("pull_request", PR_REF, "false", "success", "true", "success", "failure", 1, "Lint budget (macos-15) did not succeed"),
        # push to main: the macOS jobs are skipped
        ("push", MAIN_REF, "", "success", "true", "skipped", "skipped", 0, "skipped on main: the pull request's checks covered this tree"),
        ("push", MAIN_REF, "", "success", "false", "skipped", "skipped", 0, "skipped on main: the pull request's checks covered this tree"),
        ("push", MAIN_REF, "", "failure", "", "skipped", "skipped", 1, "Path filter job"),
        # push to main where the macOS jobs ran anyway: their results still count
        ("push", MAIN_REF, "", "success", "true", "failure", "success", 1, "shards did not all succeed"),
        ("push", MAIN_REF, "", "success", "true", "success", "success", 0, "All 3 Test (macos-15) shards"),
        ("push", MAIN_REF, "", "success", "true", "skipped", "success", 1, "shards did not all succeed"),
        # train branch, merge group, release tag: unchanged, the shards must have passed
        ("push", TRAIN_REF, "", "success", "true", "success", "success", 0, "All 3 Test (macos-15) shards"),
        ("push", TRAIN_REF, "", "success", "true", "failure", "success", 1, "shards did not all succeed"),
        ("push", TRAIN_REF, "", "success", "true", "skipped", "skipped", 1, "shards did not all succeed"),
        ("merge_group", QUEUE_REF, "", "success", "true", "success", "success", 0, "All 3 Test (macos-15) shards"),
        ("merge_group", QUEUE_REF, "", "success", "true", "skipped", "skipped", 1, "shards did not all succeed"),
        ("push", TAG_REF, "", "success", "true", "success", "success", 0, "All 3 Test (macos-15) shards"),
        ("push", TAG_REF, "", "success", "true", "skipped", "skipped", 1, "shards did not all succeed"),
    )

    def test_every_row(self) -> None:
        for event, ref, draft, changes, rust, shards, lint, code, message in self.ROWS:
            label = f"{event} {ref} draft={draft!r} changes={changes} rust={rust!r} shards={shards} lint={lint}"
            with self.subTest(label):
                result = run_aggregate(event, ref, shards, lint, changes=changes, rust=rust, draft=draft)
                self.assertEqual(result.returncode, code, f"{label}\n{result.stdout}")
                self.assertIn(message, result.stdout, label)


if __name__ == "__main__":
    unittest.main()
