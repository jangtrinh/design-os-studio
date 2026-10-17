#!/usr/bin/env python3
"""File the full-scope roadmap (master-tasks.json) on GitHub.

Creates milestones, the new epics (E11-E14) and every task issue, links each task as a sub-issue of
its epic, adds it to project 9 and sets Status/Epic/Priority/Size/Pack/Touches. It also appends the
amendments to existing issues.

- REST only: GraphQL is kept for the one-off field-option updates done beforehand.
- Paced for GitHub's content-creation limit (500/h): one write about every 8 seconds.
- Resumable: progress is kept in filed-state.json, so a rerun skips everything already done.

Usage: python3 file_master_tasks.py [--dry-run] [--limit N]
"""
import json, subprocess, sys, time, re, pathlib, hashlib

HERE = pathlib.Path(__file__).parent
REPO = "jangtrinh/design-os-studio"
OWNER, PROJECT = "jangtrinh", 9
STATE = HERE / "filed-state.json"
EPIC_ISSUES = {f"E{i}": n for i, n in zip(range(1, 11), range(14, 24))}  # E1=#14 ... E10=#23
CURRENT_PACK = "v0.4 Layout 2.0"  # tasks in the current pack go straight to Ready
WRITE_GAP = 8.0
DRY = "--dry-run" in sys.argv
LIMIT = int(sys.argv[sys.argv.index("--limit") + 1]) if "--limit" in sys.argv else None


def api(*args, data=None, retries=8):
    """gh api with retry on rate limits; returns parsed JSON (or None for empty bodies)."""
    cmd = ["gh", "api", *args]
    for attempt in range(retries):
        r = subprocess.run(cmd + (["--input", "-"] if data is not None else []),
                           input=json.dumps(data) if data is not None else None,
                           capture_output=True, text=True)
        if r.returncode == 0:
            return json.loads(r.stdout) if r.stdout.strip() else None
        err = r.stderr + r.stdout
        if re.search(r"rate limit|secondary|abuse|502|503", err, re.I):
            wait = 60 * (attempt + 1)
            print(f"  rate limited, waiting {wait}s", flush=True)
            time.sleep(wait)
            continue
        raise RuntimeError(f"gh api {' '.join(args)} failed: {err.strip()[:300]}")
    raise RuntimeError(f"gh api {' '.join(args)}: gave up after {retries} rate-limit retries")


def write(*args, data=None):
    if DRY:
        print("  DRY", args[:3], (data or {}).get("title", "") if isinstance(data, dict) else "")
        return {"number": 0, "id": 0}
    out = api(*args, data=data)
    time.sleep(WRITE_GAP)
    return out


def load_state():
    return json.loads(STATE.read_text()) if STATE.exists() else {"milestones": {}, "epics": {}, "tasks": {}, "amended": []}


def save_state(s):
    if not DRY:
        STATE.write_text(json.dumps(s, indent=1))


def field_map():
    """Project field ids and single-select option ids, keyed by field name then option name."""
    fields = api(f"/users/{OWNER}/projectsV2/{PROJECT}/fields")
    out = {}
    for f in fields:
        opts = {}
        for o in f.get("options") or []:
            name = o["name"]["raw"] if isinstance(o["name"], dict) else o["name"]
            opts[name] = o["id"]
        out[f["name"]] = (f["id"], opts)
    return out


def option(fields, field, wanted, prefix_match=False):
    fid, opts = fields[field]
    if wanted in opts:
        return fid, opts[wanted]
    if prefix_match:
        for name, oid in opts.items():
            if name.split(" ")[0] == wanted.split(" ")[0]:
                return fid, oid
    raise KeyError(f"{field} has no option {wanted!r}; options: {list(opts)}")


def ensure_milestones(d, state):
    existing = {m["title"].split(" ")[0]: m["number"] for m in api(f"repos/{REPO}/milestones?state=all&per_page=100")}
    for p in d["packs"]:
        title = p["title"]
        if not title.startswith("v"):
            continue  # Later / Unscheduled carry no milestone
        ver = title.split(" ")[0]
        if title in state["milestones"]:
            continue
        if ver in existing:
            state["milestones"][title] = existing[ver]
        else:
            m = write(f"repos/{REPO}/milestones", "-X", "POST", data={"title": title, "description": p.get("description", "")[:1000]})
            state["milestones"][title] = m["number"]
            print(f"milestone {title} -> {m['number']}", flush=True)
        save_state(state)


def find_item(issue_number):
    """Project item id for an issue already on the board (it got added when it became a sub-issue)."""
    r = subprocess.run(["gh", "api", "--paginate", "--slurp", f"/users/{OWNER}/projectsV2/{PROJECT}/items?per_page=100"],
                       capture_output=True, text=True)
    for page in json.loads(r.stdout or "[]"):
        for it in page:
            c = it.get("content") or {}
            if c.get("number") == issue_number and REPO.split("/")[1] in (c.get("repository_url") or c.get("url") or ""):
                return it["id"]
    raise RuntimeError(f"issue #{issue_number} is not on project {PROJECT}")


def add_to_project(issue_id, issue_number):
    try:
        item = write(f"/users/{OWNER}/projectsV2/{PROJECT}/items", "-X", "POST", data={"type": "Issue", "id": issue_id})
        return item["id"]
    except RuntimeError as e:
        if "already exists" not in str(e):
            raise
        return find_item(issue_number)


def set_fields(item_id, values):
    write(f"/users/{OWNER}/projectsV2/{PROJECT}/items/{item_id}", "-X", "PATCH", data={"fields": values})


def ensure_epics(d, state, fields):
    for e in d.get("new_epics", []):
        if e["code"] in state["epics"]:
            continue
        iss = write(f"repos/{REPO}/issues", "-X", "POST", data={"title": e["title"], "body": e["body"], "labels": ["epic", "roadmap"]})
        item = add_to_project(iss["id"], iss["number"])
        efid, eopt = option(fields, "Epic", e["code"], prefix_match=True)
        sfid, sopt = option(fields, "Status", "Backlog")
        set_fields(item, [{"id": efid, "value": eopt}, {"id": sfid, "value": sopt}])
        state["epics"][e["code"]] = iss["number"]
        save_state(state)
        print(f"epic {e['code']} -> #{iss['number']}", flush=True)


def topo(tasks):
    """Order tasks so every dependency is filed first, letting bodies cite real #numbers."""
    by_id = {t["id"]: t for t in tasks}
    seen, order = set(), []

    def visit(t):
        if t["id"] in seen:
            return
        seen.add(t["id"])
        for dep in t.get("deps", []):
            if dep in by_id:
                visit(by_id[dep])
        order.append(t)

    for t in tasks:
        visit(t)
    return order


def render_body(t, state):
    def ref(dep):
        if dep in state["tasks"]:
            return f"#{state['tasks'][dep]} ({dep})"
        if re.fullmatch(r"#?\d+", str(dep)):
            return f"#{str(dep).lstrip('#')}"
        return dep
    deps = ", ".join(ref(x) for x in t.get("deps", [])) or "none"
    return f"{t['body']}\n\n---\n**Task ID:** {t['id']} · **Depends on:** {deps}\n\nFiled from `plans/261017-epics-full-scope/master-tasks.json` (owner-ruled full scope, 2026-10-17). Claim with `scripts/roadmap-claim.sh` before starting (protocol #41)."


def file_tasks(d, state, fields):
    epics = {**EPIC_ISSUES, **state["epics"]}
    done = 0
    for t in topo(d["tasks"]):
        if t["id"] in state["tasks"]:
            continue
        if LIMIT is not None and done >= LIMIT:
            break
        payload = {"title": t["title"], "body": render_body(t, state), "labels": ["task", "roadmap"]}
        if t["pack"] in state["milestones"]:
            payload["milestone"] = state["milestones"][t["pack"]]
        resumed = state.get("partial", {}).get(t["id"])
        if resumed:  # created on an earlier run that stopped before recording it
            iss = api(f"repos/{REPO}/issues/{resumed}")
        else:
            iss = write(f"repos/{REPO}/issues", "-X", "POST", data=payload)
            state.setdefault("partial", {})[t["id"]] = iss["number"]
            save_state(state)
        item = add_to_project(iss["id"], iss["number"])
        status = "Ready" if t["pack"] == CURRENT_PACK else t.get("status", "Backlog")
        vals = []
        for field, wanted, pref in (("Status", status, False), ("Epic", t["epic"], True), ("Priority", t["prio"], True),
                                    ("Size", t["size"], True), ("Pack", t["pack"], False)):
            fid, oid = option(fields, field, wanted, prefix_match=pref)
            vals.append({"id": fid, "value": oid})
        if t.get("touches"):
            vals.append({"id": fields["Touches"][0], "value": t["touches"][:1000]})
        set_fields(item, vals)
        epic_no = epics[t["epic"]]
        try:
            write(f"repos/{REPO}/issues/{epic_no}/sub_issues", "-X", "POST", data={"sub_issue_id": iss["id"]})
        except RuntimeError as e:
            if "already" not in str(e).lower() and "duplicate" not in str(e).lower():
                raise
        state["tasks"][t["id"]] = iss["number"]
        state.get("partial", {}).pop(t["id"], None)
        save_state(state)
        done += 1
        print(f"{t['id']} -> #{iss['number']} [{t['pack']}]", flush=True)


def amend(d, state):
    for a in d.get("amend", []):
        key = f"{a['issue']}:{hashlib.sha1(a['add_text'].encode()).hexdigest()[:10]}"
        if key in state["amended"]:
            continue
        body = api(f"repos/{REPO}/issues/{a['issue']}")["body"] or ""
        if a["add_text"] in body:
            state["amended"].append(key)
            continue
        write(f"repos/{REPO}/issues/{a['issue']}", "-X", "PATCH",
              data={"body": body + "\n\n---\n**Full-scope amendment (2026-10-17):** " + a["add_text"]})
        state["amended"].append(key)
        save_state(state)
        print(f"amended #{a['issue']}", flush=True)


def main():
    d = json.loads((HERE / "master-tasks.json").read_text())
    state = load_state()
    fields = field_map()
    missing = [p["title"] for p in d["packs"] if p["title"] not in fields["Pack"][1]]
    missing_epics = [e["code"] for e in d.get("new_epics", []) if not any(n.split(" ")[0] == e["code"] for n in fields["Epic"][1])]
    if missing or missing_epics:
        sys.exit(f"Add project field options first. Pack missing: {missing}; Epic missing: {missing_epics}")
    ensure_milestones(d, state)
    ensure_epics(d, state, fields)
    file_tasks(d, state, fields)
    amend(d, state)
    print(f"DONE tasks={len(state['tasks'])}/{len(d['tasks'])} epics={state['epics']} amended={len(state['amended'])}")


if __name__ == "__main__":
    main()
