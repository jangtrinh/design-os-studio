#!/usr/bin/env python3
"""File the agent terminal UX tasks (agent-terminal-ux-tasks.json) into pack v0.6 AI design, epic E15.

REST only, paced through file_master_tasks.write, resumable (state in filed-state.json next to this file).
Sub-issues of epic #585 so they show under E15.
"""
import importlib.util, json, pathlib, re

HERE = pathlib.Path(__file__).parent
spec = importlib.util.spec_from_file_location("f", HERE.parent / "261017-epics-full-scope" / "file_master_tasks.py")
f = importlib.util.module_from_spec(spec); spec.loader.exec_module(f)

_BANNED_FILE = pathlib.Path.home() / "Products/plans/reports/banned-terms.txt"  # kept outside the repo
BANNED = re.compile(_BANNED_FILE.read_text().strip() if _BANNED_FILE.exists() else r"(?!)", re.I)
PACK, EPIC_ISSUE, MILESTONE = "v0.6 AI design", 585, 4


def main():
    d = json.loads((HERE / "agent-terminal-ux-tasks.json").read_text())
    state_path = HERE / "filed-state.json"
    st = json.loads(state_path.read_text()) if state_path.exists() else {"tasks": {}, "partial": {}}
    save = lambda: state_path.write_text(json.dumps(st, indent=1))
    for t in d["tasks"]:
        assert not BANNED.search(t["title"] + t["body"]), t["id"]
    fields = f.field_map()
    for t in d["tasks"]:
        if t["id"] in st["tasks"]:
            continue
        num = st["partial"].get(t["id"])
        if num:
            iss = f.api(f"repos/{f.REPO}/issues/{num}")
        else:
            body = t["body"] + "\n\n---\nSource: agent terminal UX study (2026-10-19). Epic E15 AI design. Claim with `scripts/roadmap-claim.sh` (protocol #41)."
            iss = f.write(f"repos/{f.REPO}/issues", "-X", "POST",
                          data={"title": t["title"], "body": body, "labels": ["task", "roadmap"], "milestone": MILESTONE})
            st["partial"][t["id"]] = iss["number"]; save()
        try:
            item = f.find_item(iss["number"])
        except RuntimeError:
            item = f.add_to_project(iss["id"], iss["number"])
        vals = []
        for field, wanted, pref in (("Status", "Backlog", False), ("Epic", "E15", True), ("Priority", t["priority"], True),
                                    ("Size", t["size"], True), ("Pack", PACK, False)):
            fid, oid = f.option(fields, field, wanted, prefix_match=pref); vals.append({"id": fid, "value": oid})
        f.set_fields(item, vals)
        try:
            f.write(f"repos/{f.REPO}/issues/{EPIC_ISSUE}/sub_issues", "-X", "POST", data={"sub_issue_id": iss["id"]})
        except RuntimeError as e:
            if "already" not in str(e).lower():
                raise
        st["tasks"][t["id"]] = iss["number"]; st["partial"].pop(t["id"], None); save()
        print(f"{t['id']} -> #{iss['number']}", flush=True)
    print("DONE", st["tasks"], flush=True)


if __name__ == "__main__":
    main()
