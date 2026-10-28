"""Spec T check P7 pins (fix round 2). One schema for every set: {file, fn, item, lines, count, note}, keyed by
(file, enclosing fn, item), where item is the literal or the call. Studio at 8dfaee93; comments, string bodies and
#[cfg(test)] items are dropped with the repo's scripts/rust_literal_scan.py clean(); files under */tests/ and tests.rs
are skipped. Usage: python3 -I pin-ns-sites.py <repo> <out.json>"""
import collections, importlib.util, json, re, subprocess, sys
REPO, OUT = sys.argv[1], sys.argv[2]
R = 'crates/designos-app/src/'
REV = '8dfaee93'
src_rls = subprocess.run(['git', '-C', REPO, 'show', REV + ':scripts/rust_literal_scan.py'], capture_output=True, text=True).stdout
rls = {}
exec(compile(src_rls, 'rust_literal_scan.py', 'exec'), rls)
clean = rls['clean']
ALLOW = {'AI', 'API', 'CLI', 'CSS', 'MCP', 'PRD', 'SVG', 'PNG', 'JPG', 'PDF', 'URL', 'ID', 'UI', 'AA', 'AAA'}
PAINT = re.compile(r'\.text\(|label\(|RichText::new\(|badge|tag\(|chip')
NS2 = re.compile(r'if\s+[!(]*\s*(?:[a-z_.]*\b)?(active|selected|is_active|is_selected|current)\b[^{};]*\{\s*(?:theme::)?BG_BRAND\b')
MESH = re.compile(r'Mesh::(default|with_texture|new)\(')
MONO = re.compile(r'font_mono\(|FontFamily::Monospace')
NOTE = {('skills/mod.rs', 'READY'): 'debt', ('specs.rs', 'ON THIS PAGE'): 'debt (hidden reader)',
        ('showcase/mobile.rs', 'ALEX MORGAN'): 'showcase content', ('showcase/kit.rs', 'AM'): 'showcase content (avatar initials)',
        ('cmdk.rs', 'PASS'): 'document content written by the audit action', ('panels/check_designs.rs', 'BG_BRAND'): 'debt (AA/AAA active segment)',
        ('home/mod.rs', 'Mesh::default()'): 'content preview (board thumbnail dots)',
        ('widgets/mod.rs', 'Mesh::default()'): 'content preview (gradient swatch of a paint)',
        ('widgets/picker.rs', 'Mesh::default()'): 'colour picker content (gradients of the edited paint)',
        ('home/main_view.rs', 'Mesh::default()'): 'orphan file, not in the module tree (home/mod.rs declares main_view inline)'}


def fn_at(lines, i):
    for j in range(i, -1, -1):
        m = re.match(r'^\s*(?:pub(?:\([a-z]+\))? )?(?:async )?fn ([a-z_0-9]+)', lines[j])
        if m:
            return m.group(1)
    return '(module)'


sets = {k: collections.OrderedDict() for k in ('ns1', 'ns2', 'ns4', 'ns5')}


def add(key, f, fn, item, line):
    rec = sets[key].setdefault((f, fn, item), {'file': f, 'fn': fn, 'item': item, 'lines': [], 'count': 0,
                                               'note': NOTE.get((f, item if key != 'ns4' else item), NOTE.get((f, item.split(' ')[0]), ''))})
    rec['lines'].append(line); rec['count'] += 1


files = subprocess.run(['git', '-C', REPO, 'ls-tree', '-r', '--name-only', REV, R], capture_output=True, text=True).stdout.split()
for path in files:
    if not path.endswith('.rs') or '/tests/' in path or path.endswith('/tests.rs'):
        continue
    f = path[len(R):]
    src = subprocess.run(['git', '-C', REPO, 'show', REV + ':' + path], capture_output=True, text=True).stdout
    code = clean(src)
    C, L = code.split('\n'), src.split('\n')
    document = f.startswith('canvas/') or f.startswith('board/')
    for i, (cl, rl) in enumerate(zip(C, L)):
        if PAINT.search(cl):
            for m in re.finditer(r'"([A-Z][A-Z ]*[A-Z])"', rl):
                if m.group(1) not in ALLOW:
                    add('ns1', f, fn_at(C, i), m.group(1), i + 1)
        if not document:
            for m in MESH.finditer(cl):
                add('ns4', f, fn_at(C, i), 'Mesh::' + m.group(1) + '()', i + 1)
        for m in MONO.finditer(cl):
            add('ns5', f, fn_at(C, i), m.group(0).rstrip('('), i + 1)
    if not document:
        for m in NS2.finditer(code):
            i = code.count('\n', 0, m.start())
            add('ns2', f, fn_at(C, i), 'BG_BRAND', i + 1)
out = {'schema': 'each set is a list of {file, fn, item, lines, count, note}; P7 compares the multiset of (file, fn, item) with count',
       'revision': REV, 'acronym_allowlist': sorted(ALLOW),
       'scanners': {'ns1': 'literal "[A-Z][A-Z ]*[A-Z]" not in the allowlist, on a line with .text( / label( / RichText::new( / badge / tag( / chip',
                    'ns2': 'if <…active|selected|current> { BG_BRAND, outside canvas/ and board/',
                    'ns4': 'Mesh::default|with_texture|new, outside canvas/ and board/ (document rendering)',
                    'ns5': 'font_mono( or FontFamily::Monospace'}}
for k, v in sets.items():
    out[k] = list(v.values())
    print(k, len(v), sum(r['count'] for r in v.values()))
json.dump(out, open(OUT, 'w'), indent=1)
