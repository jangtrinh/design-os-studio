"""Registry `region` Panel/DevInspect (page `03 Studio`, row `REGION_ROW`). Lines are the v0.7.5 tag: `panels/inspect.rs` `show` (:313-336) and `body` (:338-640), `dev/sections.rs` (:16-368) and
`dev/code_view.rs` `prompt_box` (:174-189). The parts are in `regions_home_dev_parts.py`.

Panel/DevInspect has no registry variants, so it is one plain component, 241 x 751: the inspect body of the right panel in Dev mode, from the cursor's `c.x0` = `region.left() - BORDER_WIDTH` (x
1199) to the window edge and from y 113 (below the `Inspect` / `Plugins` tab row) to the scrollbar track's end at 864 (g-dev-react.layout.json `scrollbar-track`), clipped to it (`clipped`, :318).
Default state: the capture `g-dev-react` scrolled to the top, one selected frame in `Ready for dev` with an annotation and a dev link. The two other captured states (`g-dev-annotate`: the
annotation editor with its pin chips and `Save` / `Cancel`; `g-dev-compare`: the `Changed` status, no annotations or links, and the compare dialog) are the same body with other data: gaps, the
registry gives the record no state. Every rect is the capture's minus the body's origin (1199, 113); the y of each part is the Rust's running `c.y` (header 12, kind row 36, status 62, buttons 90,
then the sections' hairlines at 126, 251, 328, 567 and 702; `ROW_H` 24, `SECTION_TITLE_HEIGHT` 40).
Nested (live): Divider/Hairline (the 5 `hline(c.x0, c.x0 + c.w)` rules, 241 wide), Button/Icon `Xs` resized to 24 (`widgets::icon_button`, the link row's remove cross, sections.rs:351-352). Local,
because no set draws them: the node name and kind row, the three 20 px header actions, the status pill and its two buttons (the pill-shaped brand `primary_button` and the `small_button` ghost:
`BG_SECONDARY` at `RADIUS_CONTROL`, 24 / 20 / 26 / 28 high; Button/Secondary is 32 / 36 / 40), the section titles, the annotation and link rows, the MCP and Code Connect headers, `kv` rows, the
mono prompt box, the `Not started` pill and the description.
Deviations (nearest-tone rule of the 2026-10-31 rulings; each raw colour literal of the Rust is mapped to the nearest theme tone, hue family kept): the status pill is
`DEV_READY.gamma_multiply(0.22)`: the fill is bound to `color/dev-ready` and the paint carries an opacity of 0.22 (`alpha=lit(0.22, ..)`: `fills/0/opacity` in the allow-list, over BG it is
(12, 46, 28));
`Color32::WHITE` text (MCP, Code Connect, Layer properties) is `ICON` (white); `0xB3` text (`Selection details`, `Not started`) is `TEXT_QUATERNARY` (d 16) and `0x8C` text (the description) the
same (d 23); the pill fill `0x38` is `BG_PRESSED`
(d 8). `kv` shows a number in `font_mono` and a word in `font_body`. Descriptions and the prompt are drawn in rows (15 px for egui's layout of `font_body`, 18 for `CODE_LINE_HEIGHT`) rather than
wrapped, so the line breaks are the capture's. Gaps: the scrollbar thumb (`panels::scrollbar`); `Colors`, `Variables`, `Assets` and the rest of the body under `Layer properties` (the region is
clipped there); hover fills (`BG_SECONDARY` on `kv` and the annotation rows); the annotation and link editors (`annotation_editor`, `link_editor`); the `Completed` / `Changed` / no-status button
rows (`status_header`, :75-110); the Dev mode left list (`dev/list.rs`: `Ready for dev` / `Layers` tabs, status rows) and the right panel's `Inspect` / `Plugins` tab row, which are
Panel/LeftLayers and Panel/RightInspector's. Frame names are neutral (`Frame 1`); the compare sets the capture's text on a scratch copy.
"""
from components.regions_home_dev_parts import annotations, code_connect, header, layer_properties, links, mcp, status
from components.regions_home_gap2_dev import SHIFT, component_body, editor, shifted
from components.regions_home_gap2_devlist import list_body
from components.regions_home_parts import REGION_ROW, region

W, H = 241, 751


def inspect(values, env):
    state = values["State"]
    if state == "List":
        return list_body(env)
    if state == "Component":
        root = region("Component", component_body(env), env, W, H)
    else:
        below = [*links(env), *mcp(env), *code_connect(env), *layer_properties(env)]
        kids = [*header(env), *status(env), *annotations(env)]
        kids += [*editor(env), *shifted(below, SHIFT)] if state == "Annotate" else below
        root = region("Annotate" if state == "Annotate" else "Inspect", kids, env, W, H)
    root["clip"] = True
    return root


def component_sets():
    return [{"name": "Panel/DevInspect", "row": REGION_ROW, "axes": {"State": ["Inspect", "Annotate", "Component", "List"]}, "build": inspect, "uses": ["Button/Icon", "Divider/Hairline"]}]
