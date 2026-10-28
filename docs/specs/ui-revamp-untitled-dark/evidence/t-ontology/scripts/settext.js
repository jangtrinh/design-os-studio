// read-only: word/char counts of text inside selected kit component sets (Dark/Light identical text)
const sets = { "Dropdown menu": "3281:383083", "Context menu": "11547:540792", "_Dropdown menu list item": "11546:537565", "Tooltip": "1052:489", "Notification": "1135:618", "Modal": "4057:415205", "_Modal header": "4055:413620", "Tree view": "12451:566966", "_Tree view item": "12450:557173", "Command bar": "3307:407570", "_Command dropdown menu item": "3307:404956", "Badge": "1046:3819", "Empty state": "1182:317", "Horizontal tabs": "1118:69893", "Section header": "190:4871" };
const out = {};
for (const [name, id] of Object.entries(sets)) {
  const n = await figma.getNodeByIdAsync(id);
  if (!n) { out[name] = { missing: true }; continue; }
  const texts = n.findAll(x => x.type === "TEXT" && x.visible).map(x => x.characters.trim()).filter(Boolean);
  const w = texts.map(t => t.split(/\s+/).length), c = texts.map(t => t.length);
  const upper = texts.filter(t => { const l = t.replace(/[^A-Za-z]/g, ""); return l.length >= 2 && l === l.toUpperCase(); }).length;
  out[name] = { type: n.type, n: texts.length, w, c, upper };
}
return out;
