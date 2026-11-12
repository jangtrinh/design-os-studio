"""The cache seed job writes the very rust-cache keys the macOS shards and the lint budget job read.

A pull request can restore caches only from its own ref or from main, and neither macOS job runs on main, so one seed
job per cache keeps them warm. Each leg must see the same runner, toolchain step, key-feeding environment and cache
inputs as the job it serves, and must compile the same dependency graph. These tests compare the workflow text.
"""

import re
import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from ci_workflow_reader import (  # noqa: E402
    KEY_ENV_PREFIXES,
    MAIN_PUSH,
    MAIN_REF,
    PR_REF,
    QUEUE_REF,
    TAG_REF,
    TRAIN_REF,
    aggregate_script,
    job_block,
    job_env,
    job_field,
    job_if,
    job_runs,
    job_steps,
    matrix_legs,
    resolve,
    rust_cache_steps,
)

SEED = "cache-seed-macos"
HIT = "steps.lookup.outputs.cache-hit != 'true'"
SERVED = {"shards": "test-macos", "lint budget": "lint-budget"}
FEATURE_FLAGS = ("--all-features", "--features", "--no-default-features", "-F")


def legs() -> list:
    return matrix_legs(SEED)


def seed_steps(leg: dict) -> list:
    """The seed job's steps with the matrix values of one leg filled in."""
    return [{**step, "with": {k: resolve(v, leg) for k, v in step["with"].items()}, **({"run": resolve(step["run"], leg)} if "run" in step else {})} for step in job_steps(SEED)]


def steps_before_cache(job: str) -> list:
    steps = job_steps(job)
    return steps[: next(i for i, s in enumerate(steps) if s.get("uses", "").startswith("Swatinem/rust-cache@"))]


def key_env_names(job: str) -> set:
    """Variables that feed the rust-cache key and are set by a step before the cache step: step `env:` and writes to GITHUB_ENV."""
    names = set()
    for step in steps_before_cache(job):
        names |= {name for name in step["env"] if name.startswith(KEY_ENV_PREFIXES)}
        for line in step.get("run", "").splitlines():
            if "GITHUB_ENV" in line:
                names |= {name for name in re.findall(r"\b([A-Z][A-Z0-9_]*)=", line) if name.startswith(KEY_ENV_PREFIXES)}
    return names


def cargo_commands(job: str) -> list:
    return [line.strip() for step in job_steps(job) for line in step.get("run", "").splitlines() if line.strip().startswith("cargo ")]


def feature_flags(commands: list) -> set:
    return {tok.split("=")[0] for cmd in commands for tok in cmd.split() if tok.split("=")[0] in FEATURE_FLAGS}


class CacheSeedJob(unittest.TestCase):
    def test_it_is_two_legs_with_names_nobody_mistakes_for_a_test_job(self) -> None:
        self.assertIn("name: Cache seed (macos-15, ${{ matrix.leg }})", job_block(SEED))
        self.assertEqual([leg["leg"] for leg in legs()], list(SERVED))
        self.assertNotIn("Test (macos-15", job_block(SEED))
        self.assertEqual(len({leg["shared-key"] for leg in legs()}), 2, "each cache needs its own key")

    def test_it_runs_on_a_push_to_main_while_the_train_is_not_live_and_on_nothing_else(self) -> None:
        self.assertIn(MAIN_PUSH, job_if(SEED))
        self.assertNotIn(f"!({MAIN_PUSH})", job_if(SEED), "the seed job runs on the predicate, it does not negate it")
        for event, ref, expected in (
            ("push", MAIN_REF, True),
            ("pull_request", PR_REF, False),
            ("push", TRAIN_REF, False),
            ("push", TAG_REF, False),
            ("merge_group", QUEUE_REF, False),
        ):
            self.assertEqual(job_runs(SEED, event, ref), expected, f"{event} {ref}")
        self.assertFalse(job_runs(SEED, "push", MAIN_REF, rust="false"), "a docs-only push cannot change a dependency cache key")
        self.assertFalse(job_runs(SEED, "push", MAIN_REF, train_live="true"), "with the train live the shards run on main and write the cache themselves")

    def test_it_is_not_part_of_the_required_aggregate(self) -> None:
        self.assertNotIn(SEED, job_field("test-macos-required", "needs"), "a failed seed job must not fail the Test (macos-15) check")
        self.assertNotIn(SEED, aggregate_script())

    def test_each_leg_matches_the_runner_toolchain_and_key_env_of_the_job_it_serves(self) -> None:
        toolchain = lambda job: [s for s in job_steps(job) if s.get("uses", "").startswith("dtolnay/rust-toolchain@")]
        for leg in legs():
            served = SERVED[leg["leg"]]
            with self.subTest(leg["leg"]):
                self.assertEqual(job_field(SEED, "runs-on"), "macos-15")
                self.assertEqual(job_field(served, "runs-on"), "macos-15")
                self.assertEqual(job_env(SEED), job_env(served), "CARGO* env feeds the rust-cache key, so it must be equal")
                self.assertTrue(job_env(SEED), "the debug-info env must be present")
                self.assertEqual(toolchain(SEED), toolchain(served))
                self.assertEqual(len(toolchain(SEED)), 1)

    def test_each_leg_reads_and_writes_exactly_the_cache_inputs_of_the_job_it_serves(self) -> None:
        for leg in legs():
            served = SERVED[leg["leg"]]
            with self.subTest(leg["leg"]):
                (served_cache,) = rust_cache_steps(served)
                self.assertEqual(served_cache["with"].get("shared-key"), leg["shared-key"], "the job id is part of the default key, so both sides name one shared-key")
                lookup, restore = [s for s in seed_steps(leg) if s.get("uses", "").startswith("Swatinem/rust-cache@")]
                self.assertEqual(restore["with"], served_cache["with"])
                self.assertEqual({k: v for k, v in lookup["with"].items() if k not in ("lookup-only", "save-if")}, served_cache["with"])
                self.assertEqual(lookup["uses"], served_cache["uses"])
                self.assertEqual(restore["uses"], served_cache["uses"])

    def test_the_lookup_comes_first_and_neither_downloads_nor_saves(self) -> None:
        steps = job_steps(SEED)
        lookup = rust_cache_steps(SEED)[0]
        self.assertEqual(steps.index(lookup), 2, "checkout, toolchain, then the lookup")
        self.assertEqual(lookup["id"], "lookup")
        self.assertEqual(lookup["with"].get("lookup-only"), "true")
        self.assertEqual(lookup["with"].get("save-if"), "false", "otherwise its post action saves a second time after the real one")

    def test_the_restore_and_the_compile_run_only_on_a_miss(self) -> None:
        steps = job_steps(SEED)
        restore = rust_cache_steps(SEED)[1]
        self.assertEqual(restore["if"], HIT)
        self.assertNotIn("lookup-only", restore["with"])
        self.assertEqual(steps[-1]["if"], HIT)
        self.assertEqual(steps[-1]["run"], "${{ matrix.compile }}")
        self.assertEqual([s for s in steps[3:] if "if" not in s], [], "every step after the lookup is conditional on the miss")

    def test_the_shards_leg_compiles_the_shards_dependency_graph(self) -> None:
        """nextest builds with `cargo test --no-run`; --profile and --partition only choose and filter tests."""
        compile_cmd = next(leg for leg in legs() if leg["leg"] == "shards")["compile"].split()
        self.assertEqual(compile_cmd[:2], ["cargo", "test"])
        nextest = re.sub(r"\$\{\{.*?\}\}", "N", next(c for c in cargo_commands("test-macos") if c.startswith("cargo nextest run"))).split()[3:]
        build_flags = []
        skip = False
        for token in nextest:
            if skip:
                skip = False
            elif token in ("--profile", "--partition"):
                skip = True
            else:
                build_flags.append(token)
        self.assertEqual(sorted(compile_cmd[2:]), sorted(build_flags + ["--no-run"]))

    def test_the_lint_leg_compiles_what_the_lint_job_runs(self) -> None:
        compile_cmd = next(leg for leg in legs() if leg["leg"] == "lint budget")["compile"].split()
        (served_cmd,) = cargo_commands("lint-budget")
        served = served_cmd.split()
        self.assertEqual(compile_cmd[-1], "--no-run")
        self.assertEqual(compile_cmd[:-1], served[: len(compile_cmd) - 1], "same package, profile and target as the lint job, minus the test filter")
        self.assertIn("--release", compile_cmd)

    def test_no_feature_flag_on_a_served_job_is_missing_from_its_leg(self) -> None:
        for leg in legs():
            served = SERVED[leg["leg"]]
            with self.subTest(leg["leg"]):
                self.assertEqual(feature_flags(cargo_commands(served)), feature_flags([leg["compile"]]), "a feature flag changes the dependency graph")

    def test_no_step_before_the_cache_sets_a_key_variable_the_other_job_lacks(self) -> None:
        for leg in legs():
            served = SERVED[leg["leg"]]
            with self.subTest(leg["leg"]):
                self.assertEqual(key_env_names(served), key_env_names(SEED), "RUSTFLAGS and CARGO* set before rust-cache change the key")


if __name__ == "__main__":
    unittest.main()
