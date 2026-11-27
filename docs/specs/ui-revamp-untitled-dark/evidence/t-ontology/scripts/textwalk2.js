// read-only: font families (first run), all-caps by slot and length, gradient owners
const pages = ["1664:398802","6277:291692","1686:419019"];
const DARK = "5353:0";
const out = { fonts: {}, monoBySlot: {}, upperBySlot: {}, upperLen: {}, gradBy: {}, frames: 0 };
function slotOf(n) { let p = n.parent; while (p && p.type !== "PAGE") { if (p.type === "INSTANCE") return p.name; p = p.parent; } return "(frame)"; }
for (const pid of pages) {
  const page = await figma.getNodeByIdAsync(pid); await page.loadAsync();
  for (const f of page.children) {
    if (f.type !== "FRAME" || f.name !== "Desktop") continue;
    if (!Object.values(f.explicitVariableModes || {}).includes(DARK)) continue;
    out.frames++;
    for (const n of f.findAll(() => true)) {
      if ("fills" in n && Array.isArray(n.fills)) for (const p of n.fills) if (p.visible !== false && String(p.type).startsWith("GRADIENT")) { const k = slotOf(n); out.gradBy[k] = (out.gradBy[k] || 0) + 1; }
      if (n.type !== "TEXT" || !n.visible) continue;
      const t = n.characters.trim(); if (!t) continue;
      const fn = n.getRangeFontName(0, 1); const fam = fn && fn.family ? fn.family : "?";
      out.fonts[fam] = (out.fonts[fam] || 0) + 1;
      if (/mono|code/i.test(fam)) { const k = slotOf(n); out.monoBySlot[k] = (out.monoBySlot[k] || 0) + 1; }
      const letters = t.replace(/[^A-Za-z]/g, "");
      if (letters.length >= 2 && letters === letters.toUpperCase()) {
        const k = slotOf(n); out.upperBySlot[k] = (out.upperBySlot[k] || 0) + 1;
        const L = letters.length <= 4 ? "2-4" : "5+"; out.upperLen[L] = (out.upperLen[L] || 0) + 1;
      }
    }
  }
}
return out;
