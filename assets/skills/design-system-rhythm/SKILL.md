---
name: design-system-rhythm
description: Enforce 4px spacing grid, typographic scale, and semantic token bindings across frames.
version: 0.6.0
license: MIT OR Apache-2.0
attribution: DESIGN:OS Studio contributors
---

# Design System Rhythm & Token Discipline

Apply these layout and token rules to every frame and component you create or edit on the canvas:

1. **4px Spatial Grid**: All auto-layout `item_spacing`, `counter_spacing`, and `padding` values must be multiples of `4px` (`4`, `8`, `12`, `16`, `24`, `32`, `48`, `64`). Use `2px` only for optical icon/badge adjustments.
2. **Concentric Corner Radii**: When a child frame sits inside a padded container with radius $R_{\text{outer}}$ and padding $P$, set $R_{\text{inner}} = \max(4, R_{\text{outer}} - P / 2)$.
3. **Semantic Variable Bindings**: Bind fills, strokes, and spacing to document variables whenever a matching token exists in `doc.variables` rather than hardcoding raw hex values.
4. **Typographic Scale**: Keep body copy at `13px–15px` (`line-height: 1.45`), section headings at `18px–24px` (`font-weight: 600`), and captions/badges at `11px–12px`.
