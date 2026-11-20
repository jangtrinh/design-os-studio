"""Resolve the Untitled UI DTCG export (Studio --export-variables) into flat hex tables per mode."""
import json, glob, os, sys

src, out = sys.argv[1], sys.argv[2]
flat = {}  # dotted path -> {value, type}

def walk(n, path, prefix):
    if isinstance(n, dict) and "$value" in n:
        flat[".".join([prefix] + path)] = {"value": n["$value"], "type": n.get("$type")}
        return
    if isinstance(n, dict):
        for k, v in n.items():
            if not k.startswith("$"):
                walk(v, path + [k], prefix)

files = {}
for f in sorted(glob.glob(os.path.join(src, "*.tokens.json"))):
    name = os.path.basename(f)[: -len(".tokens.json")]
    coll, _, mode = name.rpartition(".")
    d = json.load(open(f))
    files[name] = d
    # DTCG files are rooted at the collection name already
    before = set(flat)
    walk(d, [], "")
    # re-key with mode suffix so Light and Dark do not collide
    for k in set(flat) - before:
        flat[f"{mode}|{k.lstrip('.')}"] = flat.pop(k)

def resolve(mode, value, depth=0):
    if isinstance(value, str) and value.startswith("{") and value.endswith("}") and depth < 20:
        ref = value[1:-1]
        for m in (mode, "Style", "Mode 1", "Value"):
            hit = flat.get(f"{m}|{ref}")
            if hit:
                return resolve(m if m != mode else mode, hit["value"], depth + 1)
        return f"UNRESOLVED:{ref}"
    return value

table = {}
for key, tok in flat.items():
    mode, path = key.split("|", 1)
    table.setdefault(mode, {})[path] = {"raw": tok["value"], "resolved": resolve(mode, tok["value"]), "type": tok["type"]}
json.dump(table, open(out, "w"), indent=1, ensure_ascii=False)
for mode, rows in table.items():
    print(mode, len(rows))
