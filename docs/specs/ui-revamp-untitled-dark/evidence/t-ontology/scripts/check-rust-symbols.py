import subprocess,re,sys
R='crates/designos-app/src/'
def src(f):
    r=subprocess.run(['git','-C','.','show','8dfaee93:'+R+f],capture_output=True,text=True); return r.stdout if r.returncode==0 else None
def check(sym):
    parts=sym.split('::'); f=parts[0]; path=parts[1:]
    s=src(f)
    if s is None: return 'NOFILE'
    if len(path)==2:
        m=re.search(r'^(\s*)(?:pub(?:\([a-z]+\))? )?mod '+path[0]+r' \{',s,re.M)
        if not m: return 'NOMOD'
        body=s[m.end():]
        return 'ok' if re.search(r'^\s+(?:pub(?:\([a-z]+\))? )?fn '+path[1]+r'\b',body,re.M) else 'NOFN'
    return 'ok' if re.search(r'^\s*(?:pub(?:\([a-z]+\))? )?(?:async )?fn '+path[0]+r'\b',s,re.M) else 'NOFN'
for a in sys.argv[1:]: print(check(a), a)
