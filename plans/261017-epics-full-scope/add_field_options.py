#!/usr/bin/env python3
"""Add the new Pack and Epic options to project 9's single-select fields (GraphQL; there is no REST write for options).

updateProjectV2Field replaces the whole option list, so every existing option is resent with its current
name, colour and description, then the new ones are appended. Run this only after the retention test on a
scratch project has confirmed that existing item values survive.
Usage: python3 add_field_options.py [--dry-run]
"""
import json, subprocess, sys, time, pathlib

HERE = pathlib.Path(__file__).parent
DRY = "--dry-run" in sys.argv
NEW_EPICS = ["E11 Prototyping", "E12 Canvas, accessibility & text", "E13 Sites & code layers", "E14 iPad / mobile", "E15 AI design"]


def gql(query, retries=30):
    for _ in range(retries):
        r = subprocess.run(["gh", "api", "graphql", "-f", f"query={query}"], capture_output=True, text=True)
        out = r.stdout + r.stderr
        if r.returncode == 0 and "rate limit" not in out.lower():
            return json.loads(r.stdout)
        if "rate limit" not in out.lower():
            sys.exit(f"GraphQL error: {out[:400]}")
        time.sleep(60)
    sys.exit("GraphQL: gave up after rate-limit retries")


def field(name):
    q = ('query{user(login:"jangtrinh"){projectV2(number:9){field(name:"%s"){... on ProjectV2SingleSelectField'
         '{id options{id name color description}}}}}}' % name)
    return gql(q)["data"]["user"]["projectV2"]["field"]


def esc(s):
    return s.replace("\\", "\\\\").replace('"', '\\"')


def update(f, new_names, color):
    have = [o["name"] for o in f["options"]]
    add = [n for n in new_names if n not in have]
    if not add:
        print(f"{f['id']}: nothing to add")
        return []
    opts = [f'{{id:"{o["id"]}",name:"{esc(o["name"])}",color:{o["color"]},description:"{esc(o["description"] or "")}"}}' for o in f["options"]]
    opts += [f'{{name:"{esc(n)}",color:{color},description:""}}' for n in add]
    print(f"{f['id']}: keeping {len(have)}, adding {len(add)}: {add[:3]}…")
    if DRY:
        return []
    m = 'mutation{updateProjectV2Field(input:{fieldId:"%s",singleSelectOptions:[%s]}){projectV2Field{... on ProjectV2SingleSelectField{options{id name}}}}}' % (f["id"], ",".join(opts))
    res = gql(m)["data"]["updateProjectV2Field"]["projectV2Field"]["options"]
    kept = {o["name"]: o["id"] for o in res}
    lost = [o["name"] for o in f["options"] if kept.get(o["name"]) != o["id"]]
    print(f"  done: {len(res)} options; existing ids changed: {lost or 'none'}")
    return lost


def rest(*args, data=None):
    for _ in range(10):
        r = subprocess.run(["gh", "api", *args] + (["--input", "-"] if data is not None else []),
                           input=json.dumps(data) if data is not None else None, capture_output=True, text=True)
        if r.returncode == 0:
            return json.loads(r.stdout) if r.stdout.strip() else None
        if "rate limit" not in (r.stderr + r.stdout).lower():
            sys.exit(f"REST error {args[:2]}: {(r.stderr + r.stdout)[:300]}")
        time.sleep(60)
    sys.exit("REST: gave up after rate-limit retries")


def snapshot(names):
    """Every project item's current option NAME for the given fields (REST, all pages via --paginate --slurp)."""
    r = subprocess.run(["gh", "api", "--paginate", "--slurp", "/users/jangtrinh/projectsV2/9/items?per_page=100&fields=427867491,427841525"],
                       capture_output=True, text=True)
    if r.returncode != 0:
        sys.exit(f"snapshot failed: {(r.stderr + r.stdout)[:300]}")
    snap = {}
    for page in json.loads(r.stdout):
        for it in page:
            vals = {}
            for f in it.get("fields") or []:
                if f.get("name") in names and isinstance(f.get("value"), dict):
                    n = f["value"].get("name")
                    vals[f["name"]] = n.get("raw") if isinstance(n, dict) else n
            if vals:
                snap[str(it["id"])] = vals
    return snap


def restore(snap):
    """Re-apply each item's option by NAME, in case the option ids changed. Skips items that already match."""
    fields = {f["name"]: f for f in rest("/users/jangtrinh/projectsV2/9/fields")}
    ids = {fn: {(o["name"]["raw"] if isinstance(o["name"], dict) else o["name"]): o["id"] for o in fields[fn]["options"]}
           for fn in ("Pack", "Epic")}
    fixed = 0
    for item, vals in snap.items():
        payload = [{"id": fields[fn]["id"], "value": ids[fn][name]} for fn, name in vals.items() if name in ids[fn]]
        if payload and not DRY:
            rest(f"/users/jangtrinh/projectsV2/9/items/{item}", "-X", "PATCH", data={"fields": payload})
            fixed += 1
            time.sleep(2)
    print(f"restored values on {fixed} items")


def main():
    d = json.loads((HERE / "master-tasks.json").read_text())
    snap = snapshot({"Pack", "Epic"})
    print(f"snapshot: {len(snap)} items with Pack/Epic values -> field-values-snapshot.json")
    # Any option-list update wipes item values on GitHub, even when ids are kept (verified on a scratch project).
    update(field("Pack"), [p["title"] for p in d["packs"]] + ["v0.6 AI design"], "BLUE")
    update(field("Epic"), NEW_EPICS, "PURPLE")
    if DRY:
        return
    restore(snap)
    after = snapshot({"Pack", "Epic"})
    missing = {k: v for k, v in snap.items() if after.get(k) != v}
    (HERE / "field-values-snapshot.json").write_text(json.dumps(snap, indent=1))  # keep the pre-update snapshot on disk
    print(f"verify: {len(snap) - len(missing)}/{len(snap)} items match the pre-update values; mismatches: {list(missing.items())[:5]}")
    if missing:
        sys.exit("VERIFY FAILED: some items lost Pack/Epic values; rerun restore from field-values-snapshot.json")


if __name__ == "__main__":
    main()
