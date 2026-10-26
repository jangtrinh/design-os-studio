import subprocess, sys, tempfile, unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import pr_labels  # noqa: E402


def make_artifact(
    *, pr_number: int = 1163, head_sha: str = "a" * 40, additions: int = 10, deletions: int = 5,
    changed_files: int | None = None, paths: list[str] | None = None,
) -> dict[str, object]:
    paths = ["README.md"] if paths is None else paths
    return {
        "pr_number": pr_number, "head_sha": head_sha, "additions": additions, "deletions": deletions,
        "lines_changed": additions + deletions, "changed_files": len(paths) if changed_files is None else changed_files,
        "paths": paths,
    }


class PrLabelsTests(unittest.TestCase):
    def test_400_lines_adds_no_size_label_while_401_lines_adds_size_l(self) -> None:
        self.assertEqual(pr_labels.labels_for_artifact(make_artifact(additions=250, deletions=150)), [])
        self.assertEqual(pr_labels.labels_for_artifact(make_artifact(additions=250, deletions=151)), ["size/L"])

    def test_10_files_adds_no_size_label_while_11_files_adds_size_l(self) -> None:
        self.assertEqual(pr_labels.labels_for_artifact(make_artifact(paths=[f"docs/p{i}.md" for i in range(10)])), [])
        self.assertEqual(pr_labels.labels_for_artifact(make_artifact(paths=[f"docs/p{i}.md" for i in range(11)])), ["size/L"])

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
                self.assertEqual(pr_labels.labels_for_artifact(make_artifact(paths=[path])), ["sensitive-path"])

    def test_rename_into_sensitive_path_triggers_sensitive_path_label(self) -> None:
        for renamed in [
            "docs/draft.yml => .github/workflows/pr-gate.yml",
            "crates/{designos-doc => designos-mcp}/src/lib.rs",
            "crates/designos-app/src/{notes.rs => agent_session_guard.rs}",
            "{docs => skills/vendor}/README.md",
        ]:
            with self.subTest(renamed=renamed):
                self.assertEqual(pr_labels.labels_for_artifact(make_artifact(paths=[renamed])), ["sensitive-path"])

    def test_missing_artifact_exits_zero_and_outputs_nothing(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            missing = Path(tmp) / "pr-gate-result.json"
            self.assertEqual(pr_labels.labels_from_artifact_path(missing), [])
            proc = subprocess.run(
                [sys.executable, "-B", str(Path(__file__).with_name("pr_labels.py")), str(missing)],
                capture_output=True, text=True, check=False,
            )
            self.assertEqual((proc.returncode, proc.stdout), (0, ""))

    def test_malformed_artifact_is_rejected(self) -> None:
        for bad in [
            "not-an-object", {}, {**make_artifact(), "pr_number": 0}, {**make_artifact(), "pr_number": True},
            {**make_artifact(), "head_sha": "short"}, {**make_artifact(), "additions": -1},
            {**make_artifact(), "lines_changed": 999}, {**make_artifact(), "changed_files": 5},
            {**make_artifact(), "paths": [""], "changed_files": 1},
        ]:
            with self.subTest(bad=bad):
                with self.assertRaises(ValueError):
                    pr_labels.labels_for_artifact(bad)
        with tempfile.TemporaryDirectory() as tmp:
            bad_json = Path(tmp) / "bad.json"
            bad_json.write_text("{not valid json", encoding="utf-8")
            proc = subprocess.run(
                [sys.executable, "-B", str(Path(__file__).with_name("pr_labels.py")), str(bad_json)],
                capture_output=True, text=True, check=False,
            )
            self.assertNotEqual(proc.returncode, 0)


if __name__ == "__main__":
    unittest.main()
