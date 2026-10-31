// read-only: fill variable of selected/current variants in row, tab and menu sets
const sets = { "_Tab button base": "43:2412", "_Tree view item": "12450:557173", "_Dropdown menu list item": "11546:537565", "_Nav item base": null };
const out = {};
async function fillVar(n) {
  const b = n.boundVariables && n.boundVariables.fills && n.boundVariables.fills[0];
  if (!b) return "unbound";
  const v = await figma.variables.getVariableByIdAsync(b.id); return v ? v.name : "?";
}
for (const [name, id] of Object.entries(sets)) {
  if (!id) continue;
  const set = await figma.getNodeByIdAsync(id);
  const res = {};
  for (const c of set.children) {
    const p = c.variantProperties || {};
    const sel = Object.entries(p).find(([k, v]) => /current|selected|active|open/i.test(k) && /true|yes/i.test(v));
    const state = p.State || p.state || "";
    const key = (sel ? "selected" : "not-selected") + (state ? "/" + state : "");
    const fv = await fillVar(c);
    res[key + " -> " + fv] = (res[key + " -> " + fv] || 0) + 1;
  }
  out[name] = res;
}
return out;
