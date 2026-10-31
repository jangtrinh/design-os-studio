// read-only: list pages in order with top-level child types
const out = [];
for (const p of figma.root.children) {
  out.push({ id: p.id, name: p.name });
}
return out;
