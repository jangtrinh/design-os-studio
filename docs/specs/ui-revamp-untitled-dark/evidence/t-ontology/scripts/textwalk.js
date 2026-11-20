// read-only: measure text slots in Dark-mode Desktop frames of the kit's application example pages
const pages = ["1664:398802","6277:291692","1686:419019"];
const DARK = "5353:0";
const res = { frames: [], slots: {}, fonts: {}, upper: 0, upperEx: [], textCaseUpper: 0, total: 0, mono: 0, gradientFills: 0, nodes: 0 };
function slotOf(n) {
  let p = n.parent; const names = [];
  while (p && p.type !== "PAGE" && names.length < 2) {
    if (p.type === "INSTANCE") names.push(p.name);
    p = p.parent;
  }
  return names.join(" < ") || "(frame text)";
}
for (const pid of pages) {
  const page = await figma.getNodeByIdAsync(pid);
  await page.loadAsync();
  for (const f of page.children) {
    if (f.type !== "FRAME" || f.name !== "Desktop") continue;
    const modes = f.explicitVariableModes || {};
    const isDark = Object.values(modes).includes(DARK);
    if (!isDark) continue;
    res.frames.push(f.id);
    const all = f.findAll(() => true);
    res.nodes += all.length;
    for (const n of all) {
      if ("fills" in n && Array.isArray(n.fills)) for (const p of n.fills) if (p.visible !== false && String(p.type).startsWith("GRADIENT")) res.gradientFills++;
      if (n.type !== "TEXT" || !n.visible) continue;
      const t = n.characters.trim(); if (!t) continue;
      res.total++;
      const fam = typeof n.fontName === "object" && n.fontName.family ? n.fontName.family : "mixed";
      res.fonts[fam] = (res.fonts[fam] || 0) + 1;
      if (/mono|code|courier|menlo|consolas/i.test(fam)) res.mono++;
      if (n.textCase === "UPPER") res.textCaseUpper++;
      const letters = t.replace(/[^A-Za-z]/g, "");
      if (letters.length >= 2 && letters === letters.toUpperCase()) { res.upper++; if (res.upperEx.length < 8) res.upperEx.push([t.slice(0, 16), slotOf(n)]); }
      const k = slotOf(n);
      const s = res.slots[k] || (res.slots[k] = { w: [], c: [] });
      s.w.push(t.split(/\s+/).length); s.c.push(t.length);
    }
  }
}
return res;
