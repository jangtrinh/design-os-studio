"""Builds one terminal-style HTML page from the #1068 evidence logs (lines copied verbatim) for a PNG capture.

Usage: python3 -I render_summary_html.py <evidence folder> <fix head sha> <out.html>
"""
import html
import json
import re
import sys
from pathlib import Path

E, SHA, OUT = Path(sys.argv[1]), sys.argv[2], Path(sys.argv[3])


def lines(name, patterns=(r".*",), limit=None):
    path = E / name
    if not path.exists():
        return [f"(missing: {name})"]
    found = [ln.rstrip() for ln in path.read_text(errors="replace").splitlines() if any(re.search(p, ln) for p in patterns)]
    return found[:limit] if limit else found


def cls(line):
    if re.search(r"FAIL|FAILED|HANG|exit=[1-9]|SUMMARY: 0 PASS", line):
        return "bad"
    if re.search(r"PASS|test result: ok|Finished|exit=0|Gate Verdict: PASS|SUMMARY: 10 PASS", line):
        return "good"
    return "ok"


base = {r["case"]: r for r in json.loads((E / "e2e-base.json").read_text())}
fix = {r["case"]: r for r in json.loads((E / "e2e-fix.json").read_text())}
rows = ['<div class="cmd">$ mcp_session_link_probe.py &lt;designos-mcp&gt; &lt;scratch&gt;   (one fresh session folder and server per case; victim = a user file outside the folder)</div>']
rows.append('<table><tr><th>planted in the session folder</th><th>base 533817f2</th><th>fix ' + SHA[:8] + "</th></tr>")
for case, b in base.items():
    f = fix[case]
    rows.append(
        f"<tr><td>{html.escape(case)}</td><td class='{'good' if b['verdict'] == 'PASS' else 'bad'}'>{b['verdict']} &nbsp;{html.escape(b['detail'])}</td>"
        f"<td class='{'good' if f['verdict'] == 'PASS' else 'bad'}'>{f['verdict']} &nbsp;{html.escape(f['detail'])}</td></tr>"
    )
rows.append("</table>")

sections = [
    ("Rust tests on the base sources (red): cargo test -p designos-mcp --test session_files", lines("red-session-files-on-base.log", [r"^test result", r"^exit="]) + lines("red-session-files-on-base.log", [r"^test .*FAILED"], 3) + ["  ... (all 26 in red-session-files-on-base.log; the 4 that pass on base are the replaced-by-rename guards)"]),
    ("Rust tests on the fix (green): same command", lines("green-session-files-on-fix.log", [r"^test result", r"exit="])),
    ("gate: cargo fmt --all -- --check", lines("gate-fmt.log")),
    ("gate: cargo clippy -p designos-mcp --all-targets --locked -- -D warnings", lines("gate-clippy.log", [r"Finished", r"exit="])),
    ("gate: cargo test -p designos-mcp --locked (isolated HOME/TMPDIR)", lines("gate-test-mcp.log", [r"^test result: ok\. (242|35)", r"FAILED", r"exit="])),
    ("CI parity: cargo clippy --workspace --all-targets --locked", lines("gate-clippy-workspace-ci.log", [r"Finished", r"exit="])),
    ("CI parity: cargo test -p designos-eval --locked (spawns designos-mcp with DESIGNOS_MCP_TRANSCRIPT)", lines("gate-test-eval.log", [r"^test result: ok\. (16|8)", r"FAILED", r"exit="])),
    ("gate: cargo test -p designos-app --bin designos --locked, run 1 of 2: one test failed (a tab-strip status dot, no session file involved); it passed alone and in run 2", lines("gate-test-app-run1-one-failure.log", [r"^test result", r"FAILED$"])),
    ("same test alone (9 agents_tab_strip tests), then the whole gate again", lines("app-tab-strip-tests-rerun-alone.log", [r"^test result"]) + lines("gate-test-app.log", [r"^test result", r"FAILED", r"exit="])),
    ("gate: python3 scripts/zero_fake_ui_gate.py", lines("gate-zero-fake-ui.log", [r"Gate Verdict", r"exit="])),
    ("completeness: fs sites in designos-mcp (non-test), base vs fix", lines("fs-sites-classified-base.txt", [r"^SUMMARY"]) + lines("fs-sites-classified-fixed.txt", [r"^SUMMARY"])),
    ("real-home mtimes (stat only): before, then after", lines("home-mtimes-before.txt") + lines("home-mtimes-after.txt")),
]
home = str(Path.home())
for title, body in sections:
    rows.append(f'<div class="cmd">$ {html.escape(title)}</div>')
    for line in body:
        rows.append(f'<div class="{cls(line)}">{html.escape(line.replace(home, "~")[:200])}</div>')

OUT.write_text(
    f"""<!doctype html><meta charset="utf-8"><style>
body{{background:#0d1117;color:#c9d1d9;font:13px/1.45 Menlo,monospace;margin:0;padding:18px 22px;width:1560px}}
.cmd{{color:#79c0ff;margin-top:12px}} .good{{color:#7ee787}} .bad{{color:#ffa198}} .ok{{color:#c9d1d9}}
table{{border-collapse:collapse;margin:6px 0}} td,th{{padding:2px 14px 2px 0;text-align:left;vertical-align:top;white-space:nowrap}} th{{color:#8b949e;font-weight:normal}}
h1{{font-size:14px;color:#e6edf3;margin:0 0 6px}}
</style><h1>designos-mcp session files (#1068): a link or FIFO planted in the session folder. base 533817f2 vs fix {SHA[:8]}, 2026-10-24</h1>
{''.join(rows)}"""
)
print(OUT)
