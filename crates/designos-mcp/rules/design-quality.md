<!--
  Adapted from the MIT-licensed `ease-design` skill (`knowledge/design-tells.md` and
  `knowledge/taste-rubric.md`), condensed into DESIGN:OS Studio's own wording (E15-15 §3.3).
-->

## Design Quality & Anti-Generic Rules

Enforce these qualitative standards alongside automated linting:

- **Visual hierarchy**: Establish one primary action per view and one focal point per screen. Keep at least a 3:1 size ratio between display headlines and body copy. Never give primary, secondary, and tertiary actions equal visual weight.
- **Layout rhythm & cards**: Do not wrap every group in a card, and never nest cards inside cards. Avoid a uniform grid of identical cards when a domain-specific layout fits. Use the file's type scale and spacing scale (`var:` / `style:` tokens), with <= 3 gap values per screen and padding increasing with hierarchy (`page >= section >= card >= control`).
- **Surface & colour discipline**: Keep gradients, shadows, and corner radii restrained and consistent with tokens. Use one primary accent per screen, reserve red/amber/green for semantic status, and express dark-mode depth via surface luminance and subtle borders.
- **Icons, logos & illustrations**: Never hand-draw logos, illustrations, or icons with vector paths — instantiate library components (`kind: "instance"`) or use a labelled placeholder frame.
- **Typography**: Use the file's type scale (<= 2 font families, <= 4 font sizes per screen). Keep body line length 45–75 chars, and use `tabular-nums` for prices, metrics, timestamps, and tables.
- **Content realism**: Use real content, never lorem ipsum (`placeholder-text` flags `"Lorem ipsum"`, `"Card 1 / Card 2 / Card 3"`, `"John Doe"`, `"$99.99"`, `"123456"`). Populate screens with domain-specific labels, realistic uneven numbers, and varied states.
