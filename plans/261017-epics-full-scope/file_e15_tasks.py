#!/usr/bin/env python3
"""File epic E15 "AI design" (e15-tasks.json): rename milestone v0.6, apply pack moves, create the epic and
its tasks, append amendments. REST only, paced and resumable (state in filed-state.json under "e15").

Run add_field_options.py first so the Pack option "v0.6 AI design" and the Epic option "E15 AI design" exist.
"""
import importlib.util, json, re, pathlib, sys

HERE = pathlib.Path(__file__).parent
spec = importlib.util.spec_from_file_location("f", HERE / "file_master_tasks.py")
f = importlib.util.module_from_spec(spec); spec.loader.exec_module(f)

_BANNED_FILE = pathlib.Path.home() / "Products/plans/reports/banned-terms.txt"  # kept outside the repo
BANNED = re.compile(_BANNED_FILE.read_text().strip() if _BANNED_FILE.exists() else r"(?!)", re.I)
PACK = "v0.6 AI design"
OLD_V06_MILESTONE = 4


def main():
    d = json.loads((HERE / "e15-tasks.json").read_text())
    state = json.loads((HERE / "filed-state.json").read_text())
    st = state.setdefault("e15", {"tasks": {}, "moved": [], "amended": [], "epic": None, "milestone": False})
    save = lambda: (HERE / "filed-state.json").write_text(json.dumps(state, indent=1))
    for t in d["tasks"]:
        assert not BANNED.search(t["title"] + t["body"]), f"banned name in {t['id']}"
    for a in d["amend"]:
        assert not BANNED.search(a["add_text"]), f"banned name in amend {a['issue']}"
    assert not BANNED.search(d["epic"]["title"] + d["epic"]["body"])
    fields = f.field_map()
    f.option(fields, "Pack", PACK)
    f.option(fields, "Epic", "E15", prefix_match=True)

    # 1. The v0.6 milestone becomes the AI design pack.
    if not st["milestone"]:
        f.write(f"repos/{f.REPO}/milestones/{OLD_V06_MILESTONE}", "-X", "PATCH",
                data={"title": PACK, "description": "Headline: AI design (E15, P0). Agents get eyes, grounding, a transactional write protocol, a geometric linter with auto-fix, style archetypes, critique, variants and an offline eval harness. Ships right after v0.5."})
        st["milestone"] = True; save(); print("milestone #4 ->", PACK)
    ms = {**state["milestones"], PACK: OLD_V06_MILESTONE}

    # 2. Pack moves: milestone + Pack field.
    for m in d["moves"]:
        key = f"{m['issue']}->{m['to']}"
        if key in st["moved"]:
            continue
        f.write(f"repos/{f.REPO}/issues/{m['issue']}", "-X", "PATCH", data={"milestone": ms[m["to"]]})
        try:
            item = f.find_item(m["issue"])
        except RuntimeError:  # filed by another session without a board item
            iss = f.api(f"repos/{f.REPO}/issues/{m['issue']}")
            item = f.add_to_project(iss["id"], iss["number"])
        fid, oid = f.option(fields, "Pack", m["to"])
        f.set_fields(item, [{"id": fid, "value": oid}])
        st["moved"].append(key); save(); print(f"moved #{m['issue']} -> {m['to']}")

    # 3. Epic E15.
    if not st["epic"]:
        e = d["epic"]
        iss = f.write(f"repos/{f.REPO}/issues", "-X", "POST", data={"title": e["title"], "body": e["body"], "labels": ["epic", "roadmap"], "milestone": OLD_V06_MILESTONE})
        item = f.add_to_project(iss["id"], iss["number"])
        vals = []
        for field, wanted, pref in (("Epic", "E15", True), ("Status", "Backlog", False), ("Priority", "P0", True), ("Pack", PACK, False)):
            fid, oid = f.option(fields, field, wanted, prefix_match=pref); vals.append({"id": fid, "value": oid})
        f.set_fields(item, vals)
        st["epic"] = iss["number"]; save(); print("epic E15 ->", iss["number"])

    # 4. Tasks, dependencies first so bodies cite real issue numbers.
    def ref(dep):
        if dep in st["tasks"]:
            return f"#{st['tasks'][dep]} ({dep})"
        return dep if str(dep).startswith("#") else f"{dep}"
    for t in f.topo(d["tasks"]):
        if t["id"] in st["tasks"]:
            continue
        deps = ", ".join(ref(x) for x in t.get("deps", [])) or "none"
        body = f"{t['body']}\n\n---\n**Task ID:** {t['id']} · **Depends on:** {deps}\n\nEpic E15 AI design (P0). Claim with `scripts/roadmap-claim.sh` (protocol #41). Spec-ready gate: a full spec is written before the pack opens (docs/releasing.md)."
        iss = f.write(f"repos/{f.REPO}/issues", "-X", "POST", data={"title": t["title"], "body": body, "labels": ["task", "roadmap"], "milestone": OLD_V06_MILESTONE})
        st.setdefault("partial", {})[t["id"]] = iss["number"]; save()
        item = f.add_to_project(iss["id"], iss["number"])
        vals = []
        for field, wanted, pref in (("Status", "Backlog", False), ("Epic", "E15", True), ("Priority", "P0", True), ("Size", t["size"], True), ("Pack", PACK, False)):
            fid, oid = f.option(fields, field, wanted, prefix_match=pref); vals.append({"id": fid, "value": oid})
        vals.append({"id": fields["Touches"][0], "value": t["touches"][:1000]})
        f.set_fields(item, vals)
        try:
            f.write(f"repos/{f.REPO}/issues/{st['epic']}/sub_issues", "-X", "POST", data={"sub_issue_id": iss["id"]})
        except RuntimeError as e:
            if "already" not in str(e).lower():
                raise
        st["tasks"][t["id"]] = iss["number"]; st["partial"].pop(t["id"], None); save()
        print(f"{t['id']} -> #{iss['number']}")

    # 5. Amendments.
    for a in d["amend"]:
        if str(a["issue"]) in st["amended"]:
            continue
        body = f.api(f"repos/{f.REPO}/issues/{a['issue']}")["body"] or ""
        if a["add_text"] not in body:
            f.write(f"repos/{f.REPO}/issues/{a['issue']}", "-X", "PATCH", data={"body": body + "\n\n---\n" + a["add_text"]})
        st["amended"].append(str(a["issue"])); save(); print(f"amended #{a['issue']}")
    print(f"DONE epic=#{st['epic']} tasks={len(st['tasks'])}/{len(d['tasks'])} moved={len(st['moved'])} amended={len(st['amended'])}")


if __name__ == "__main__":
    main()
