"""Builds a terminal-style HTML page from the #1061 evidence logs (lines copied verbatim from the logs) for a PNG capture.
Usage: python3 -I render-gate-summary-1061.py <evidence folder> <out.html> <head sha> <pr number>"""
import html
import re
import sys
from pathlib import Path

E = Path(sys.argv[1])
out = Path(sys.argv[2])
head, pr = sys.argv[3], sys.argv[4]


def grab(name, patterns):
    path = E / name
    if not path.exists():
        return [f"(missing log: {name})"]
    text = path.read_text(errors="replace").splitlines()
    return [line for line in text if any(re.search(p, line) for p in patterns)]


sections = [
    ("~/.gemini file mtimes before any final gate run (names + mtimes only)", grab("mtimes-before.txt", [r"\d"])),
    ("cargo fmt --all -- --check", grab("gate-fmt.log", [r"exit="])),
    ("cargo clippy -p designos-app --all-targets --locked -- -D warnings", grab("gate-clippy.log", [r"Finished", r"exit="])),
    ("python3 scripts/zero_fake_ui_gate.py", grab("gate-zero-fake-ui.log", [r"Gate Verdict", r"exit="])),
    ("cargo test -p designos-mcp --locked  (isolated HOME, scratch TMPDIR with no '//')", grab("gate-test-mcp.log", [r"^test result: ok\. 2\d\d", r"FAILED", r"^mcp exit="])),
    ("cargo test -p designos-app --bin designos --locked  (isolated HOME, scratch TMPDIR)", grab("gate-test-app.log", [r"^test result", r"^app exit=", r"FAILED$", r"scratch removed"])),
    ("flake #996, rerun alone 3x (only if it failed)", grab("flake-996-rerun.log", [r"^== ", r"test result", r"panicked"])),
    ("RED on 533817f2 (a copy of the base + the tests): what the old code did", grab("red-on-base-533817f2-round2.log", [r"^test .*FAILED", r"the target changed", r"^test result", r"^  left:|^ right:"])),
    ("RED on 533817f2, first batch (race: swapped-in link; session files through a link)", grab("red-on-base-533817f2.log", [r"race:", r"changed its target", r"keep running", r"wrote through the link: the target now holds .gemini", r"^test result"])),
    ("GREEN: the race on the fixed tree (swappers on .gemini, .grok/hooks, .agents/plugins/designos; Studio rewrites 200 times per run)", grab("race-green-runs.log", [r"^run"])),
    ("MUTATION: check the entry, then open following links (natural window, then widened by 100 us)", grab("mutation-check-then-open-race.log", [r"^run [16]"]) + grab("mutation-check-then-open-race-widened.log", [r"^run"])),
    ("Real Codex sandbox probe on 533817f2 (agent plants a link; Studio rewrites)", grab("codex-sandbox-probe-on-base-533817f2.log", [r"^== A|ln -s exit|^PROBE write|^victim (before|after A)|RESULT"])),
    ("Real Codex sandbox probe on the fix", grab("codex-sandbox-probe.log", [r"^tree|^== |ln -s exit|^PROBE write|^victim|hard link|RESULT|mv .agents"])),
    ("trial merge with PR #1058 (git merge-tree --write-tree)", grab("merge-tree-1058.log", [r".*"])),
    ("~/.gemini file mtimes after all runs", grab("mtimes-after.txt", [r"\d"])),
]

home = str(Path.home())
rows = []
for title, lines in sections:
    rows.append(f'<div class="cmd">$ {html.escape(title)}</div>')
    for line in lines:
        line = line.replace(home, "~")
        line = re.sub(r"/Users/[^ ]*/agent-[0-9a-f]+", "<worktree>", line)
        cls = "bad" if re.search(r"FAILED|panicked|exit=[12]|CONFLICT|HIT=[1-9]|CHANGED|\(FAIL\)", line) else "ok"
        if re.search(r"test result: ok|exit=0|PASS|HIT=0|Finished|Gate Verdict: PASS", line):
            cls = "good"
        rows.append(f'<div class="{cls}">{html.escape(line[:215])}</div>')

page = f"""<!doctype html><meta charset="utf-8"><style>
body{{background:#0d1117;color:#c9d1d9;font:13px/1.45 Menlo,monospace;margin:0;padding:18px 22px;width:1560px}}
.cmd{{color:#79c0ff;margin-top:10px}} .good{{color:#7ee787}} .bad{{color:#ffa198}} .ok{{color:#c9d1d9}}
h1{{font-size:14px;color:#e6edf3;margin:0 0 6px}}
</style><h1>#1061 no-follow writes below a session folder: PR #{html.escape(pr)}, head {html.escape(head)}, 2026-10-24</h1>
{''.join(rows)}"""
out.write_text(page)
print(out)
