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


if __name__ == "__main__":
    unittest.main()
