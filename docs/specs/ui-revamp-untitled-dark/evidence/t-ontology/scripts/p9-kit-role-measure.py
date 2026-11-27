import re, subprocess, json, sys
REPO='.'
TC=REPO+'/plans/261025-1247-untitled-ui-dark-revamp/specs/token-contract.md'
lines=open(TC).read().splitlines()
roles=set(); consts=set()
mode=None
for ln in lines:
    if ln.startswith('| # | Constant'): mode=('a',6,2); continue
    if ln.startswith('| # | New constant'): mode=('b',4,2); continue
    if mode and not ln.startswith('|'): mode=None; continue
    if mode and re.match(r'^\|\s*N?\d+\s*\|',ln):
        cells=ln.split('|')
        tok=cells[mode[1]].strip(); c=cells[mode[2]]
        for m in re.findall(r'[A-Z][A-Z0-9_]{2,}',c): consts.add(m)
        for m in re.findall(r'\b[a-z][a-z0-9]*(?:[-_][a-z0-9]+)+\b',tok): roles.add(m)
# shipped names: constants, kebab constants (V1 pending paths), V2 names, store key segments
v2=['BADGE_NEUTRAL_BG','BADGE_NEUTRAL_BORDER','BADGE_NEUTRAL_TEXT','BADGE_SUCCESS_BORDER','BADGE_WARNING_BORDER','BADGE_ERROR_BORDER','BADGE_SUCCESS_TEXT','BADGE_WARNING_TEXT','BADGE_ERROR_TEXT','TILE_DESIGN_BG','TILE_DESIGN_GLYPH','TILE_BOARD_BG','TILE_BOARD_GLYPH','TILE_DECK_BG','TILE_DECK_GLYPH','TILE_SPEC_BG','TILE_SPEC_GLYPH']
shipped=set()
for c in list(consts)+v2:
    shipped.add(c); shipped.add(c.lower().replace('_','-'))
def show(f):
    r=subprocess.run(['git','-C',REPO,'show','8dfaee93:'+f],capture_output=True,text=True)
    return r.stdout if r.returncode==0 else None
tok=json.loads(show('design/design.tokens.json'))
def walk(o):
    if isinstance(o,dict):
        for k,v in o.items():
            if not k.startswith('$'): shipped.add(k)
            walk(v)
walk(tok)
kit=roles-shipped
store=['design/soul.md','design/principles.md','design/DESIGN.md','design/design.tokens.json','design/tokens.json','design/component-registry.json','design/ds.manifest.json','design/specimen.html','crates/designos-app/src/theme.rs']
hits=[]
for f in store:
    t=show(f)
    if t is None: continue
    for i,l in enumerate(t.splitlines(),1):
        for m in re.finditer(r'[A-Za-z0-9_-]+',l):
            if m.group(0) in kit: hits.append((f,i,m.group(0)))
RES=({'kit_roles_in_contract':len(roles),'shipped_overlap_dropped':len(roles&shipped),'kit_roles_checked':len(kit),'dropped_examples':sorted(roles&shipped)[:12],'hits':len(hits),'hit_tokens':sorted({h[2] for h in hits}),'hits_by_file':{f:sum(1 for h in hits if h[0]==f) for f in store}})
print(json.dumps(RES,indent=1))
print("probe", {k:(k in kit) for k in ["bg-secondary_hover","fg-quaternary","text-placeholder","bg-secondary"]})
for h in hits[:30]: print(h)
