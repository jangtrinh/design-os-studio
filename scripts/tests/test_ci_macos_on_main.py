"""The macOS CI jobs skip a push to main, one cache seed job keeps the shards' dependency cache warm there, and the
`Test (macos-15)` aggregate reports the skip instead of failing.

The tests read `.github/workflows/ci.yml` as text (no PyYAML needed): they evaluate the `if:` of each macOS job for
every kind of event, run the aggregate job's shell script once per row of its truth table, check the swiftc leg of the
native export compile matrix, and check that the cache seed job builds the very rust-cache key the shards read.
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


def job_env(name: str) -> dict:
    """The job-level `env:` mapping (keys at 6 spaces, directly under a 4-space `env:`)."""
    match = re.search(r"^    env:\n((?:      .+\n)+)", job_block(name) + "\n", re.MULTILINE)
    if not match:
        return {}
    pairs = (line.strip().split(": ", 1) for line in match.group(1).splitlines())
    return {key: value.strip('"') for key, value in pairs}


def job_steps(name: str) -> list:
    """Each step as {"id", "name", "if", "uses", "run", "with": {...}}; comments and multi-line run bodies are skipped."""
    lines = job_block(name).splitlines()
    start = next(i for i, line in enumerate(lines) if line == "    steps:")
    steps: list = []
    in_with = False
    for line in lines[start + 1 :]:
        if not line.strip() or line.strip().startswith("#"):
            continue
        if line.startswith("      - "):
            steps.append({"with": {}})
            line = "        " + line[8:]
            in_with = False
        if not steps:
            continue
        indent = len(line) - len(line.lstrip())
        key, _, value = line.strip().partition(":")
        value = value.strip().strip('"')
        if indent == 8:
            in_with = key == "with"
            if not in_with:
                steps[-1][key] = value
        elif indent == 10 and in_with:
            steps[-1]["with"][key] = value
    return steps


def rust_cache_steps(name: str) -> list:
    return [step for step in job_steps(name) if step.get("uses", "").startswith("Swatinem/rust-cache@")]


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


class CacheSeedJob(unittest.TestCase):
    """main skips the shards, so one small macOS job keeps their rust-cache key populated: PRs can only read main's caches."""

    SEED = "cache-seed-macos"
    HIT = "steps.lookup.outputs.cache-hit != 'true'"

    def test_it_exists_under_a_name_nobody_mistakes_for_a_test_job(self) -> None:
        block = job_block(self.SEED)
        self.assertIn("name: Cache seed (macos-15)", block)
        self.assertNotIn("Test (macos-15", block)

    def test_it_runs_on_a_push_to_main_and_on_nothing_else(self) -> None:
        self.assertIn(MAIN_PUSH, job_if(self.SEED))
        self.assertNotIn(f"!({MAIN_PUSH})", job_if(self.SEED), "the seed job runs on the predicate, it does not negate it")
        for event, ref, expected in (
            ("push", MAIN_REF, True),
            ("pull_request", PR_REF, False),
            ("push", TRAIN_REF, False),
            ("push", TAG_REF, False),
            ("merge_group", QUEUE_REF, False),
        ):
            self.assertEqual(job_runs(self.SEED, event, ref), expected, f"{event} {ref}")
        self.assertFalse(job_runs(self.SEED, "push", MAIN_REF, rust="false"), "a docs-only push cannot change the dependency cache key")

    def test_it_is_not_part_of_the_required_aggregate(self) -> None:
        needs = re.search(r"^    needs: (.+)$", job_block("test-macos-required"), re.MULTILINE)
        self.assertIsNotNone(needs)
        self.assertNotIn(self.SEED, needs.group(1), "a failed seed job must not fail the Test (macos-15) check")
        self.assertNotIn(self.SEED, aggregate_script())

    def test_it_uses_the_shards_runner_toolchain_and_job_env(self) -> None:
        self.assertEqual(re.search(r"^    runs-on: (.+)$", job_block(self.SEED), re.MULTILINE).group(1), "macos-15")
        self.assertEqual(re.search(r"^    runs-on: (.+)$", job_block("test-macos"), re.MULTILINE).group(1), "macos-15")
        self.assertEqual(job_env(self.SEED), job_env("test-macos"), "CARGO* env feeds the rust-cache key, so it must be equal")
        self.assertTrue(job_env(self.SEED), "the debug-info env must be present")
        toolchain = lambda job: [s for s in job_steps(job) if s.get("uses", "").startswith("dtolnay/rust-toolchain@")]
        self.assertEqual(toolchain(self.SEED), toolchain("test-macos"))
        self.assertEqual(len(toolchain(self.SEED)), 1)

    def test_its_cache_inputs_equal_the_shards_inputs(self) -> None:
        (shard_cache,) = rust_cache_steps("test-macos")
        lookup, restore = rust_cache_steps(self.SEED)
        self.assertEqual(shard_cache["with"].get("shared-key"), "macos-test", "shards and seed job need one shared-key: the job id is part of the default key")
        self.assertEqual(restore["with"], shard_cache["with"])
        self.assertEqual({k: v for k, v in lookup["with"].items() if k not in ("lookup-only", "save-if")}, shard_cache["with"])
        self.assertEqual(lookup["uses"], shard_cache["uses"])
        self.assertEqual(restore["uses"], shard_cache["uses"])

    def test_the_lookup_comes_first_and_neither_downloads_nor_saves(self) -> None:
        lookup, _ = rust_cache_steps(self.SEED)
        steps = job_steps(self.SEED)
        self.assertEqual(steps.index(lookup), 2, "checkout, toolchain, then the lookup")
        self.assertEqual(lookup["id"], "lookup")
        self.assertEqual(lookup["with"].get("lookup-only"), "true")
        self.assertEqual(lookup["with"].get("save-if"), "false", "otherwise its post action saves a second time after the real one")

    def test_the_restore_and_the_compile_run_only_on_a_miss(self) -> None:
        _, restore = rust_cache_steps(self.SEED)
        steps = job_steps(self.SEED)
        self.assertEqual(restore["if"], self.HIT)
        self.assertNotIn("lookup-only", restore["with"])
        compile_step = steps[-1]
        self.assertEqual(compile_step["if"], self.HIT)
        self.assertEqual(compile_step["run"], "cargo test --workspace --locked --no-run")
        self.assertEqual([s for s in steps[3:] if "if" not in s], [], "every step after the lookup is conditional on the miss")

    def test_the_workflow_test_runs_in_the_linux_test_job_only(self) -> None:
        ours = [s for s in job_steps("test") if "test_ci_macos_on_main" in s.get("run", "")]
        self.assertEqual(len(ours), 1)
        self.assertEqual(ours[0]["if"], "runner.os == 'Linux'")
        for job in ("test-macos", "lint-budget", self.SEED, "test-macos-required"):
            self.assertNotIn("test_ci_macos_on_main", job_block(job), f"{job} must not run it")


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
