# DESIGN:OS Studio Implementation Specifications

This directory contains engineering specifications, architectural designs, and clean-room reverse engineering dossiers for DESIGN:OS Studio.

## 🚀 Epic & Figma 126.11.7 REA Implementation Specs (Reference)

All 24 GitHub Epics (`#14` through `#1788`), Proactive Epic `E21` Multiplayer, all 56 Open Parity/UX Bugs, and Figma Desktop `126.11.7` reverse-engineered clean-room implementation specs and REA dossiers live under [`epics-figma-rea/`](epics-figma-rea/):

- **Master Execution Playbook & Index**: [`epics-figma-rea/EPIC-FIGMA-SPECS-INDEX.md`](epics-figma-rea/EPIC-FIGMA-SPECS-INDEX.md)
  - Complete 24-Epic Master Matrix (`#14` – `#1788`) + Proactive Epic `E21`
  - Clean-Room Figma `126.11.7` REA Evidence Dossiers Inventory (incl. Multiplayer & 56 Open Bugs)
  - Hot-File Queue & Engine Safety Invariants
  - Acceptance Test Commands & Parity Criteria
- **Clean-Room REA Dossiers**: [`epics-figma-rea/evidence/`](epics-figma-rea/evidence/)
- **Wave Implementation Specs**: [`epics-figma-rea/`](epics-figma-rea/) (directories `e1-*` through `e20-*`, `tables-0710/`, `finalize-e1-e2-e3/`)
- **Proactive Epic E21 Multiplayer**: [`epics-figma-rea/e21-multiplayer-realtime-sync/`](epics-figma-rea/e21-multiplayer-realtime-sync/) (Real-Time Multiplayer & Multi-Agent Co-Editing)
- **Open Parity & UX Bugs REA Specs**: [`epics-figma-rea/open-bugs-rea/`](epics-figma-rea/open-bugs-rea/) (56 Open Parity/UX Bugs across the 570-issue repo audit)

The specs are reference material to implement faster, not a contract. Before work on an Epic or one of its issues, read [`EPIC-FIGMA-SPECS-INDEX.md`](epics-figma-rea/EPIC-FIGMA-SPECS-INDEX.md) (for an open bug, also [`open-bugs-rea/README.md`](epics-figma-rea/open-bugs-rea/README.md)) and the matching spec, and take from it what helps: data models, Kiwi field tables, acceptance ideas, slice order.
A reviewed spec carries its review status at its top and in the index (a spec without one has not been reviewed yet); read the linked [review](epics-figma-rea/reviews/README.md) first, it lists the known errors and the corrections.
Check every symbol, path and line number against main before relying on it.
When a spec disagrees with code on main, the CHANGELOG or an owner ruling, the spec loses.
A statement about Figma's behaviour is a fact only with a checkable source (the Kiwi schema, a Figma-authored file, Figma's documentation or interface strings, a recorded observation); without one, treat it as a hypothesis and verify it before relying on it.

---

## Historical Version Specs

- `v0.5/` — v0.5 architecture specifications
- `v0.6/` — v0.6 architecture specifications
- `v0.6.x/` — v0.6.x point release specifications
- `v0.7/` — v0.7 architecture specifications
