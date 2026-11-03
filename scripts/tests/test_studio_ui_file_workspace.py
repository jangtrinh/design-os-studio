"""The scratch workspace of `library`, `foundation` and `evidence compare` is removed when the run ends, and no argument parser makes one.
Every earlier run and every unit-suite run left a `studio-ui-file-*` directory behind: parse_args made it with mkdtemp and nothing removed it.

Run: python3 -B -m unittest scripts.tests.test_studio_ui_file_workspace
"""
import contextlib
import io
import sys
import tempfile
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent / "studio_ui_file"))
import assemble_foundation  # noqa: E402
import mcp_client  # noqa: E402
import scratch_workspace as scratch  # noqa: E402


def scratch_dirs():
    return {p.name for p in Path(tempfile.gettempdir()).glob("studio-ui-*")}


class ParseArgsTest(unittest.TestCase):
    def test_parsing_arguments_makes_no_directory(self):
        before = scratch_dirs()
        a = assemble_foundation.parse_args("t", [])
        assemble_foundation.parse_args("t", ["--out", "x.designos"], default_out=False)
        self.assertEqual(scratch_dirs(), before)
        self.assertIsNone(a.workspace)
        self.assertFalse(a.keep_workspace)

    def test_a_named_workspace_is_passed_through_and_keep_is_a_flag(self):
        a = assemble_foundation.parse_args("t", ["--workspace", "w", "--keep-workspace"])
        self.assertEqual(a.workspace, "w")
        self.assertTrue(a.keep_workspace)


class WorkspaceTest(unittest.TestCase):
    def test_a_temporary_workspace_exists_inside_the_block_and_is_gone_after(self):
        with scratch.workspace() as ws:
            self.assertTrue(ws.is_dir())
            (ws / "stage0.designos").write_text("x")
            seen = ws
        self.assertFalse(seen.exists())

    def test_a_failing_run_removes_it_too(self):
        with self.assertRaises(SystemExit):
            with scratch.workspace() as ws:
                seen = ws
                raise SystemExit("a check failed")
        self.assertFalse(seen.exists())

    def test_keep_leaves_the_temporary_workspace_and_names_it(self):
        with tempfile.TemporaryDirectory() as parent:
            out = io.StringIO()
            with contextlib.redirect_stdout(out), scratch.workspace(prefix=str(Path(parent) / "kept-"), keep=True) as ws:
                pass
            self.assertTrue(ws.is_dir())
            self.assertEqual(out.getvalue().strip(), f"workspace kept: {ws}")

    def test_a_named_workspace_is_created_and_never_removed(self):
        with tempfile.TemporaryDirectory() as parent:
            named = Path(parent) / "nested" / "ws"
            with scratch.workspace(named) as ws:
                self.assertEqual(ws, named)
                (ws / "keep.txt").write_text("x")
            self.assertEqual((named / "keep.txt").read_text(), "x")


class LiveServerTest(unittest.TestCase):
    """A shell script that execs `cat` stands in for the server: it echoes the initialize request, which carries the id the client waits for."""

    def fake_server(self, parent):
        script = Path(parent) / "fake-designos-mcp"
        script.write_text("#!/bin/sh\nexec cat\n")
        script.chmod(0o755)
        return str(script)

    def test_close_all_stops_a_server_a_failing_run_left_running(self):
        with tempfile.TemporaryDirectory() as parent:
            mcp = mcp_client.Mcp(parent, binary=self.fake_server(parent))
            self.assertIsNone(mcp.p.poll())
            mcp_client.close_all()
            self.assertIsNotNone(mcp.p.poll())

    def test_close_all_skips_a_server_that_already_closed(self):
        with tempfile.TemporaryDirectory() as parent:
            mcp = mcp_client.Mcp(parent, binary=self.fake_server(parent))
            mcp.close()
            self.assertIsNotNone(mcp.p.poll())
            mcp_client.close_all()
            self.assertEqual(mcp_client.LIVE, [])


if __name__ == "__main__":
    unittest.main()
