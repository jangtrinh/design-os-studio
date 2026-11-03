"""The scratch workspace is removed whatever interrupts the run: a second Ctrl-C landing inside `close_all`, a SIGTERM, a server that is killed
instead of waited for. The W1a fix-3 review found the folder leaking on two Ctrl-C presses 20 ms apart (the second `KeyboardInterrupt` escaped
`close`'s `except Exception` and `rmtree` never ran) and on SIGTERM (Python skips `finally` on a default SIGTERM). Two mutants survived the
suite: the exit no longer called `close_all()`, and `close_all` stopped only the first server.

Run: python3 -B -m unittest scripts.tests.test_studio_ui_file_cleanup_signals
"""
import os
import signal
import subprocess
import sys
import tempfile
import time
import unittest
from pathlib import Path
from unittest import mock

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent / "studio_ui_file"))
import mcp_client  # noqa: E402
import scratch_workspace as scratch  # noqa: E402


def fake_server(parent):
    """A shell script that execs `cat`: it echoes the initialize request, which carries the id the client waits for."""
    script = Path(parent) / "fake-designos-mcp"
    script.write_text("#!/bin/sh\nexec cat\n")
    script.chmod(0o755)
    return str(script)


class FakeProcess:
    """Just enough of a Popen for Mcp.close(): `wait` raises what the test says, `kill` is recorded."""

    def __init__(self, wait_raises):
        self.wait_raises, self.killed = wait_raises, 0
        self.stdin = mock.Mock()

    def wait(self, timeout=None):
        raise self.wait_raises

    def kill(self):
        self.killed += 1

    def poll(self):
        return None


def bare_mcp(process):
    mcp = mcp_client.Mcp.__new__(mcp_client.Mcp)
    mcp.p = process
    return mcp


class CloseTest(unittest.TestCase):
    def test_a_second_interrupt_while_waiting_kills_the_server_and_propagates(self):
        p = FakeProcess(KeyboardInterrupt())
        with self.assertRaises(KeyboardInterrupt):
            bare_mcp(p).close()
        self.assertEqual(p.killed, 1)

    def test_a_system_exit_while_waiting_kills_the_server_and_propagates(self):
        p = FakeProcess(SystemExit(143))
        with self.assertRaises(SystemExit):
            bare_mcp(p).close()
        self.assertEqual(p.killed, 1)

    def test_a_server_that_will_not_stop_is_killed_without_an_error(self):
        p = FakeProcess(subprocess.TimeoutExpired("designos-mcp", 60))
        bare_mcp(p).close()
        self.assertEqual(p.killed, 1)


class ExitTest(unittest.TestCase):
    def setUp(self):
        self.parent = tempfile.TemporaryDirectory()
        self.addCleanup(self.parent.cleanup)
        self.addCleanup(mcp_client.close_all)
        self.binary = fake_server(self.parent.name)

    def test_leaving_the_block_stops_a_server_still_running_in_it(self):
        """Kills the mutant that dropped `close_all()` from the exit."""
        with scratch.workspace() as ws:
            mcp = mcp_client.Mcp(str(ws), binary=self.binary)
            self.assertIsNone(mcp.p.poll())
        self.assertIsNotNone(mcp.p.poll())
        self.assertFalse(ws.exists())

    def test_close_all_stops_every_live_server_not_just_the_first(self):
        """Kills the mutant whose `close_all` returned after the first server."""
        first, second = (mcp_client.Mcp(self.parent.name, binary=self.binary) for _ in range(2))
        self.assertIsNone(first.p.poll())
        self.assertIsNone(second.p.poll())
        mcp_client.close_all()
        self.assertIsNotNone(first.p.poll())
        self.assertIsNotNone(second.p.poll())
        self.assertEqual(mcp_client.LIVE, [])

    def test_an_interrupt_while_stopping_one_server_still_stops_the_others(self):
        first, second = (mcp_client.Mcp(self.parent.name, binary=self.binary) for _ in range(2))
        real_close = first.close

        def interrupted_close():
            real_close()
            raise KeyboardInterrupt

        first.close = interrupted_close
        with self.assertRaises(KeyboardInterrupt):
            mcp_client.close_all()
        self.assertIsNotNone(first.p.poll())
        self.assertIsNotNone(second.p.poll())
        self.assertEqual(mcp_client.LIVE, [])

    def test_the_folder_goes_even_when_close_all_is_interrupted(self):
        """A second Ctrl-C inside `close_all`: `rmtree` runs in its own `finally`."""
        with mock.patch.object(mcp_client, "close_all", side_effect=KeyboardInterrupt):
            with self.assertRaises(KeyboardInterrupt):
                with scratch.workspace() as ws:
                    (ws / "stage0.designos").write_text("x")
        self.assertFalse(ws.exists())

    def test_the_folder_goes_even_when_close_all_fails(self):
        with mock.patch.object(mcp_client, "close_all", side_effect=OSError("boom")):
            with self.assertRaises(OSError):
                with scratch.workspace() as ws:
                    pass
        self.assertFalse(ws.exists())

    def test_keep_still_names_the_folder_when_close_all_is_interrupted(self):
        with mock.patch.object(mcp_client, "close_all", side_effect=KeyboardInterrupt), mock.patch("builtins.print") as shown:
            with self.assertRaises(KeyboardInterrupt):
                with scratch.workspace(prefix=str(Path(self.parent.name) / "kept-"), keep=True) as ws:
                    pass
        self.assertTrue(ws.is_dir())
        shown.assert_called_with(f"workspace kept: {ws}")


class SigtermTest(unittest.TestCase):
    def test_sigterm_inside_the_block_raises_system_exit_and_removes_the_folder(self):
        before = signal.getsignal(signal.SIGTERM)
        with self.assertRaises(SystemExit):
            with scratch.workspace() as ws:
                seen = ws
                handler = signal.getsignal(signal.SIGTERM)
                if not callable(handler):  # a default SIGTERM would end the test runner itself
                    self.fail(f"the workspace installs no SIGTERM handler (found {handler!r})")
                os.kill(os.getpid(), signal.SIGTERM)
                time.sleep(2)  # the handler raises here
        self.assertFalse(seen.exists())
        self.assertIs(signal.getsignal(signal.SIGTERM), before, "the previous SIGTERM handler is put back")

    def test_the_previous_handler_is_restored_after_the_block_whatever_it_was(self):
        """A sentinel, not the default: a restore that never ran leaves `_terminate` installed, which a test that compares with whatever
        was installed before cannot see once an earlier test has leaked it (the mutant that dropped the restore passed the suite)."""
        def sentinel(signum, frame):
            pass

        previous = signal.signal(signal.SIGTERM, sentinel)
        self.addCleanup(signal.signal, signal.SIGTERM, previous)
        with scratch.workspace():
            self.assertIs(signal.getsignal(signal.SIGTERM), scratch._terminate)
        self.assertIs(signal.getsignal(signal.SIGTERM), sentinel)

    def test_the_previous_handler_is_restored_when_the_block_raises(self):
        def sentinel(signum, frame):
            pass

        previous = signal.signal(signal.SIGTERM, sentinel)
        self.addCleanup(signal.signal, signal.SIGTERM, previous)
        with self.assertRaises(RuntimeError):
            with scratch.workspace():
                raise RuntimeError("the check failed")
        self.assertIs(signal.getsignal(signal.SIGTERM), sentinel)

    def test_a_named_workspace_leaves_the_signal_handler_alone(self):
        before = signal.getsignal(signal.SIGTERM)
        with tempfile.TemporaryDirectory() as parent, scratch.workspace(Path(parent) / "w"):
            self.assertIs(signal.getsignal(signal.SIGTERM), before)


if __name__ == "__main__":
    unittest.main()
