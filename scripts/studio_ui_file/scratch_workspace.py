"""The scratch directory of an MCP session, removed when the run ends.

    with workspace(args.workspace, keep=args.keep_workspace) as ws:    # `ws` is a pathlib.Path
        ...

A directory the caller names (`--workspace`) is created if it is missing and never removed: the caller owns it. Without one, a new
temporary directory is made and removed on the way out, also when a check failed, on a second Ctrl-C inside the cleanup and on SIGTERM (which
raises SystemExit for the length of the block), unless `keep` is set (then its path is printed).
"""
import contextlib
import pathlib
import shutil
import signal
import sys
import tempfile
import threading

import mcp_client


def _terminate(signum, frame):
    """SIGTERM would end the interpreter without running a `finally`: turn it into the exception that does."""
    raise SystemExit(128 + signum)


@contextlib.contextmanager
def _sigterm_raises_system_exit():
    """For the length of the block SIGTERM raises SystemExit, so the cleanup in `workspace` runs. Only the main thread can install a handler;
    anywhere else, or where the platform has no SIGTERM handler to set, the block runs without one."""
    if threading.current_thread() is not threading.main_thread():
        yield
        return
    try:
        previous = signal.signal(signal.SIGTERM, _terminate)
    except (ValueError, OSError):
        yield
        return
    try:
        yield
    finally:
        signal.signal(signal.SIGTERM, previous)


@contextlib.contextmanager
def workspace(path=None, prefix="studio-ui-file-", keep=False):
    if path:
        named = pathlib.Path(path)
        named.mkdir(parents=True, exist_ok=True)
        yield named
        return
    made = pathlib.Path(tempfile.mkdtemp(prefix=prefix))
    with _sigterm_raises_system_exit():
        try:
            yield made
        finally:
            try:
                mcp_client.close_all()  # a server still running in the folder must stop before it is removed
            finally:  # its own `finally`: a second Ctrl-C or an error inside close_all must not skip the removal
                if keep:
                    print(f"workspace kept: {made}")
                else:
                    try:
                        shutil.rmtree(made)
                    except OSError as e:
                        print(f"could not remove the scratch workspace {made}: {e}", file=sys.stderr)
