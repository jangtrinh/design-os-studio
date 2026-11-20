"""Applies T's P1 (families), P3 (d) colours, P4, P5 and P6 rules to registry-records.json and the contract.
A dry run of the gate's rules on spec data; the gate itself reads the live registry and theme.rs."""
import collections, importlib.util, json, os, subprocess, sys
sys.dont_write_bytecode = True  # imports here must not write caches into specs/ or scripts/
HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.normpath(os.path.join(HERE, '..', '..', '..', '..', '..'))
sp = importlib.util.spec_from_file_location('g', os.path.join(HERE, 'gen-record-bindings.py')); g = importlib.util.module_from_spec(sp); sp.loader.exec_module(g)
recs = json.load(open(os.path.join(HERE, '..', 'registry-records.json')))
head = json.loads(subprocess.run(['git', '-C', REPO, 'show', '8dfaee93:design/design.tokens.json'], capture_output=True, text=True).stdout)
store = {}
for k, v in head['colors'].items():
    new = g.STORE.get('colors.' + k, (None,))[0]
    store['colors.' + k] = (new or v['$value']).upper()
fails = collections.defaultdict(list)
names = {r['name']: r for r in recs}
TIER = {'element': 0, 'component': 1, 'composite': 2, 'region': 3, 'screen': 4}
fam = collections.defaultdict(set)
for r in recs:
    for s in r['rust']:
        fam[s].add(r['name'].split('/')[0])
fails['P1'] = [s for s, f in fam.items() if len(f) > 1]
for r in recs:
    if r['tier'] not in ('element', 'component', 'composite'):
        continue
    colour_paints = [c for c in r['paints'] if c in g.VALUE]
    covered = set()
    for t in r['tokens']:
        if not t.startswith('colors.'):
            continue
        hit = [c for c in colour_paints if g.VALUE[c] == store[t]]
        if not hit:
            fails['P3d token with no equal painted value'].append((r['name'], t, store[t]))
        covered |= set(hit)
    for c in colour_paints:
        if c not in covered and c not in r['pending']:
            fails['P3d painted colour neither bound nor pending'].append((r['name'], c))
for r in recs:
    for c in r['composes']:
        if c.endswith('/*'):
            kids = [n for n in names if n.startswith(c[:-1])]
            if not kids:
                fails['P4 unknown child'].append((r['name'], c))
            continue
        if c not in names:
            fails['P4 unknown child'].append((r['name'], c)); continue
        if TIER[names[c]['tier']] > TIER[r['tier']]:
            fails['P4 upward'].append((r['name'], c))
mod = g.load_c(); cw = g.c_widgets(mod)
for n, w in cw.items():
    r = names[n]
    if (r['states'] or []) != w['states'] or r['selected'] != w['selected']:
        fails['P5'].append(n)
flow = json.loads(subprocess.run(['git', '-C', REPO, 'show', '8dfaee93:design/flow.json'], capture_output=True, text=True).stdout)
cnt = collections.Counter(f for r in recs for f in r.get('flow', []))
fails['P6'] = [s['id'] for s in flow['screens'] if cnt[s['id']] != 1]
total = sum(len(v) for v in fails.values())
for k, v in fails.items():
    print(k, len(v), v[:5])
print('records', len(recs), 'failures', total)
sys.exit(1 if total else 0)
