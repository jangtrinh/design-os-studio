// read-only: top-level children of selected pages
const ids = ["1:1183","18:0","85:1269","172:4293","12206:573377","3307:403757","43:0","1664:398802","1686:419019","176:4256","1172:31","12:539"];
const out = [];
for (const id of ids) {
  const p = await figma.getNodeByIdAsync(id);
  if (!p) { out.push({id, missing:true}); continue; }
  await p.loadAsync();
  out.push({ id, page: p.name.trim(), children: p.children.map(c => ({ id: c.id, type: c.type, name: c.name, w: Math.round(c.width), h: Math.round(c.height), n: ("children" in c) ? c.children.length : 0 })) });
}
return out;
