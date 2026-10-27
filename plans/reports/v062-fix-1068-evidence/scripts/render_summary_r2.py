"""Builds one terminal-style HTML page from the round-2 evidence logs of #1068 (lines copied verbatim) for a PNG capture.

Usage: python3 -I render_summary_r2.py <evidence folder> <fix head sha> <out.html>
"""
import html
import json
import re
import sys
from pathlib import Path

E, SHA, OUT = Path(sys.argv[1]), sys.argv[2], Path(sys.argv[3])
HOME = str(Path.home())


def lines(name, patterns=(r".*",), limit=None):
    path = E / name
    if not path.exists():
        return [f"(missing: {name})"]
    found = [ln.rstrip() for ln in path.read_text(errors="replace").splitlines() if any(re.search(p, ln) for p in patterns)]
    return found[:limit] if limit else found


def cls(line):
    if re.search(r"FAIL|FAILED|HANG|exit=[1-9]|SUMMARY: 0 PASS|not allowlisted|raw filesystem|stale ", line):
        return "bad"
    if re.search(r"PASS|test result: ok|Finished|exit=0|Gate Verdict: PASS|SUMMARY: 15 PASS", line):
        return "good"
    return "ok"


base = {r["case"]: r for r in json.loads((E / "r2-e2e-base.json").read_text())}
fix = {r["case"]: r for r in json.loads((E / "r2-e2e-fix.json").read_text())}
rows = ['<div class="cmd">$ mcp_session_link_probe.py &lt;designos-mcp&gt; &lt;scratch&gt;   (a fresh session folder and server per case; round 2 adds the last five)</div>']
rows.append("<table><tr><th>planted in the session folder</th><th>base f8f2186f</th><th>fix " + SHA[:8] + "</th></tr>")
for case, b in base.items():
    f = fix[case]
    short = lambda d: html.escape(d.split(";")[0].split(" (")[0][:70])
    rows.append(
        f"<tr><td>{html.escape(case[:78])}</td><td class='{'good' if b['verdict'] == 'PASS' else 'bad'}'>{b['verdict']} {short(b['detail'])}</td>"
        f"<td class='{'good' if f['verdict'] == 'PASS' else 'bad'}'>{f['verdict']} {short(f['detail'])}</td></tr>"
    )
rows.append("</table>")

ITEMS = [
    ("1 copy: one style, cause first, then ', then try again'", [r"the_refusal_reads_as_the_sentence", r"a_link_at_the_(canvas|edit_log|ledger)", r"fifos::a_fifo_at_the_canvas", r"a_session_folder_that_is_gone"]),
    ("2 F1 warnings reach the model's text (metadata XML, screenshot)", [r"reports::a_warning_reaches_the_model"]),
    ("3 F2 a hard link is refused on reads too", [r"a_hard_link_at_the_canvas_is_not_loaded", r"a_hard_link_is_refused_by_every_call"]),
    ("4 F3 folder, socket, read-only: a sentence with a next step", [r"hazards::", r"a_folder_at_a_rename_target_and_a_socket"]),
    ("5 F4 ELOOP under O_NOFOLLOW is a link, no re-check", [r"eloop_under_no_follow"]),
    ("6 O_NONBLOCK cleared after the open", [r"a_file_is_handed_back_in_blocking_mode"]),
]
sections = [
    ("round 2 red, on e6caa677 sources (tests first): lib and session_files", lines("r2-red-on-round1-head.log", [r"^test result"])),
]
for title, patterns in ITEMS:
    shown = [ln for ln in lines("r2-red-on-round1-head.log", [r"^test .*FAILED"]) if any(re.search(p, ln) for p in patterns)][:2]
    sections.append((f"red item {title}", shown or ["(see the log)"]))
sections += [
    ("round 2 green, same tests on the fix: lib and session_files", lines("r2-green-items-1-6.log", [r"^test result"])),
    ("item 7 scan test: green on the tree", lines("r2-scan-test-green.log", [r"^test result"])),
    ("item 7 mutation 1: one helper call routed back to std::fs (ledger.rs append_json_line)", lines("r2-scan-mutation-1-helper-call-back-to-std-fs.log", [r"^filesystem call", r"^test result"])),
    ("item 7 mutation 2: std::fs::read_to_string(session_dir.join(\"x\")) in with_session_dir", lines("r2-scan-mutation-2-fresh-raw-read-of-session-dir.log", [r"^raw filesystem", r"^test result"])),
    ("item 7 mutation 3: a stale allowlist entry", lines("r2-scan-mutation-3-stale-allowlist-entry.log", [r"^stale", r"^test result"])),
    ("gate: cargo fmt --all -- --check", lines("r2-gate-fmt.log")),
    ("gate: cargo clippy --workspace --all-targets --locked -- -D warnings", lines("r2-gate-clippy-workspace.log", [r"Finished", r"^error", r"exit="])),
    ("gate: cargo test -p designos-mcp --locked", lines("r2-gate-test-mcp.log", [r"^test result: ok\. (24\d|4\d|9) ", r"FAILED", r"exit="])),
    ("gate: cargo test -p designos-eval --locked", lines("r2-gate-test-eval.log", [r"^test result: ok\. (16|8) ", r"FAILED", r"exit="])),
    ("gate: cargo test -p designos-app --bin designos --locked", lines("r2-gate-test-app.log", [r"^test result", r"FAILED$", r"exit="])),
    ("gate: python3 scripts/zero_fake_ui_gate.py", lines("r2-gate-zero-fake-ui.log", [r"Gate Verdict", r"exit="])),
    ("real-home mtimes (stat only): before, then after", lines("r2-home-mtimes-before.txt") + lines("r2-home-mtimes-after.txt")),
]
for title, body in sections:
    rows.append(f'<div class="cmd">$ {html.escape(title)}</div>')
    for line in body:
        rows.append(f'<div class="{cls(line)}">{html.escape(line.replace(HOME, "~")[:215])}</div>')

OUT.write_text(
    f"""<!doctype html><meta charset="utf-8"><style>
body{{background:#0d1117;color:#c9d1d9;font:13px/1.45 Menlo,monospace;margin:0;padding:18px 22px;width:1660px}}
.cmd{{color:#79c0ff;margin-top:12px}} .good{{color:#7ee787}} .bad{{color:#ffa198}} .ok{{color:#c9d1d9}}
table{{border-collapse:collapse;margin:6px 0}} td,th{{padding:2px 14px 2px 0;text-align:left;vertical-align:top;white-space:nowrap}} th{{color:#8b949e;font-weight:normal}}
h1{{font-size:14px;color:#e6edf3;margin:0 0 6px}}
</style><h1>designos-mcp session files, round 2 (PR #1071): copy, model-visible warnings, hard-link reads, hazards, scan test. base f8f2186f vs fix {SHA[:8]}, 2026-10-24</h1>
{''.join(rows)}"""
)
print(OUT)
