import json, subprocess, sys, tempfile, unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import pr_labels  # noqa: E402

SHA_A = "a" * 40
SHA_B = "b" * 40
REPO = "jangtrinh/design-os-studio"


def make_commits_pulls(
    *, pr_number: int = 1184, head_sha: str = SHA_A, head_repo: str = REPO,
) -> list[dict[str, object]]:
    return [{"number": pr_number, "head": {"sha": head_sha, "repo": {"full_name": head_repo}}}]


def make_pull(
    *, pr_number: int = 1184, head_sha: str = SHA_A, head_repo: str = REPO,
    additions: int = 10, deletions: int = 5, changed_files: int = 1,
) -> dict[str, object]:
    return {
        "number": pr_number,
        "head": {"sha": head_sha, "repo": {"full_name": head_repo}},
        "additions": additions,
        "deletions": deletions,
        "changed_files": changed_files,
    }


def make_files(paths: list[str | dict[str, object]] | None = None) -> list[dict[str, object]]:
    if paths is None:
        paths = ["README.md"]
    out: list[dict[str, object]] = []
    for entry in paths:
        if isinstance(entry, dict):
            out.append(entry)
        elif " => " in entry:
            prev, curr = entry.split(" => ", 1)
            out.append({"filename": curr, "status": "renamed", "previous_filename": prev})
        else:
            out.append({"filename": entry, "status": "modified"})
    return out


class PrLabelsTests(unittest.TestCase):
    def test_400_lines_adds_no_size_label_while_401_lines_adds_size_l(self) -> None:
        pulls = make_commits_pulls()
        files = make_files(["README.md"])
        self.assertEqual(
            pr_labels.evaluate_pr_labels(
                pulls=pulls, pull=make_pull(additions=250, deletions=150, changed_files=1),
                files=files, head_sha=SHA_A, head_repo=REPO,
            ),
            (1184, []),
        )
        self.assertEqual(
            pr_labels.evaluate_pr_labels(
                pulls=pulls, pull=make_pull(additions=250, deletions=151, changed_files=1),
                files=files, head_sha=SHA_A, head_repo=REPO,
            ),
            (1184, ["size/L"]),
        )

    def test_10_files_adds_no_size_label_while_11_files_adds_size_l(self) -> None:
        pulls = make_commits_pulls()
        ten = make_files([f"docs/p{i}.md" for i in range(10)])
        eleven = make_files([f"docs/p{i}.md" for i in range(11)])
        self.assertEqual(
            pr_labels.evaluate_pr_labels(
                pulls=pulls, pull=make_pull(changed_files=10),
                files=ten, head_sha=SHA_A, head_repo=REPO,
            ),
            (1184, []),
        )
        self.assertEqual(
            pr_labels.evaluate_pr_labels(
                pulls=pulls, pull=make_pull(changed_files=11),
                files=eleven, head_sha=SHA_A, head_repo=REPO,
            ),
            (1184, ["size/L"]),
        )

    def test_sensitive_paths_match_all_configured_categories(self) -> None:
        for path in [
            ".github/workflows/pr-gate.yml", "packaging/macos/Info.plist.in", "scripts/build-release.sh",
            "scripts/package.sh", "scripts/package-macos.sh", "scripts/release-smoke-macos.sh",
            "scripts/release-sync-pages.py", "crates/designos-mcp/src/main.rs", "crates/designos-eval/src/sandbox.rs",
            "crates/designos-app/src/agent_permissions.rs", "crates/designos-app/src/agent_sessions.rs",
            "crates/designos-app/src/agent_session_files.rs", "crates/designos-app/src/agent_session_failure.rs",
            "crates/designos-app/src/skills/hook_delivery.rs", "crates/designos-app/src/terminal.rs",
            "crates/designos-app/src/terminal/ask_mode_tests.rs", "crates/designos-app/src/terminal_undo_chord.rs",
            "crates/designos-app/src/shell/tests/terminal_undo_chord.rs", "skills/vendor/jakubkrehel-skills/rules.lock",
        ]:
            with self.subTest(path=path):
                self.assertTrue(pr_labels.is_sensitive_path(path))

    def test_rename_into_sensitive_path_triggers_sensitive_path_label(self) -> None:
        for renamed in [
            "docs/draft.yml => .github/workflows/pr-gate.yml",
            "crates/{designos-doc => designos-mcp}/src/lib.rs",
            "crates/designos-app/src/{notes.rs => agent_session_guard.rs}",
            "{docs => skills/vendor}/README.md",
        ]:
            with self.subTest(renamed=renamed):
                self.assertTrue(pr_labels.is_sensitive_path(renamed))

    def test_candidate_number_with_different_head_sha_is_refused(self) -> None:
        with self.assertRaises(ValueError):
            pr_labels.evaluate_pr_labels(
                pulls=[],
                pull=make_pull(pr_number=1184, head_sha=SHA_B, head_repo=REPO),
                files=make_files(["README.md"]),
                head_sha=SHA_A,
                head_repo=REPO,
                artifact={"pr_number": 1184, "head_sha": SHA_A},
            )

    def test_candidate_number_with_different_head_repo_is_refused(self) -> None:
        with self.assertRaises(ValueError):
            pr_labels.evaluate_pr_labels(
                pulls=[],
                pull=make_pull(pr_number=1184, head_sha=SHA_A, head_repo="fork-owner/design-os-studio"),
                files=make_files(["README.md"]),
                head_sha=SHA_A,
                head_repo=REPO,
                artifact={"pr_number": 1184, "head_sha": SHA_A},
            )

    def test_labels_come_from_files_list_ignoring_conflicting_artifact(self) -> None:
        lying_artifact = {
            "pr_number": 1,
            "head_sha": SHA_A,
            "additions": 1,
            "deletions": 0,
            "lines_changed": 1,
            "changed_files": 1,
            "paths": ["README.md"],
        }
        pull = make_pull(pr_number=1184, additions=401, deletions=0, changed_files=1)
        files = make_files([".github/workflows/pr-gate.yml"])
        self.assertEqual(
            pr_labels.evaluate_pr_labels(
                pulls=make_commits_pulls(pr_number=1184),
                pull=pull,
                files=files,
                head_sha=SHA_A,
                head_repo=REPO,
                artifact=lying_artifact,
            ),
            (1184, ["size/L", "sensitive-path"]),
        )
        candidate_lying_artifact = {**lying_artifact, "pr_number": 1184}
        self.assertEqual(
            pr_labels.evaluate_pr_labels(
                pulls=[],
                pull=pull,
                files=files,
                head_sha=SHA_A,
                head_repo=REPO,
                artifact=candidate_lying_artifact,
            ),
            (1184, ["size/L", "sensitive-path"]),
        )

    def test_rename_into_github_counts_as_one_file_and_is_sensitive(self) -> None:
        files = make_files(
            [{"filename": ".github/workflows/pr-gate.yml", "status": "renamed", "previous_filename": "docs/draft.yml"}]
            + [f"docs/p{i}.md" for i in range(9)]
        )
        paths = pr_labels.extract_file_paths(files)
        self.assertEqual(len(paths), 10)
        self.assertEqual(paths[0], "docs/draft.yml => .github/workflows/pr-gate.yml")
        self.assertEqual(
            pr_labels.evaluate_pr_labels(
                pulls=make_commits_pulls(),
                pull=make_pull(additions=20, deletions=5, changed_files=10),
                files=files,
                head_sha=SHA_A,
                head_repo=REPO,
            ),
            (1184, ["sensitive-path"]),
        )

    def test_files_list_shorter_than_changed_files_gives_both_labels(self) -> None:
        self.assertEqual(
            pr_labels.evaluate_pr_labels(
                pulls=make_commits_pulls(),
                pull=make_pull(additions=5, deletions=0, changed_files=3001),
                files=make_files(["README.md"]),
                head_sha=SHA_A,
                head_repo=REPO,
            ),
            (1184, ["size/L", "sensitive-path"]),
        )

    def test_no_pr_found_and_no_artifact_gives_no_labels_and_no_error(self) -> None:
        self.assertEqual(
            pr_labels.evaluate_pr_labels(pulls=[], head_sha=SHA_A, head_repo=REPO),
            (None, []),
        )
        with tempfile.TemporaryDirectory() as tmp:
            pulls_path = Path(tmp) / "pulls.json"
            pulls_path.write_text("[]\n", encoding="utf-8")
            proc = subprocess.run(
                [
                    sys.executable, "-B", str(Path(__file__).with_name("pr_labels.py")),
                    "--pulls", str(pulls_path),
                    "--head-sha", SHA_A,
                    "--head-repo", REPO,
                ],
                capture_output=True, text=True, check=False,
            )
            self.assertEqual(proc.returncode, 0)
            self.assertEqual(json.loads(proc.stdout), {"pr_number": None, "labels": []})

    def test_pr_found_with_no_artifact_is_still_labelled(self) -> None:
        pulls = make_commits_pulls(pr_number=1184)
        pull = make_pull(pr_number=1184, additions=50, deletions=10, changed_files=1)
        files = make_files([".github/workflows/pr-labeller.yml"])
        self.assertEqual(
            pr_labels.evaluate_pr_labels(
                pulls=pulls, pull=pull, files=files, head_sha=SHA_A, head_repo=REPO, artifact=None,
            ),
            (1184, ["sensitive-path"]),
        )
        with tempfile.TemporaryDirectory() as tmp:
            pulls_p, pull_p, files_p = Path(tmp) / "pulls.json", Path(tmp) / "pull.json", Path(tmp) / "files.json"
            pulls_p.write_text(json.dumps(pulls), encoding="utf-8")
            pull_p.write_text(json.dumps(pull), encoding="utf-8")
            files_p.write_text(json.dumps(files), encoding="utf-8")
            proc = subprocess.run(
                [
                    sys.executable, "-B", str(Path(__file__).with_name("pr_labels.py")),
                    "--pulls", str(pulls_p),
                    "--pull", str(pull_p),
                    "--files", str(files_p),
                    "--head-sha", SHA_A,
                    "--head-repo", REPO,
                ],
                capture_output=True, text=True, check=False,
            )
            self.assertEqual(proc.returncode, 0)
            self.assertEqual(json.loads(proc.stdout), {"pr_number": 1184, "labels": ["sensitive-path"]})

    def test_oversized_artifact_over_64_kib_is_refused(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            pulls_p = Path(tmp) / "pulls.json"
            art_p = Path(tmp) / "artifact.json"
            pulls_p.write_text("[]\n", encoding="utf-8")
            art_p.write_text(json.dumps({"pr_number": 1184, "pad": "x" * 65536}), encoding="utf-8")
            proc = subprocess.run(
                [
                    sys.executable, "-B", str(Path(__file__).with_name("pr_labels.py")),
                    "--pulls", str(pulls_p),
                    "--head-sha", SHA_A,
                    "--head-repo", REPO,
                    "--artifact", str(art_p),
                ],
                capture_output=True, text=True, check=False,
            )
            self.assertEqual(proc.returncode, 1)

    def test_head_label_resolves_fork_pr_when_commit_lookup_empty_and_no_artifact(self) -> None:
        fork_repo = "NamAIEngineer/design-os-studio"
        head_pulls = make_commits_pulls(pr_number=1175, head_sha=SHA_A, head_repo=fork_repo)
        pull = make_pull(
            pr_number=1175, head_sha=SHA_A, head_repo=fork_repo,
            additions=405, deletions=0, changed_files=1,
        )
        files = make_files([".github/workflows/pr-labeller.yml"])
        self.assertEqual(
            pr_labels.evaluate_pr_labels(
                head_pulls=head_pulls,
                pulls=[],
                pull=pull,
                files=files,
                head_sha=SHA_A,
                head_repo=fork_repo,
                artifact=None,
            ),
            (1175, ["size/L", "sensitive-path"]),
        )
        with tempfile.TemporaryDirectory() as tmp:
            hp_p = Path(tmp) / "head-pulls.json"
            pulls_p = Path(tmp) / "pulls.json"
            pull_p = Path(tmp) / "pull.json"
            files_p = Path(tmp) / "files.json"
            hp_p.write_text(json.dumps(head_pulls), encoding="utf-8")
            pulls_p.write_text("[]\n", encoding="utf-8")
            pull_p.write_text(json.dumps(pull), encoding="utf-8")
            files_p.write_text(json.dumps(files), encoding="utf-8")
            proc = subprocess.run(
                [
                    sys.executable, "-B", str(Path(__file__).with_name("pr_labels.py")),
                    "--head-pulls", str(hp_p),
                    "--pulls", str(pulls_p),
                    "--pull", str(pull_p),
                    "--files", str(files_p),
                    "--head-sha", SHA_A,
                    "--head-repo", fork_repo,
                ],
                capture_output=True, text=True, check=False,
            )
            self.assertEqual(proc.returncode, 0)
            self.assertEqual(
                json.loads(proc.stdout),
                {"pr_number": 1175, "labels": ["size/L", "sensitive-path"]},
            )

    def test_head_label_with_different_head_sha_or_head_repo_is_not_used(self) -> None:
        fork_repo = "NamAIEngineer/design-os-studio"
        mismatched_head_pulls = [
            *make_commits_pulls(pr_number=1175, head_sha=SHA_B, head_repo=fork_repo),
            *make_commits_pulls(pr_number=1176, head_sha=SHA_A, head_repo=REPO),
        ]
        self.assertIsNone(
            pr_labels.resolve_pr_number(
                head_pulls=mismatched_head_pulls,
                pulls=[],
                head_sha=SHA_A,
                head_repo=fork_repo,
            )
        )
        self.assertEqual(
            pr_labels.evaluate_pr_labels(
                head_pulls=mismatched_head_pulls,
                pulls=[],
                head_sha=SHA_A,
                head_repo=fork_repo,
            ),
            (None, []),
        )

    def test_lookup_order_head_label_then_commit_then_artifact(self) -> None:
        fork_repo = "NamAIEngineer/design-os-studio"
        head_pulls_match = make_commits_pulls(pr_number=1175, head_sha=SHA_A, head_repo=fork_repo)
        commits_pulls_match = make_commits_pulls(pr_number=1184, head_sha=SHA_A, head_repo=fork_repo)
        artifact_candidate = {"pr_number": 1190}
        files = make_files([".github/workflows/pr-labeller.yml"])

        # 1. Head label wins over commit lookup and artifact.
        self.assertEqual(
            pr_labels.resolve_pr_number(
                head_pulls=head_pulls_match,
                pulls=commits_pulls_match,
                head_sha=SHA_A,
                head_repo=fork_repo,
                artifact=artifact_candidate,
            ),
            1175,
        )
        self.assertEqual(
            pr_labels.evaluate_pr_labels(
                head_pulls=head_pulls_match,
                pulls=commits_pulls_match,
                pull=make_pull(pr_number=1175, head_sha=SHA_A, head_repo=fork_repo),
                files=files,
                head_sha=SHA_A,
                head_repo=fork_repo,
                artifact=artifact_candidate,
            ),
            (1175, ["sensitive-path"]),
        )

        # 2. Commit lookup wins over artifact when head label has no match.
        self.assertEqual(
            pr_labels.resolve_pr_number(
                head_pulls=[],
                pulls=commits_pulls_match,
                head_sha=SHA_A,
                head_repo=fork_repo,
                artifact=artifact_candidate,
            ),
            1184,
        )
        self.assertEqual(
            pr_labels.evaluate_pr_labels(
                head_pulls=[],
                pulls=commits_pulls_match,
                pull=make_pull(pr_number=1184, head_sha=SHA_A, head_repo=fork_repo),
                files=files,
                head_sha=SHA_A,
                head_repo=fork_repo,
                artifact=artifact_candidate,
            ),
            (1184, ["sensitive-path"]),
        )

        # 3. Artifact is used third only when both head label and commit lookup are empty.
        self.assertEqual(
            pr_labels.resolve_pr_number(
                head_pulls=[],
                pulls=[],
                head_sha=SHA_A,
                head_repo=fork_repo,
                artifact=artifact_candidate,
            ),
            1190,
        )
        self.assertEqual(
            pr_labels.evaluate_pr_labels(
                head_pulls=[],
                pulls=[],
                pull=make_pull(pr_number=1190, head_sha=SHA_A, head_repo=fork_repo),
                files=files,
                head_sha=SHA_A,
                head_repo=fork_repo,
                artifact=artifact_candidate,
            ),
            (1190, ["sensitive-path"]),
        )

    def test_two_head_label_matches_with_different_diffs_label_each_from_own_diff(self) -> None:
        fork_repo = "NamAIEngineer/design-os-studio"
        head_pulls = [
            *make_commits_pulls(pr_number=2001, head_sha=SHA_A, head_repo=fork_repo),
            *make_commits_pulls(pr_number=2000, head_sha=SHA_A, head_repo=fork_repo),
        ]
        pulls_by_number = {
            2001: make_pull(
                pr_number=2001, head_sha=SHA_A, head_repo=fork_repo,
                additions=5, deletions=0, changed_files=1,
            ),
            2000: make_pull(
                pr_number=2000, head_sha=SHA_A, head_repo=fork_repo,
                additions=500, deletions=0, changed_files=1,
            ),
        }
        files_by_number = {
            2001: make_files(["README.md"]),
            2000: make_files([".github/workflows/pr-gate.yml"]),
        }
        self.assertEqual(
            pr_labels.evaluate_pr_labels(
                head_pulls=head_pulls,
                pulls=[],
                pull=pulls_by_number[2001],
                files=files_by_number[2001],
                head_sha=SHA_A,
                head_repo=fork_repo,
            ),
            (2001, []),
        )
        self.assertEqual(
            pr_labels.evaluate_pr_labels(
                head_pulls=head_pulls,
                pulls=[],
                pull=pulls_by_number[2000],
                files=files_by_number[2000],
                head_sha=SHA_A,
                head_repo=fork_repo,
            ),
            (2000, ["size/L", "sensitive-path"]),
        )
        self.assertEqual(
            pr_labels.resolve_pr_numbers(
                head_pulls=head_pulls,
                pulls=[],
                head_sha=SHA_A,
                head_repo=fork_repo,
            ),
            [2001, 2000],
        )
        results, errors = pr_labels.evaluate_all_pr_labels(
            head_pulls=head_pulls,
            pulls=[],
            pulls_by_number=pulls_by_number,
            files_by_number=files_by_number,
            head_sha=SHA_A,
            head_repo=fork_repo,
        )
        self.assertEqual(errors, [])
        self.assertEqual(
            results,
            [
                (2001, []),
                (2000, ["size/L", "sensitive-path"]),
            ],
        )
        with tempfile.TemporaryDirectory() as tmp:
            tmp_dir = Path(tmp)
            hp_p = tmp_dir / "head-pulls.json"
            pulls_p = tmp_dir / "pulls.json"
            hp_p.write_text(json.dumps(head_pulls), encoding="utf-8")
            pulls_p.write_text("[]\n", encoding="utf-8")
            for num in (2001, 2000):
                (tmp_dir / f"pull-{num}.json").write_text(json.dumps(pulls_by_number[num]), encoding="utf-8")
                (tmp_dir / f"files-{num}.json").write_text(json.dumps(files_by_number[num]), encoding="utf-8")
            res_proc = subprocess.run(
                [
                    sys.executable, "-B", str(Path(__file__).with_name("pr_labels.py")),
                    "--resolve-pr",
                    "--head-pulls", str(hp_p),
                    "--pulls", str(pulls_p),
                    "--head-sha", SHA_A,
                    "--head-repo", fork_repo,
                ],
                capture_output=True, text=True, check=False,
            )
            self.assertEqual(res_proc.returncode, 0)
            self.assertEqual(res_proc.stdout.strip().splitlines(), ["2001", "2000"])
            batch_proc = subprocess.run(
                [
                    sys.executable, "-B", str(Path(__file__).with_name("pr_labels.py")),
                    "--head-pulls", str(hp_p),
                    "--pulls", str(pulls_p),
                    "--pulls-dir", str(tmp_dir),
                    "--head-sha", SHA_A,
                    "--head-repo", fork_repo,
                ],
                capture_output=True, text=True, check=False,
            )
            self.assertEqual(batch_proc.returncode, 0)
            self.assertEqual(
                json.loads(batch_proc.stdout),
                {
                    "results": [
                        {"pr_number": 2001, "labels": []},
                        {"pr_number": 2000, "labels": ["size/L", "sensitive-path"]},
                    ],
                    "errors": [],
                },
            )

    def test_head_label_list_with_one_matching_and_one_non_matching_labels_only_matching(self) -> None:
        fork_repo = "NamAIEngineer/design-os-studio"
        head_pulls = [
            *make_commits_pulls(pr_number=2001, head_sha=SHA_B, head_repo=fork_repo),
            *make_commits_pulls(pr_number=2000, head_sha=SHA_A, head_repo=fork_repo),
        ]
        pulls_by_number = {
            2001: make_pull(
                pr_number=2001, head_sha=SHA_B, head_repo=fork_repo,
                additions=500, deletions=0, changed_files=1,
            ),
            2000: make_pull(
                pr_number=2000, head_sha=SHA_A, head_repo=fork_repo,
                additions=20, deletions=0, changed_files=1,
            ),
        }
        files_by_number = {
            2001: make_files([".github/workflows/pr-gate.yml"]),
            2000: make_files([".github/workflows/pr-labeller.yml"]),
        }
        self.assertEqual(
            pr_labels.resolve_pr_numbers(
                head_pulls=head_pulls,
                pulls=[],
                head_sha=SHA_A,
                head_repo=fork_repo,
            ),
            [2000],
        )
        results, errors = pr_labels.evaluate_all_pr_labels(
            head_pulls=head_pulls,
            pulls=[],
            pulls_by_number=pulls_by_number,
            files_by_number=files_by_number,
            head_sha=SHA_A,
            head_repo=fork_repo,
        )
        self.assertEqual(errors, [])
        self.assertEqual(results, [(2000, ["sensitive-path"])])

    def test_fallback_uses_all_matches_from_commits_pulls_then_artifact(self) -> None:
        fork_repo = "NamAIEngineer/design-os-studio"
        commits_pulls = [
            *make_commits_pulls(pr_number=1184, head_sha=SHA_A, head_repo=fork_repo),
            *make_commits_pulls(pr_number=1185, head_sha=SHA_A, head_repo=fork_repo),
        ]
        self.assertEqual(
            pr_labels.resolve_pr_numbers(
                head_pulls=[],
                pulls=commits_pulls,
                head_sha=SHA_A,
                head_repo=fork_repo,
                artifact={"pr_number": 1190},
            ),
            [1184, 1185],
        )
        self.assertEqual(
            pr_labels.resolve_pr_numbers(
                head_pulls=[],
                pulls=[],
                head_sha=SHA_A,
                head_repo=fork_repo,
                artifact={"pr_number": 1190},
            ),
            [1190],
        )

    def test_one_failed_pr_validation_still_labels_rest_and_fails_job(self) -> None:
        fork_repo = "NamAIEngineer/design-os-studio"
        head_pulls = [
            *make_commits_pulls(pr_number=2001, head_sha=SHA_A, head_repo=fork_repo),
            *make_commits_pulls(pr_number=2000, head_sha=SHA_A, head_repo=fork_repo),
        ]
        pulls_by_number = {
            2001: make_pull(pr_number=2001, head_sha=SHA_B, head_repo=fork_repo),
            2000: make_pull(
                pr_number=2000, head_sha=SHA_A, head_repo=fork_repo,
                additions=500, deletions=0, changed_files=1,
            ),
        }
        files_by_number = {
            2001: make_files(["README.md"]),
            2000: make_files([".github/workflows/pr-gate.yml"]),
        }
        results, errors = pr_labels.evaluate_all_pr_labels(
            head_pulls=head_pulls,
            pulls=[],
            pulls_by_number=pulls_by_number,
            files_by_number=files_by_number,
            head_sha=SHA_A,
            head_repo=fork_repo,
        )
        self.assertEqual(results, [(2000, ["size/L", "sensitive-path"])])
        self.assertEqual(len(errors), 1)
        self.assertIn("2001", errors[0])

    def test_cap_at_20_matching_prs_labels_first_20_and_fails_naming_count(self) -> None:
        fork_repo = "NamAIEngineer/design-os-studio"
        pr_nums = list(range(1001, 1023))  # 22 matching PRs
        head_pulls = [
            entry
            for n in pr_nums
            for entry in make_commits_pulls(pr_number=n, head_sha=SHA_A, head_repo=fork_repo)
        ]
        pulls_by_number = {
            n: make_pull(pr_number=n, head_sha=SHA_A, head_repo=fork_repo, additions=10, deletions=0, changed_files=1)
            for n in pr_nums
        }
        files_by_number = {n: make_files([".github/workflows/pr-gate.yml"]) for n in pr_nums}
        results, errors = pr_labels.evaluate_all_pr_labels(
            head_pulls=head_pulls,
            pulls=[],
            pulls_by_number=pulls_by_number,
            files_by_number=files_by_number,
            head_sha=SHA_A,
            head_repo=fork_repo,
        )
        self.assertEqual([n for n, _ in results], pr_nums[:20])
        self.assertTrue(all(labels == ["sensitive-path"] for _, labels in results))
        self.assertEqual(len(errors), 1)
        self.assertIn("22", errors[0])

    def test_paginated_parser_merges_adjacent_and_newline_arrays_and_refuses_junk_or_non_array(self) -> None:
        self.assertEqual(
            pr_labels.parse_paginated_arrays('[{"id": 1}][{"id": 2}, {"id": 3}]'),
            [{"id": 1}, {"id": 2}, {"id": 3}],
        )
        self.assertEqual(
            pr_labels.parse_paginated_arrays('[{"id": 1}]\n[{"id": 2}]\n'),
            [{"id": 1}, {"id": 2}],
        )
        for bad in [
            '[{"id": 1}] trailing',
            '{"id": 1}',
            '[{"id": 1}]\n{"id": 2}',
            '123',
            '',
        ]:
            with self.subTest(bad=bad):
                with self.assertRaises(ValueError):
                    pr_labels.parse_paginated_arrays(bad)


if __name__ == "__main__":
    unittest.main()
