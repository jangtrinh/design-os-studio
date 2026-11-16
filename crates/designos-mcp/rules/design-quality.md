<!--
  Adapted from the MIT-licensed `ease-design` skill (`knowledge/design-tells.md` and
  `knowledge/taste-rubric.md`), condensed into DESIGN:OS Studio's own wording (E15-15 §3.3).
-->

## Design Quality & Anti-Generic Rules

- **Visual hierarchy**: One primary action per view, >= 3:1 headline-to-body.
- **Layout & cards**: Do not wrap every group in a card or nest cards. No uniform grid of identical cards. Use the type scale and spacing scale (`var:` / `style:` tokens), <= 3 gaps, padding `page >= section >= card >= control`.
- **Centred controls**: Buttons, pills, badges: preset `easeui-button-primary` / `easeui-button-secondary` / `easeui-badge`, or `layout` row with `justify` and `align` center (plain `"frame"`: labels top-left).
- **Surface & colour**: Restrain gradients, shadows, and corner radii. One primary accent per screen; red/amber/green for status.
- **Icons & illustrations**: Never hand-draw logos, illustrations, or icons as vector paths. Use library instances or labelled placeholders.
- **Typography**: <= 2 font families, <= 4 sizes, 45–75 char lines. Real copy, no lorem ipsum.
