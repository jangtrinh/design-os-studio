"""Minimal stdio JSON-RPC client for the installed designos-mcp (0.7.5).

Usage:
    mcp = Mcp(workspace_dir)
    payload = mcp.tool_or_die("designos_get_document", {})

`tool()` returns (ok, payload). payload is the structured content when the server sends it,
otherwise the joined text, otherwise the raw JSON-RPC reply. Errors are never swallowed:
`tool_or_die()` raises McpError with the tool name and the payload. The server is the installed app's
binary; set DESIGNOS_MCP to use another build of the same version.
"""
import json
import os
import subprocess

INSTALLED_MCP = os.environ.get("DESIGNOS_MCP", "/Applications/designos.app/Contents/MacOS/designos-mcp")


LIVE = []  # every server this process started: close_all stops the ones a failing run left running


class McpError(RuntimeError):
    pass


class Mcp:
    def __init__(self, workspace: str, binary: str = INSTALLED_MCP):
        self.p = subprocess.Popen(
            [binary, "--workspace", workspace],
            stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL, text=True,
        )
        self.i = 0
        LIVE.append(self)
        r = self._rpc("initialize", {"protocolVersion": "2025-06-18", "capabilities": {},
                                      "clientInfo": {"name": "studio-ui-redraw", "version": "1"}})
        self.server_info = (r.get("result") or {}).get("serverInfo", {})
        self._send({"jsonrpc": "2.0", "method": "notifications/initialized"})

    def _send(self, m):
        self.p.stdin.write(json.dumps(m) + "\n")
        self.p.stdin.flush()

    def _rpc(self, method, params):
        self.i += 1
        self._send({"jsonrpc": "2.0", "id": self.i, "method": method, "params": params})
        while True:
            line = self.p.stdout.readline()
            if not line:
                raise McpError("designos-mcp closed the pipe")
            m = json.loads(line)
            if m.get("id") == self.i:
                return m

    def tool(self, name, args):
        r = self._rpc("tools/call", {"name": name, "arguments": args})
        res = r.get("result") or {}
        texts = [c.get("text", "") for c in res.get("content", []) if c.get("type") == "text"]
        sc = res.get("structuredContent")
        ok = not res.get("isError") and "error" not in r
        return ok, (sc if sc is not None else ("\n".join(texts) or r))

    def tool_or_die(self, name, args):
        ok, payload = self.tool(name, args)
        if not ok:
            raise McpError(f"{name} failed: {json.dumps(payload)[:2000]}")
        return payload

    def close(self):
        try:
            self.p.stdin.close()
            self.p.wait(timeout=60)
        except Exception:
            self.p.kill()
        except BaseException:  # a second Ctrl-C or a SystemExit while waiting: the server dies with the run, then the interrupt goes on
            self.p.kill()
            raise


def close_all():
    """Stop every server of this process that is still running (a run that failed between open and close leaves one). An interrupt while
    waiting for one server does not skip the others: the interrupted one is already killed, the rest are stopped, and the first interrupt is
    raised once every server is down."""
    interrupted = None
    for mcp in list(LIVE):
        if mcp.p.poll() is None:
            try:
                mcp.close()
            except BaseException as e:  # noqa: BLE001 - re-raised below after the other servers are stopped
                interrupted = interrupted or e
    LIVE.clear()
    if interrupted is not None:
        raise interrupted
