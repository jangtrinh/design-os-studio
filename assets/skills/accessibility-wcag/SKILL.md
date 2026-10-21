---
name: accessibility-wcag
description: Verify WCAG 2.2 AA contrast ratios, 32px minimum tap targets, and semantic layer naming.
version: 0.6.0
license: MIT OR Apache-2.0
attribution: DESIGN:OS Studio contributors
---

# Accessibility & WCAG 2.2 AA Craft Rules

Ensure every interactive screen and component meets WCAG 2.2 AA requirements:

1. **Contrast Ratios**: Normal text (`< 18px` or `< 14px bold`) must maintain at least `4.5:1` contrast against its resolved background surface; large text and interactive icons must maintain at least `3.0:1`.
2. **Hit Targets**: Interactive controls (buttons, inputs, icon buttons, tabs, checkboxes, and list rows) must have a minimum bounding box of `32x32 px` on desktop and `44x44 px` on mobile frames.
3. **Semantic Layer Hierarchy**: Name frames and interactive layers descriptively (`Header / Primary Nav`, `Button / Submit Order`, `Input / Email Address`) instead of generic `Frame 1` or `Rectangle 4`.
4. **Visible Focus & Error States**: Pair color-coded status indicators with an explicit text label or icon so state is never communicated by color alone.
