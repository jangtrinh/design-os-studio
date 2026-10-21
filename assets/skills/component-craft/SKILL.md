---
name: component-craft
description: Structure reusable components with variants, slots, and resilient auto-layout constraints.
version: 0.6.0
license: MIT OR Apache-2.0
attribution: DESIGN:OS Studio contributors
---

# Component Architecture & Auto-Layout Craft

Structure reusable components so they scale cleanly across variants and screen sizes:

1. **Auto-Layout First**: Wrap multi-element components in `Horizontal` or `Vertical` auto-layout containers with explicit `Primary` and `Counter` sizing (`Hug` for badges/buttons, `Fill` for responsive cards and rows).
2. **Component Properties & Slots**: Expose text labels as `Text` properties, optional badges/icons as `Boolean` properties, and replaceable content regions as `Slot` containers.
3. **Variant Matrix Hygiene**: Name variants using `Property=Value` pairs (for example `Variant=Primary, Size=Medium, State=Default`) and keep layer names consistent across variants so instance overrides are preserved on swap.
4. **Overflow & Truncation**: Test text layers with both short and long strings so labels wrap or elide cleanly without overlapping sibling controls.
