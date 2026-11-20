---
id: detail-figma-live-variable-check
type: note
title: "Figma live variable check"
updated: "2026-10-25"
status: draft
---


Owner asked to use the Ease Design Figma Agent if needed (21:19). Read-only `figma-agent exec-js` scripts run against the open file
`❖ Untitled UI Figma – PRO VARIABLES (v8.0) gKXp7LqFY2NV (Copy)` (fileKey VEffelx01mDl5BJJanwxja). Aliases are resolved in the
script: a target collection that lacks the mode falls back to its default mode. The output was compared with Studio's
`.fig` import (`--export-variables`, the resolved table from `resolve-untitled-tokens.py`).

| Check | Figma | Studio import | Result |
|---|---|---|---|
| Colour variables, Dark mode (`1. Color modes`) | 273 | 273 | 273/273 identical (hex + alpha) |
| Colour variables, Light mode | 273 | 273 | 273/273 identical |
| Spacing, radius, widths, containers (numbers) | 76 | 76 | 76/76 identical once units are normalised |
| `6. Typography` variables | 32 | 1 | **Studio's export lacks 31 of 32**; this adds to spec P (DTCG export drops collections) |

Collections in Figma: `_Primitives` (Style, 343), `1. Color modes` (Light/Dark, 273), `2. Radius` (11), `3. Spacing` (17),
`4. Widths` (12), `5. Containers` (3), `6. Typography` (32).

The type scale the specs use matches Figma: text-xs 12/18, text-sm 14/20, text-md 16/24, text-lg 18/28, text-xl 20/30, Inter.

Conclusion: every colour and number the token contract takes from the kit equals the live Figma value. Figma is not needed
for values. Studio may still render some kit components differently from Figma (see #1154 for one NodeSpec case).
