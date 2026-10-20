"""Tests for scripts/release-sync-pages.py. Run: python3 -m unittest discover -s scripts/tests -v"""
import importlib.util
import re
import shutil
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

HERE = Path(__file__).parent
FIX = HERE / "fixtures" / "release-sync"
SCRIPT = HERE.parent / "release-sync-pages.py"

spec = importlib.util.spec_from_file_location("release_sync_pages", SCRIPT)
mod = importlib.util.module_from_spec(spec)
spec.loader.exec_module(mod)

REPO_URL = "https://github.com/jangtrinh/design-os-studio/releases/download"
FULL = "release-0.4.0.json"
MAC_ONLY = "release-0.4.0-mac-only.json"


class SyncCase(unittest.TestCase):
    def setUp(self):
        self._tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self._tmp.cleanup)
        self.tree = Path(self._tmp.name)
        (self.tree / "docs").mkdir()
        shutil.copy(FIX / "README.md", self.tree / "README.md")
        for name in ("index.html", "install.md", "llms.txt", "sitemap.xml"):
            shutil.copy(FIX / name, self.tree / "docs" / name)

    def sync(self, assets=FULL, version="0.4.0", *extra):
        return subprocess.run(
            [sys.executable, str(SCRIPT), version, "--root", str(self.tree),
             "--assets-json", str(FIX / assets), *extra],
            capture_output=True, text=True,
        )

    def release_json(self, version):
        """Release JSON for `version`: same asset layout as release-0.4.0.json, tagName v<version>."""
        import json
        data = json.loads((FIX / FULL).read_text())
        data["tagName"] = f"v{version}"
        for asset in data["assets"]:
            asset["name"] = asset["name"].replace("0.4.0", version)
        path = self.tree / f"release-{version}.json"
        path.write_text(json.dumps(data))
        return path

    def sync_to(self, version, *extra):
        """Sync to `version` using a release json whose tagName matches it."""
        return subprocess.run(
            [sys.executable, str(SCRIPT), version, "--root", str(self.tree),
             "--assets-json", str(self.release_json(version)), *extra],
            capture_output=True, text=True,
        )

    def read(self, rel):
        return (self.tree / rel).read_text()

    def snapshot(self):
        return {p.name: p.read_text() for p in self.tree.rglob("*") if p.is_file()}


class RewriteTests(SyncCase):
    def test_rewrites_every_download_link_and_size(self):
        out = self.sync()
        self.assertEqual(out.returncode, 0, out.stderr)
        html = self.read("docs/index.html")
        expected = {
            "designos-0.4.0-macos.dmg": "105 MB",
            "designos-0.4.0-macos-universal.tar.gz": "95 MB",
            "designos-0.4.0-x86_64-pc-windows-msvc.zip": "43 MB",
            "designos_0.4.0_amd64.deb": "41 MB",
            "designos-0.4.0-x86_64-unknown-linux-gnu.tar.gz": "55 MB",
        }
        for name, size in expected.items():
            bodies = re.findall(rf'<a[^>]*/v0\.4\.0/{re.escape(name)}"[^>]*>(.*?)</a>', html)
            self.assertTrue(bodies, name)
            self.assertTrue(any(f"<small>{size}</small>" in b for b in bodies), name)
        self.assertEqual(html.count("releases/download/v0.4.0/"), 6)  # hero + 5
        self.assertNotIn("releases/download/v0.3.0", html)
        self.assertIn("<h3>Download Studio 0.4.0</h3>", html)

    def test_updates_readme_install_and_sitemap(self):
        self.sync()
        readme = self.read("README.md")
        self.assertEqual(readme.count("build-release.sh 0.4.0 --local-host"), 2)
        self.assertIn(f"{REPO_URL}/v0.4.0/designos-0.4.0-macos.dmg", readme)
        install = self.read("docs/install.md")
        self.assertEqual(install.count("build-release.sh 0.4.0"), 3)
        self.assertNotIn("build-release.sh 0.3.0", install)
        self.assertIn("<lastmod>2026-10-18</lastmod>", self.read("docs/sitemap.xml"))

    def test_marketing_copy_is_not_rewritten(self):
        self.sync()
        self.assertIn("New in 0.3.0", self.read("README.md"))
        self.assertIn("#new-in-v030", self.read("README.md"))
        self.assertIn("New in 0.3.0.", self.read("docs/index.html"))

    def test_idempotent(self):
        self.assertEqual(self.sync().returncode, 0)
        before = self.snapshot()
        again = self.sync()
        self.assertEqual(again.returncode, 0, again.stderr)
        self.assertIn("no changes", again.stdout)
        self.assertEqual(before, self.snapshot())

    def test_report_lists_manual_followups(self):
        report = self.tree / "report.md"
        self.sync(FULL, "0.4.0", "--report", str(report))
        text = report.read_text()
        self.assertIn("Previous version on the pages: 0.3.0", text)
        self.assertIn("New in 0.3.0", text)
        self.assertIn("v030", text)

    def test_accepts_v_prefix(self):
        self.assertEqual(self.sync(FULL, "v0.4.0").returncode, 0)
        self.assertIn("releases/download/v0.4.0/", self.read("docs/index.html"))


class MissingAssetTests(SyncCase):
    def test_optional_assets_missing_are_dropped_with_warning(self):
        out = self.sync(MAC_ONLY)
        self.assertEqual(out.returncode, 0, out.stderr)
        self.assertIn("warning", out.stderr)
        self.assertIn("windows", out.stderr)
        html = self.read("docs/index.html")
        for gone in ("windows", "amd64", "linux-gnu", '<span class="os">Windows</span>', '<span class="os">Linux</span>'):
            self.assertNotIn(gone, html)
        self.assertIn('<span class="os">macOS</span>', html)
        self.assertEqual(html.count("releases/download/v0.4.0/"), 3)  # hero + dmg + universal

    def test_missing_required_dmg_fails(self):
        out = self.sync("release-0.4.0-no-dmg.json")
        self.assertEqual(out.returncode, 1)
        self.assertIn("no *-macos.dmg", out.stderr)

    def test_missing_asset_in_markdown_fails(self):
        readme = self.tree / "README.md"
        readme.write_text(readme.read_text() +
                          f"\n{REPO_URL}/v0.3.0/designos-0.3.0-x86_64-pc-windows-msvc.zip\n")
        out = self.sync(MAC_ONLY)
        self.assertEqual(out.returncode, 1)
        self.assertIn("is not in release", out.stderr)

    def test_final_assertion_catches_leftover_old_link(self):
        release = {"assets": {"designos-0.4.0-macos.dmg": 1}, "published": "2026-10-18"}
        with self.assertRaisesRegex(mod.SyncError, "old-version link left"):
            mod.final_assertions(self.tree, ["docs/index.html"], "0.4.0", release, mod.DEFAULT_REPO)


class GuardTests(SyncCase):
    def test_refuses_downgrade(self):
        # The json matches the requested tag, so only the downgrade check can reject it.
        self.assertEqual(self.sync_to("0.4.0").returncode, 0)
        before = self.snapshot()
        out = self.sync_to("0.3.0")
        self.assertEqual(out.returncode, 1)
        self.assertIn("pages already point at v0.4.0, newer than v0.3.0; refusing to downgrade", out.stderr)
        self.assertEqual(before | {"release-0.3.0.json": ""}, self.snapshot() | {"release-0.3.0.json": ""})

    def test_version_compare_is_numeric_not_lexicographic(self):
        # "0.10.0" < "0.9.0" as strings; as versions 0.10.0 is newer.
        self.assertEqual(self.sync_to("0.9.0").returncode, 0)
        up = self.sync_to("0.10.0")
        self.assertEqual(up.returncode, 0, up.stderr)
        self.assertIn("releases/download/v0.10.0/designos-0.10.0-macos.dmg", self.read("docs/index.html"))
        self.assertNotIn("v0.9.0", self.read("docs/index.html"))
        self.assertIn("build-release.sh 0.10.0", self.read("README.md"))

    def test_numeric_compare_refuses_0_10_0_to_0_9_0(self):
        self.assertEqual(self.sync_to("0.9.0").returncode, 0)
        self.assertEqual(self.sync_to("0.10.0").returncode, 0)
        out = self.sync_to("0.9.0")
        self.assertEqual(out.returncode, 1)
        self.assertIn("pages already point at v0.10.0, newer than v0.9.0; refusing to downgrade", out.stderr)

    def test_refuses_prerelease_and_bad_version(self):
        self.assertEqual(self.sync("release-0.4.0-prerelease.json").returncode, 1)
        out = self.sync(FULL, "0.4.0-rc.1")
        self.assertEqual(out.returncode, 1)
        self.assertIn("must look like", out.stderr)

    def test_size_label_rounds_decimal_megabytes(self):
        self.assertEqual(mod.size_label(105_411_149), "105 MB")
        self.assertEqual(mod.size_label(95_044_809), "95 MB")
        self.assertEqual(mod.size_label(43_278_287), "43 MB")
        self.assertEqual(mod.size_label(104_600_000), "105 MB")


class PublishJobTests(unittest.TestCase):
    """The workflow syncs on the publish job, not on the run's own conclusion."""

    @staticmethod
    def jobs(**conclusions):
        return {"jobs": [{"name": n.replace("_", " "), "conclusion": c} for n, c in conclusions.items()]}

    def test_publish_succeeded_while_a_platform_leg_failed(self):
        run = self.jobs(version_check="success", windows_leg="failure", macos="success", Publish_release="success")
        self.assertTrue(mod.publish_job_succeeded(run))

    def test_publish_skipped_or_failed_or_missing(self):
        self.assertFalse(mod.publish_job_succeeded(self.jobs(macos="failure", Publish_release="skipped")))
        self.assertFalse(mod.publish_job_succeeded(self.jobs(Publish_release="failure")))
        self.assertFalse(mod.publish_job_succeeded(self.jobs(macos="success")))
        self.assertFalse(mod.publish_job_succeeded({}))

    def run_cli(self, payload):
        import json
        return subprocess.run([sys.executable, str(SCRIPT), "--publish-job-ok"], input=json.dumps(payload),
                              capture_output=True, text=True).returncode

    def run_cli_raw(self, stdin):
        return subprocess.run([sys.executable, str(SCRIPT), "--publish-job-ok"], input=stdin,
                              capture_output=True, text=True)

    def test_cli_unusable_answer_exits_2_not_skip(self):
        # gh prints nothing on a 403; an API error body has no "jobs". Both must be distinguishable
        # from "publish job did not succeed" (exit 1), or a token problem would skip every sync.
        for stdin in ("", "not json", '{"message": "Resource not accessible by personal access token"}', "[]"):
            out = self.run_cli_raw(stdin)
            self.assertEqual(out.returncode, 2, stdin)
            self.assertNotIn("Traceback", out.stderr)
            self.assertIn("gh run view", out.stderr)

    def test_cli_exit_code_matches(self):
        self.assertEqual(self.run_cli(self.jobs(windows_leg="failure", Publish_release="success")), 0)
        self.assertEqual(self.run_cli(self.jobs(windows_leg="success", Publish_release="skipped")), 1)


if __name__ == "__main__":
    unittest.main()
