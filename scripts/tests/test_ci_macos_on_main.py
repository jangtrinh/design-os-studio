"""The macOS CI jobs skip a push to main (unless the train is live), the `Test (macos-15)` aggregate says so instead of
failing, a draft never ends green, and runs on main are never cancelled.

The tests read `.github/workflows/ci.yml` as text (see ci_workflow_reader.py): they evaluate the `if:` of each macOS job
for every kind of event, and run the aggregate job's shell script once per row of its truth table. The cache seed job
that keeps the shards' rust-cache warm on main is covered by test_ci_cache_seed_job.py.
"""

import json
import re
import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from ci_workflow_reader import (  # noqa: E402
    MAIN_PUSH,
    MAIN_REF,
    PR_REF,
    QUEUE_REF,
    TAG_REF,
    TRAIN_REF,
    concurrency,
    eval_expr,
    job_block,
    job_env,
    job_if,
    job_runs,
    job_steps,
    run_aggregate,
)

ALL_EVENTS = (
    ("pull_request", PR_REF),
    ("push", MAIN_REF),
    ("push", TRAIN_REF),
    ("push", TAG_REF),
    ("merge_group", QUEUE_REF),
)


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

    def test_a_live_train_brings_the_macos_jobs_back_on_main(self) -> None:
        """The train goes live with 'require branches to be up to date' off, so main's tree may be one no macOS job ran."""
        for job in ("test-macos", "lint-budget"):
            self.assertFalse(job_runs(job, "push", MAIN_REF, train_live=""), f"{job}: an unset variable is an empty string")
            self.assertFalse(job_runs(job, "push", MAIN_REF, train_live="false"))
            self.assertTrue(job_runs(job, "push", MAIN_REF, train_live="true"), f"{job} must run on main while TRAIN_LIVE is 'true'")

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
        self.assertTrue(eval_expr(predicate, "push", MAIN_REF, train_live=""))
        self.assertFalse(eval_expr(predicate, "push", MAIN_REF, train_live="true"), "a live train keeps the swiftc leg on main")

    def test_linux_jobs_do_not_mention_main(self) -> None:
        for job in ("changes", "check", "test", "lint", "fmt", "release-sync-tests", "codegen-validators"):
            self.assertNotIn("refs/heads/main", job_block(job), f"{job} must run on main as before")

    def test_shards_compile_without_debug_info_through_job_env(self) -> None:
        env = job_env("test-macos")
        self.assertEqual(env.get("CARGO_PROFILE_DEV_DEBUG"), "0", "test-macos needs a job-level env so rust-cache hashes it into its key")
        self.assertEqual(env.get("CARGO_PROFILE_TEST_DEBUG"), "0")


class RunsOnMainAreNeverCancelled(unittest.TestCase):
    """A cancelled cache seed saves nothing, so a push to main must not cancel the run before it."""

    def test_cancel_in_progress_is_off_on_main_and_on_elsewhere(self) -> None:
        expr = re.fullmatch(r"\$\{\{ (.+) \}\}", concurrency()["cancel-in-progress"])
        self.assertIsNotNone(expr, "cancel-in-progress must be an expression")
        for event, ref in ALL_EVENTS:
            self.assertEqual(eval_expr(expr.group(1), event, ref), ref != MAIN_REF, f"{event} {ref}")

    def test_the_group_still_holds_one_run_per_ref(self) -> None:
        """One group per ref: at most one run waits and a newer waiting run replaces an older one, so nothing queues up
        and a quick series of merges does not start several identical dependency compiles at once."""
        self.assertEqual(concurrency()["group"], "ci-${{ github.ref }}")


class TheWorkflowTestsRunInTheLinuxTestJob(unittest.TestCase):
    FILES = ("scripts/tests/test_ci_macos_on_main.py", "scripts/tests/test_ci_cache_seed_job.py")

    def test_they_run_there_and_in_no_macos_job(self) -> None:
        ours = [s for s in job_steps("test") if "test_ci_macos_on_main" in s.get("run", "")]
        self.assertEqual(len(ours), 1)
        self.assertEqual(ours[0]["if"], "runner.os == 'Linux'")
        for path in self.FILES:
            self.assertIn(path, ours[0]["run"])
        for job in ("test-macos", "lint-budget", "cache-seed-macos", "test-macos-required"):
            self.assertNotIn("test_ci_", job_block(job), f"{job} must not run them")


class AggregateTruthTable(unittest.TestCase):
    """Rows: (event, ref, draft, changes, rust, shards, lint, TRAIN_LIVE) -> exit code and a message fragment."""

    DRAFT = "because this pull request is a draft. Mark it ready for review to run them."
    COVERED = "skipped on main: the pull request's checks covered this tree"
    ALL_OK = "All 3 Test (macos-15) shards"
    SHARDS = "shards did not all succeed"
    ROWS = (
        # draft pull request: never green, whatever the results (the draft check comes first)
        ("pull_request", PR_REF, "true", "success", "true", "skipped", "skipped", "", 1, DRAFT),
        ("pull_request", PR_REF, "true", "success", "true", "success", "success", "", 1, DRAFT),
        ("pull_request", PR_REF, "true", "success", "true", "failure", "skipped", "", 1, DRAFT),
        ("pull_request", PR_REF, "true", "success", "false", "skipped", "skipped", "", 1, DRAFT),
        ("pull_request", PR_REF, "true", "failure", "", "skipped", "skipped", "", 1, DRAFT),
        # pull request, path filter broken
        ("pull_request", PR_REF, "false", "failure", "", "skipped", "skipped", "", 1, "Path filter job"),
        # docs-only pull request
        ("pull_request", PR_REF, "false", "success", "false", "skipped", "skipped", "", 0, "skipped: docs-only"),
        ("pull_request", PR_REF, "false", "success", "false", "failure", "skipped", "", 1, "Docs-only PR shard matrix"),
        # pull request with rust changes
        ("pull_request", PR_REF, "false", "success", "true", "success", "success", "", 0, ALL_OK),
        ("pull_request", PR_REF, "false", "success", "true", "failure", "success", "", 1, SHARDS),
        ("pull_request", PR_REF, "false", "success", "true", "cancelled", "success", "", 1, SHARDS),
        ("pull_request", PR_REF, "false", "success", "true", "skipped", "success", "", 1, SHARDS),
        ("pull_request", PR_REF, "false", "success", "true", "success", "failure", "", 1, "Lint budget (macos-15) did not succeed"),
        # push to main with the train not live: the macOS jobs are skipped
        ("push", MAIN_REF, "", "success", "true", "skipped", "skipped", "", 0, COVERED),
        ("push", MAIN_REF, "", "success", "false", "skipped", "skipped", "", 0, COVERED),
        ("push", MAIN_REF, "", "success", "true", "skipped", "skipped", "false", 0, COVERED),
        ("push", MAIN_REF, "", "failure", "", "skipped", "skipped", "", 1, "Path filter job"),
        # push to main where the macOS jobs ran anyway: their results still count
        ("push", MAIN_REF, "", "success", "true", "failure", "success", "", 1, SHARDS),
        ("push", MAIN_REF, "", "success", "true", "success", "success", "", 0, ALL_OK),
        ("push", MAIN_REF, "", "success", "true", "skipped", "success", "", 1, SHARDS),
        # push to main with the train live: the macOS jobs run and are required to pass; "covered" is never claimed
        ("push", MAIN_REF, "", "success", "true", "success", "success", "true", 0, ALL_OK),
        ("push", MAIN_REF, "", "success", "true", "failure", "success", "true", 1, SHARDS),
        ("push", MAIN_REF, "", "success", "true", "skipped", "skipped", "true", 1, SHARDS),
        ("push", MAIN_REF, "", "success", "true", "skipped", "skipped", "TRUE", 1, SHARDS),
        ("push", MAIN_REF, "", "success", "false", "skipped", "skipped", "true", 0, "skipped: docs-only"),
        # train branch, merge group, release tag: unchanged, the shards must have passed
        ("push", TRAIN_REF, "", "success", "true", "success", "success", "", 0, ALL_OK),
        ("push", TRAIN_REF, "", "success", "true", "failure", "success", "", 1, SHARDS),
        ("push", TRAIN_REF, "", "success", "true", "skipped", "skipped", "", 1, SHARDS),
        ("merge_group", QUEUE_REF, "", "success", "true", "success", "success", "", 0, ALL_OK),
        ("merge_group", QUEUE_REF, "", "success", "true", "skipped", "skipped", "", 1, SHARDS),
        ("push", TAG_REF, "", "success", "true", "success", "success", "", 0, ALL_OK),
        ("push", TAG_REF, "", "success", "true", "skipped", "skipped", "", 1, SHARDS),
    )

    def test_every_row(self) -> None:
        for event, ref, draft, changes, rust, shards, lint, train_live, code, message in self.ROWS:
            label = f"{event} {ref} draft={draft!r} changes={changes} rust={rust!r} shards={shards} lint={lint} TRAIN_LIVE={train_live!r}"
            with self.subTest(label):
                result = run_aggregate(event, ref, shards, lint, changes=changes, rust=rust, draft=draft, train_live=train_live)
                self.assertEqual(result.returncode, code, f"{label}\n{result.stdout}")
                self.assertIn(message, result.stdout, label)

    def test_no_draft_row_is_green(self) -> None:
        self.assertEqual([row for row in self.ROWS if row[2] == "true" and row[8] == 0], [])


if __name__ == "__main__":
    unittest.main()
