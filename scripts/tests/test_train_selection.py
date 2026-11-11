"""Unit tests for scripts/ci/train.sh selection, ordering, dry-run, and red-batch crate attribution.

Run:
    python3 -m unittest discover -s scripts/tests -p 'test_train*' -v
"""
import json
import os
import subprocess
import tempfile
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent
REPO_ROOT = HERE.parent.parent
TRAIN_SH = REPO_ROOT / "scripts" / "ci" / "train.sh"

REQUIRED_CHECKS = ("Check", "Test (macos-15)", "Clippy", "Rustfmt")


def make_checks(overrides=None, extra=None):
    """Build a canned statusCheckRollup array with all 4 required checks green by default."""
    overrides = overrides or {}
    rollup = []
    for name in REQUIRED_CHECKS:
        spec = overrides.get(name, ("COMPLETED", "SUCCESS"))
        if spec is None:
            continue
        status, conclusion = spec
        rollup.append(
            {
                "__typename": "CheckRun",
                "name": name,
                "status": status,
                "conclusion": conclusion,
            }
        )
    if extra:
        rollup.extend(extra)
    return rollup


def make_pr(
    number,
    *,
    title=None,
    state="OPEN",
    is_draft=False,
    base="main",
    head=None,
    oid=None,
    created_at=None,
    labels=("train:ready",),
    body="Closes #1",
    mergeable="MERGEABLE",
    commits=None,
    checks=None,
):
    return {
        "number": number,
        "title": title or f"feat: change #{number}",
        "state": state,
        "isDraft": is_draft,
        "baseRefName": base,
        "headRefName": head or f"feat/pr-{number}",
        "headRefOid": oid or f"{number:040x}",
        "createdAt": created_at or f"2026-11-11T00:{number % 60:02d}:00Z",
        "labels": [{"name": lbl} for lbl in labels],
        "body": body,
        "mergeable": mergeable,
        "commits": commits or [],
        "statusCheckRollup": make_checks() if checks is None else checks,
    }


def run_select(prs, max_batch=5):
    proc = subprocess.run(
        ["bash", str(TRAIN_SH), "--max-batch", str(max_batch), "--select-json", "-"],
        input=json.dumps(prs),
        capture_output=True,
        text=True,
        cwd=str(REPO_ROOT),
        check=False,
    )
    if proc.returncode != 0:
        raise AssertionError(f"train.sh exited {proc.returncode}:\nstdout={proc.stdout}\nstderr={proc.stderr}")
    return json.loads(proc.stdout)


def run_attribute(failing_crates, prs):
    payload = {"failing_crates": failing_crates, "prs": prs}
    proc = subprocess.run(
        ["bash", str(TRAIN_SH), "--attribute-failure-json", "-"],
        input=json.dumps(payload),
        capture_output=True,
        text=True,
        cwd=str(REPO_ROOT),
        check=False,
    )
    if proc.returncode != 0:
        raise AssertionError(f"train.sh exited {proc.returncode}:\nstdout={proc.stdout}\nstderr={proc.stderr}")
    return json.loads(proc.stdout)


class TrainSelectionTests(unittest.TestCase):
    def test_draft_skipped(self):
        prs = [
            make_pr(101, is_draft=True, created_at="2026-11-11T01:00:00Z"),
            make_pr(102, is_draft=False, created_at="2026-11-11T02:00:00Z"),
        ]
        res = run_select(prs)
        self.assertEqual([p["number"] for p in res["selected"]], [102])
        skipped = {p["number"]: p["skip_reason"] for p in res["skipped"]}
        self.assertEqual(skipped.get(101), "draft")

    def test_stacked_skipped_by_base_branch_body_and_commit_oid(self):
        base_oid = "a" * 40
        prs = [
            # Open base PR (not train:ready yet)
            make_pr(200, labels=(), oid=base_oid, head="feat/base-200", created_at="2026-11-11T00:10:00Z"),
            # Stacked via non-main baseRefName
            make_pr(201, base="feat/base-200", created_at="2026-11-11T00:20:00Z"),
            # Stacked via body "Depends on #200"
            make_pr(202, body="Closes #2\nDepends on #200", created_at="2026-11-11T00:30:00Z"),
            # Stacked via carrying open PR #200's head commit in its commits list
            make_pr(203, commits=[{"oid": base_oid}, {"oid": "b" * 40}], created_at="2026-11-11T00:40:00Z"),
            # Independent PR targeting main
            make_pr(204, created_at="2026-11-11T00:50:00Z"),
        ]
        res = run_select(prs)
        self.assertEqual([p["number"] for p in res["selected"]], [204])
        skipped = {p["number"]: p["skip_reason"] for p in res["skipped"]}
        self.assertIn("stacked:", skipped[201])
        self.assertIn("stacked:", skipped[202])
        self.assertIn("stacked:", skipped[203])

    def test_merged_dependency_does_not_block_pr_on_main(self):
        prs = [
            make_pr(300, state="MERGED", oid="c" * 40, created_at="2026-11-11T00:10:00Z"),
            make_pr(301, body="Depends on #300", created_at="2026-11-11T00:20:00Z"),
        ]
        res = run_select(prs)
        self.assertEqual([p["number"] for p in res["selected"]], [301])

    def test_solo_runs_alone_by_label_or_release_prefix(self):
        # 1) Explicit train:solo label runs alone ahead of older normal PRs.
        prs_label = [
            make_pr(401, created_at="2026-11-11T01:00:00Z"),
            make_pr(402, labels=("train:ready", "train:solo"), created_at="2026-11-11T03:00:00Z"),
            make_pr(403, created_at="2026-11-11T02:00:00Z"),
        ]
        res_label = run_select(prs_label)
        self.assertEqual([p["number"] for p in res_label["selected"]], [402])

        # 2) Title starting with `release:` also runs alone.
        prs_release = [
            make_pr(411, created_at="2026-11-11T01:00:00Z"),
            make_pr(412, title="release: 0.7.13", labels=("train:ready",), created_at="2026-11-11T04:00:00Z"),
        ]
        res_release = run_select(prs_release)
        self.assertEqual([p["number"] for p in res_release["selected"]], [412])

    def test_hotfix_ordered_first_and_never_merges_with_solo(self):
        prs = [
            make_pr(501, created_at="2026-11-11T01:00:00Z"),
            make_pr(502, created_at="2026-11-11T02:00:00Z"),
            make_pr(503, labels=("train:ready", "train:solo"), created_at="2026-11-11T02:30:00Z"),
            make_pr(504, labels=("train:ready", "hotfix"), created_at="2026-11-11T05:00:00Z"),
            make_pr(505, labels=("train:ready", "hotfix"), created_at="2026-11-11T04:00:00Z"),
        ]
        res = run_select(prs)
        # Hotfix PRs (#505 older, then #504 newer) come first, followed by normal PRs (#501, #502);
        # #503 (train:solo) is excluded from the batch so it can run alone on the next cycle.
        self.assertEqual([p["number"] for p in res["selected"]], [505, 504, 501, 502])

    def test_cap_5_and_oldest_first_ordering(self):
        prs = [
            make_pr(606, created_at="2026-11-11T06:00:00Z"),
            make_pr(602, created_at="2026-11-11T02:00:00Z"),
            make_pr(607, created_at="2026-11-11T07:00:00Z"),
            make_pr(601, created_at="2026-11-11T01:00:00Z"),
            make_pr(605, created_at="2026-11-11T05:00:00Z"),
            make_pr(603, created_at="2026-11-11T03:00:00Z"),
            make_pr(604, created_at="2026-11-11T04:00:00Z"),
        ]
        res_default = run_select(prs)
        self.assertEqual([p["number"] for p in res_default["selected"]], [601, 602, 603, 604, 605])

        res_cap3 = run_select(prs, max_batch=3)
        self.assertEqual([p["number"] for p in res_cap3["selected"]], [601, 602, 603])

    def test_red_or_pending_own_required_checks_skipped(self):
        prs = [
            # Red Test (macos-15)
            make_pr(
                701,
                checks=make_checks({"Test (macos-15)": ("COMPLETED", "FAILURE")}),
                created_at="2026-11-11T01:00:00Z",
            ),
            # Red Clippy
            make_pr(
                702,
                checks=make_checks({"Clippy": ("COMPLETED", "FAILURE")}),
                created_at="2026-11-11T02:00:00Z",
            ),
            # Pending Check
            make_pr(
                703,
                checks=make_checks({"Check": ("IN_PROGRESS", "")}),
                created_at="2026-11-11T03:00:00Z",
            ),
            # Missing Rustfmt
            make_pr(
                704,
                checks=make_checks({"Rustfmt": None}),
                created_at="2026-11-11T04:00:00Z",
            ),
            # Non-required Ubuntu check failed, but all 4 required checks succeeded (or skipped)
            make_pr(
                705,
                checks=make_checks(
                    {"Test (macos-15)": ("COMPLETED", "SKIPPED")},
                    extra=[
                        {
                            "__typename": "CheckRun",
                            "name": "Test (ubuntu-latest)",
                            "status": "COMPLETED",
                            "conclusion": "FAILURE",
                        }
                    ],
                ),
                created_at="2026-11-11T05:00:00Z",
            ),
        ]
        res = run_select(prs)
        self.assertEqual([p["number"] for p in res["selected"]], [705])
        skipped = {p["number"]: p["skip_reason"] for p in res["skipped"]}
        self.assertIn("FAIL:Test (macos-15)", skipped[701])
        self.assertIn("FAIL:Clippy", skipped[702])
        self.assertIn("PENDING:Check", skipped[703])
        self.assertIn("PENDING:Rustfmt", skipped[704])

    def test_conflict_and_failed_labels_skipped(self):
        prs = [
            make_pr(801, labels=("train:ready", "train:conflict"), created_at="2026-11-11T01:00:00Z"),
            make_pr(802, labels=("train:ready", "train:failed"), created_at="2026-11-11T02:00:00Z"),
            make_pr(803, mergeable="CONFLICTING", created_at="2026-11-11T03:00:00Z"),
            make_pr(804, created_at="2026-11-11T04:00:00Z"),
        ]
        res = run_select(prs)
        self.assertEqual([p["number"] for p in res["selected"]], [804])

    def test_red_batch_crate_intersection_attributes_single_culprit_only(self):
        batch_prs = [
            {"number": 901, "files": ["crates/designos-doc/src/lib.rs"]},
            {"number": 902, "files": ["crates/designos-export/src/html.rs", "docs/releasing.md"]},
            {"number": 903, "files": ["crates/designos-gpu/src/compile.rs"]},
        ]
        # Exactly one PR (#902) touches designos-export -> culprit is 902.
        single = run_attribute(["designos-export"], batch_prs)
        self.assertEqual(single["culprit"], 902)
        self.assertEqual(single["intersecting_prs"], [902])

        # Two PRs touch failing crates -> culprit is None (label all and stop).
        multi = run_attribute(["designos-doc", "designos-gpu"], batch_prs)
        self.assertIsNone(multi["culprit"])
        self.assertEqual(multi["intersecting_prs"], [901, 903])

        # No PR touches failing crate -> culprit is None.
        none_match = run_attribute(["designos-mcp"], batch_prs)
        self.assertIsNone(none_match["culprit"])
        self.assertEqual(none_match["intersecting_prs"], [])

    def test_dry_run_mode_prints_plan_and_never_pushes(self):
        prs = [
            make_pr(1001, title="fix(doc): first fix", created_at="2026-11-11T01:00:00Z"),
            make_pr(1002, title="fix(hot): urgent fix", labels=("train:ready", "hotfix"), created_at="2026-11-11T02:00:00Z"),
        ]
        with tempfile.NamedTemporaryFile("w", suffix=".json", delete=False) as tf:
            json.dump(prs, tf)
            tf_path = tf.name
        try:
            env = {**os.environ, "DRY_RUN": "1", "PR_LIST_JSON_FILE": tf_path}
            proc = subprocess.run(
                ["bash", str(TRAIN_SH)],
                capture_output=True,
                text=True,
                cwd=str(REPO_ROOT),
                env=env,
                check=False,
            )
            self.assertEqual(proc.returncode, 0, proc.stderr)
            self.assertIn("selected 2 PR(s) in order: #1002 #1001", proc.stdout)
            self.assertIn("[dry-run] would run: git merge --no-ff", proc.stdout)
            self.assertIn("landed in <sha> with #1002 #1001", proc.stdout)
        finally:
            Path(tf_path).unlink(missing_ok=True)


if __name__ == "__main__":
    unittest.main()
