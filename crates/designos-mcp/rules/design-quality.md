<!--
  Adapted from the MIT-licensed `ease-design` skill (`knowledge/design-tells.md` and
  `knowledge/taste-rubric.md`), condensed into DESIGN:OS Studio's own wording (E15-15 §3.3).
-->

## Design Quality & Anti-Generic Rules

Enforce these qualitative standards alongside automated linting:

- **Visual hierarchy**: Establish one primary action per view and one focal point per screen, with >= 3:1 size ratio between display headlines and body copy.
- **Layout rhythm & cards**: Do not wrap every group in a card or nest cards. Avoid a uniform grid of identical cards when a domain layout fits. Use the file's type scale and spacing scale (`var:` / `style:` tokens), <= 3 gap values per screen, and padding increasing with hierarchy (`page >= section >= card >= control`).
- **Buttons & centred controls**: Build buttons, pills, and badges with `"preset": "easeui-button-primary"` / `"easeui-button-secondary"` / `"easeui-badge"` or `"layout": {"direction": "row", "justify": "center", "align": "center"}` (a plain `"frame"` without `preset` or `layout` places labels at top-left).
- **Surface & colour discipline**: Keep gradients, shadows, and corner radii restrained. Use one primary accent per screen, reserve red/amber/green for status, and express dark-mode depth via surface luminance and borders.
- **Icons & illustrations**: Never hand-draw logos, illustrations, or icons with vector paths — use library instances (`kind: "instance"`) or labelled placeholders.
- **Typography & content realism**: Use <= 2 font families, <= 4 font sizes, 45–75 char lines, and `tabular-nums` for numbers. Use real domain copy, never lorem ipsum.
