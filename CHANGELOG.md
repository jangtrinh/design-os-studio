# Changelog (`design-os-studio`)

All notable changes to `design-os-studio` (**DESIGN:OS Studio**) are documented in this file.

## [0.7.14] - 2026-11-13

A patch release from `main` that keeps the variable bindings of component property defaults in imported Figma files, makes an instance's variant choices and overrides more reliable, and carries more fixes from @NamAIEngineer. Re-solving the imported Untitled UI PRO v8.0 kit now moves 1,910 layers instead of 2,039 (#1920).

### Added
- **Components.** A component property (boolean, text or variant) whose default is bound to a variable in Figma keeps that binding: it is read on `.fig` and Figma REST import, for a variable of the same file or of a remote library, written back on `.fig` export, and listed among the component's bound variables by the design-system scan. Renaming or deleting the property, deleting the variable, publishing to or inserting from a library, a branch merge and turning the property into a slot keep the binding true, and a paste from Figma clears it (#1856, PR #1990).

### Changed
- **Approvals.** The mode Ask for risky only is now called Ask when risky; stored settings keep working (PR #1991).
- **Approvals.** Ask when risky is enforced for every way of starting an agent: a CLI that does not share the approval hook (such as Codex, Grok, Antigravity, Gemini, OpenCode or a custom command) is refused with its reason and the next step, and the composer pill follows the mode. This covers a new chat, resume, the agent picker, image jobs and Run login for Gemini and Antigravity, whose login is the agent itself; logins that are real login commands still run; by @NamAIEngineer (#1435, PR #1991).
- **Approvals.** Grok and Antigravity start on a project config that is exactly Studio's in every mode that uses the approval hook, and a hook file planted in the session folder is moved aside first (PR #1991).
- **Images.** The decoded-image cache holds at most 64 MiB of decoded pixels and evicts the oldest first; one image larger than that is still cached, alone, so it is not decoded again on every paint; by @NamAIEngineer (#1891, PR #1989).
- **Agent tools.** Deleting a layer that is mirrored inside a component instance, with `designos_delete_node` or a batch `delete`, now answers the non-retryable `INSTANCE_CHILD` with a next step (hide the layer or override it); it used to answer `FIELD_NOT_ALLOWED_ON_INSTANCE`, marked safe to retry; by @NamAIEngineer (#985, PR #1947).
- **Agent tools.** Every write to a library variable (delete, value, mode, alias) is refused, atomically inside a batch; variables of local collections stay writable; by @NamAIEngineer (#1804, PR #1947).

### Fixed
- **App shell.**
  - Quitting runs the shutdown cleanup once: the close request and eframe's `on_exit` no longer both flush notes, save the close version and stop the terminals; by @NamAIEngineer (#1648, PR #1684).
  - Work queued before a quit, including work that a queued job queues, is drained before notes, autosave and terminals shut down, also on the AppKit Dock Quit path that skips the close request; by @NamAIEngineer (PR #1786).
  - A large document's Save, which waits one frame so its status can paint, writes the tab that pressed Save even if you switch tabs in between; if that tab was closed meanwhile nothing is written and a toast says so; by @NamAIEngineer (PR #1783).
  - Saving a document of more than 25,000 layers to the library no longer copies it on the UI thread for a thumbnail, the same budget the other thumbnail paths already use; by @NamAIEngineer (#1718, PR #1785).
  - An error toast that is on screen is no longer replaced by a different error before its time is up; the new error waits behind it; by @NamAIEngineer (#1766, PR #1774).
- **Agent CLI.** `--apply-spec` says what it did: the reply names the added root (`root_name`, `parent_id`, `nodes_created`), states `added_new_subtree: true` and `updated_existing_node: false`, and warns with `ROOT_NAME_ALREADY_EXISTS` when a sibling of the same name was left unchanged; by @NamAIEngineer (#1086, PR #1175).
- **Agents panel.**
  - The Sessions popover's rename field stays in step with the row it edits, Enter and Escape act in the same frame, and the Other CLI form opens when the agent menu asks for it or the window is resized; by @NamAIEngineer (#1010, PR #1991).
  - Codex gets no design-server registration for an untitled design that has no workspace, also on resume, and the status line says so; by @NamAIEngineer (#1654, PR #1991).
  - Closing a tab that has waiting messages puts the waiting text back in the composer and says so, instead of losing it; by @NamAIEngineer (#1495, PR #1991).
  - A shell tab keeps its own name on the composer pill (PR #1991).
  - A running agent tab keeps the permission mode it was started in: a later switch of the mode applies to new tabs only, and a canvas change no longer rewrites the approval script the other tabs use; by @NamAIEngineer (#1970, PR #2010).
- **Auto layout.**
  - Re-solving an older frame with auto spacing ("space between") no longer moves what Figma leaves in place: a frame of layout version 0 with exactly one flow child that is not Fill centres that child, from layout version 1 the gap never goes below zero, and Fill children reserve no item spacing under space between, around and evenly. Re-solving the imported kit moves 1,926 layers instead of 2,039 (#1920, PR #1946).
  - A wrapped auto-layout frame whose lines leave room on the counter axis places the block of lines by the frame's counter alignment: centred for Center, at the far padding edge for End, as Figma does. Re-solving the imported kit moves 1,910 layers instead of 1,926 (#1920, PR #1979).
- **Components.**
  - A value picked by hand for one variable-bound variant property now holds only that property until it is reset, and the other bound properties keep following their variables. Before, one hand pick on a nested instance froze every bound property of it, and a pick made through the API on a top-level instance was undone by the next mode change. A nested `variant` entry in `designos_apply_spec` or a batch pins the property too, and picks survive a `.fig` save and reopen (#1901, #1902, PR #1950).
  - A frame that hugs an instance takes its new size in the same step as a variant switch that a variable drives, instead of keeping the old size until the next edit, also through undo and redo and for `set_variables` through the agent tools; by @NamAIEngineer (#1903, PR #1977).
  - A frame that hugs its content also takes its new size when the switched instance sits inside one or more groups: the groups are refitted to their children and the frame follows in the same step, for a switch that a variable drives, a variant picked by hand and an instance swap, also through undo and redo; by @NamAIEngineer (#1976, PR #2013).
  - A change made on a layer inside a nested component copy (a copy inside a copy) is no longer lost after a `.fig` save, reopen and an edit of a main component. Files saved by an earlier release are repaired when they are opened (#1925, PR #1973).
  - A variant switch or an instance swap counts every override it has to drop and reports it, per field; by @NamAIEngineer (#1915, PR #1988).
  - A text override is carried to the same-name layer of the new variant on a variant switch instead of being dropped; by @NamAIEngineer (#1916, PR #1988).
  - The new variant is refitted to the size the instance keeps after a variant switch, also for a swap by hand, an auto-layout instance and stretching children; by @NamAIEngineer (#1917, PR #1988).
  - An overridden text's box follows the overridden text after a variant switch when the text styles of both variants match; a size override without a text override is not carried into a variant whose box differs; by @NamAIEngineer (#1918, PR #1988).
  - A main component sync keeps an instance's own corner radius: the stored outline is dropped only when it is stale, and kept when the instance also overrides its size; by @NamAIEngineer (#1935, PR #1988).
  - Every edit the cycle guard refuses (a move, paste, instance swap, variant or instance-swap property, spec or replayed live patch) gets the same sentence naming the loop, in the editor and in the agent tools. `designos_update_node` and `designos_apply_spec` now refuse with `INSTANCE_CYCLE` a property edit that would make a main component hold itself, and leave the document unchanged (#1896, PR #1971).
- **Canvas.**
  - Dragging the corner radius of an imported rectangle or frame that carries a baked outline redraws it; the same fix covers components, instances, sections, and a radius edit on an imported polygon or star in the Properties panel (#1908, PR #1934).
  - A corner radius drag that leaves the radius as it was changes nothing: the shape keeps its stored outline, no undo step is added and the document is not marked changed; by @NamAIEngineer (#1939, PR #1989).
  - Paste properties and match style drop the stored outline of an imported shape when the paste changes its corner radii or corner smoothing, and keep it when it does not; by @NamAIEngineer (#1936, PR #1989).
  - Every arrow cap on a vertex has its label in the cap list instead of reading "Mixed"; by @NamAIEngineer (#1765, PR #1989).
  - Binding one corner radius of a star, polygon or ellipse to a variable drops the stale stored outline, so the new radius shows; by @NamAIEngineer (#1986, PR #2006).
  - A background blur with a radius above 32 that touches the edge of the canvas is no longer brighter at the edge than the CPU reference; the resample step treats texels outside the canvas as transparent, as the blur does; by @NamAIEngineer (#1689, PR #2005).
- **Export.**
  - A `.pen` export writes the ring arc that an instance draws itself, and the importer reads it back as the instance's own arc; before, a reopened instance showed the main component's arc (#1906, PR #1933).
  - `designos FILE.fig --export-variables OUT.json --format figma-rest` marks the variables of library collections `"remote": true` instead of `false` on every variable; by @NamAIEngineer (#1854, PR #1928).
- **Figma files.** Opening a page that was not opened before, after its main component was edited, shows the instances at the main's new size instead of the stale size stored in the file, and keeps their overrides; by @NamAIEngineer (#1888, PR #2011).
- **Agent tools.**
  - `designos_export_spec` exports the ring arc that a nested instance draws itself, and the arc applies back; by @NamAIEngineer (#1937, PR #1988).
  - An insert whose id, or the id of any layer under it, already exists in the document is refused instead of overwriting the layer that holds it; by @NamAIEngineer (#1740, PR #1941).
  - Replaying an agent's table edit keeps the row heights and column widths the user set in the editor; by @NamAIEngineer (#1817, PR #1941).
  - The 4 px grid fix and the touch-target fix leave connectors alone, so a fix no longer bends a connector or resizes it to 44 x 44; by @NamAIEngineer (#1710, PR #1941).
  - Comment and reply messages are stamped with the time they are written; by @NamAIEngineer (#714, PR #1947).
  - Read tools fill in a deferred page before they read it, so instances from a `.fig` no longer come back without their content; `designos_get_app_state` does the same for the active page and for the page of every layer it is asked about; by @NamAIEngineer (#1307, PR #1947).
  - A batch that moves a layer to another page re-homes or detaches its connectors the way Studio does; by @NamAIEngineer (#1663, PR #1947).
  - A batch `apply_spec` into a fixed frame no longer moves layers outside that frame, and a hug frame still grows; binding a fixed frame's padding or stroke re-solves that frame only; by @NamAIEngineer (#1702, #1704, PR #1947).
  - The rules pack keeps the `needs_human` retry guidance; by @NamAIEngineer (#1399, PR #1947).
  - The documented error list of `designos_update_node` names `UNKNOWN_COMPONENT_PROPERTY` and `OVERRIDE_FIELD_NOT_APPLICABLE`, so an agent can tell these refusals from a transport failure and correct its call instead of retrying; by @NamAIEngineer (#1938, PR #2002).
- **Windows.** The documented PowerShell install command parses again on Windows PowerShell 5.1: `install.ps1` is ASCII only, and the shortcut text is unchanged; by @NamAIEngineer (#1313, PR #1929).

### Known issues
- A native file in which an instance has its own ring arc cannot be opened by Studio 0.7.12 or older.
- Re-solving the imported Untitled UI PRO v8.0 kit still moves 1,910 layers, about 1% (#1920).
- A nested instance can lose its overrides when its variant is switched by hand and back (#1909). An override on a layer the other variant lacks is still lost on a variant switch; the switch now reports it (#1915).
- A main component sync with no change can resize instances imported from a `.fig` (#1919).
- In the prototype player, a "Set variable mode" or "Set variable" action does not switch the variants bound to that variable (#1904).
- The mode control of the Agents panel shows the mode for new tabs, not the mode the focused tab runs in (#2008).
- The variable-bound nested variant is checked on files made in Studio only: the kit has no bound component property, and no file authored in Figma with one was available.

## [0.7.13] - 2026-11-12

A patch release from `main` that finishes the basics of auto layout and components for imported Figma files. Re-solving every auto-layout frame of the imported Untitled UI PRO v8.0 kit moved 61,948 of 194,397 layers by more than 0.5 px before this release and moves 2,039 now (#1669). A nested instance whose variant is bound to a variable now follows the mode where it sits (#1824, PR #1912). It also includes fixes carried from @NamAIEngineer.

### Fixed
- **Auto layout.**
  - A wrapped auto-layout frame from a `.fig` or Figma REST import that has no counter-axis gap takes its item spacing as the line gap, so wrapped rows no longer collapse onto each other (#1881, PR #1884).
  - A Fill child held back by a max or min size is placed by the frame's counter alignment, so a 1280 px max-width child in a centred 1440 px frame sits at 80 instead of 0 (#1880, PR #1886).
  - Rotated and flipped children are laid out by their visual bounding box, as Figma does: a 72 x 18 text turned 270 degrees takes 18 px of a row, not 72. A child at any other angle keeps its own size. In a file made in Studio, a flipped child moves by its own size on the next relayout, to the place Figma gives it (#1882, PR #1897).
  - A `.fig` auto-layout frame with no right or bottom padding imports it as 0 instead of copying the left or top value, so end-aligned and centred frames no longer shift their children and hug frames no longer grow (#1668, PR #1671).
- **Components.**
  - A `.fig` save keeps the overrides of an instance on a page that was never opened after its main component is edited; a reopened file showed "Button CTA" where it should show "Go back"; by @NamAIEngineer (#1695, PR #1889).
  - When the cycle guard refuses an edit, such as an instance swap, a variant switch or a layers-panel drop, an error toast now says why, for example `Can't swap: Card would contain itself (Card → Icon → Row → Card)`. A drop or paste that lands outside the container says where it went; by @NamAIEngineer (#1837, PR #1899).
  - An instance keeps its own ring arc (start, sweep, inner radius) when the main component's arc changes, through a variant swap and a native or `.fig` save and reopen, and Reset gives the main's arc back. Dragging a ring's sweep, start or inner radius now redraws rings imported from Figma, and the same fix covers star and polygon handles and the polygon corner radius (#1727, PR #1911).
  - A nested instance whose variant is bound to a variable switches variant when the variable resolves differently where the copy sits: a mode set on the host frame, an ancestor, the wrapper instance or the page, a changed value or default mode, or a move or paste into a frame with another mode. Text, fill and inner swap overrides are kept, and a variant picked by hand holds until reset (#1824, PR #1912).
- **Export.**
  - Dashed, gradient and width-bound per-side Inside strokes on a grid that are wider than the padding export as an inset shadow with the padding the layout reserves, so content no longer shifts. CSS and HTML carry a short note where CSS has no border-free primitive; by @NamAIEngineer (#1792, PR #1883).
  - SwiftUI export splits unequal flexible grid columns, such as 2fr 1fr, by their weights inside a `GeometryReader`, so the columns follow the container; it needs iOS 16 or macOS 13 (#1823, PR #1892).
- **Canvas.** The grid overlay takes its row gap, column widths and row heights from the layout solver, so the gap bands sit in the real gaps instead of across the cards; by @NamAIEngineer (#1691, PR #1893).

### Changed
- **Colour.** In a Display P3 document the converted-image cache is capped at 64 MiB, releases images that nothing can show, and converts an oversized image once instead of on every eyedropper loupe frame. Swatches follow the profile in the same frame after undo, redo or a profile change, and closing the active library tab saves its pending edit first; by @NamAIEngineer (#1805, PR #1890).

### Internal
- A properties-panel spacing check passes again: the linked Animation style row uses the row spacing token, with the same pixels as before (PR #1874).
- Tests pin that Card and Button main syncs, repeated syncs, a swapped nested instance and every horizontal constraint keep a nested Stretch instance's size and position, and that the editor resize-then-edit path keeps its width. The snap-back no longer reproduced on `main`, so this closes the issue with tests only (#1445, PR #1879).
- Tests pin that a vector resized inside an instance keeps its size and scaled paths through a `.fig` save, a reopen and a main sync, including on a page that was never opened. The behaviour already held, so this closes the issue with tests only (#1725, PR #1910).
- The layout-unchanged check pairs unnamed widgets by closest size and position instead of by order, so inserting one no longer reports false drift, and a real move is still reported; by @NamAIEngineer (#1789, PR #1894).

### Known issues
- A native file in which an instance has its own ring arc cannot be opened by Studio 0.7.12 or older.
- Re-solving the imported Untitled UI PRO v8.0 kit still moves 2,039 layers, about 1% (#1920).
- A nested instance can lose its overrides when its variant is switched by hand and back (#1909). An override on a layer the other variant lacks, and a text override when the two variants hold different default text, are lost on a variant switch (#1915, #1916).
- A main component sync with no change can resize instances imported from a `.fig` (#1919).
- The variable-bound nested variant is checked on files made in Studio only: the kit has no bound component property, and no file authored in Figma with one was available.
- After a `.fig` save and reopen, a change made inside a nested instance can be lost at the next edit of the main component (#1925). It also affects 0.7.12 and older files saved natively after such an import.

## [0.7.12] - 2026-11-11

A patch release from `main` that finishes the Styles & Variables epic. Variables now bind to effects, glass, size limits, typography, easing and durations. Animation styles are new, and extended collections work across libraries and branch merges. It also includes fixes carried from @NamAIEngineer.

### Added
- **Variables.**
  - Effect parameters (blur radius, spread, offset x and y, opacity) and the glass parameters (light angle, light intensity, refraction, depth, dispersion, frost) bind to number variables. They resolve per mode, round-trip through `.fig` and work from MCP. The glass popover gets a bind slot on each field (#132, PR #1869).
  - Min and max width and height bind to number variables and feed auto layout. Font family, font style, paragraph spacing and paragraph indent can be bound on text layers, and hug text re-measures when a mode switch changes it (#133, PR #1871).
  - Easing and duration variables bind to prototype transitions and Motion tracks. An easing names a Studio preset, and a duration is in milliseconds (#143, PR #1872).
  - Extended collections can be published to a library and used from another file, including edits in the receiving file's own modes and `.fig` round trips (#1795, PR #1866).
- **Styles.** Animation styles: a style holds Motion timelines, layers link to it like effect and grid styles, it round-trips through `.fig`, and the style picker lists it (#141, PR #1870).
- **Motion.** Motion timelines and tracks import from and export to `.fig` (#209, PR #1845).
- **CLI.** `designos FILE.fig --scan-design-system OUT.json` writes the design-system scan file from a `.fig` (#1853, PR #1855).

### Fixed
- **Text styles.**
  - Editing a shared text style updates the properties a linked layer does not override and keeps the ones it does, as Figma does (#1728, PR #1868).
  - Lint fix, the command menu's auto-fix and the whiteboard text size detach a linked text style by the same rule as the inspector; by @NamAIEngineer (#1747, PR #1819).
- **Variables.**
  - Boolean operands, background blur and connector ends draw the values their variables resolve to; by @NamAIEngineer (#1736, PR #1791).
  - The Export preview redraws after a variable value or an ancestor's mode changes; by @NamAIEngineer (#1737, PR #1780).
  - A batch of variable value writes is all-or-nothing, and an extension's own variable list cannot grow past its rule; by @NamAIEngineer (#1807, PR #1860).
  - A branch merge that renames a colliding collection keeps its extensions pointing at the right parent and modes, and refuses a merge that closes a new alias cycle (#1803, PR #1867).
  - MCP re-measures bound hug text when a collection's modes change (#1341, PR #1846).
- **`.fig` import.** Effect style detaches on instances survive in any entry order, and a main linked to a library style the file lacks keeps its own bound effect values (#1808, PR #1865).
- **Components.** A main sync lays out instance children by their constraints again (#1656, PR #1847).
- **Export.**
  - Tailwind writes Center and Outside strokes as outlines, like CSS (#1787, PR #1849).
  - Native grid code writes an included stroke's inset as padding (#1829, PR #1851).
- **Colour picker.** The picker caches its binding context, so it opens faster on large files (#1443, PR #1848).

## [0.7.11] - 2026-11-10

A feature release from `main`: tables on the canvas, Display P3 colour, extended variable collections, instance properties bound to variables, text styles that survive weight and italic overrides, and a `.fig` save that keeps the overrides of instances nested in a main component, plus fixes carried from @NamAIEngineer.

### Added
- **Tables.**
  - A document can hold a table: ordered rows and columns with stable ids, sizes and merged cells, repaired on load and readable over MCP (PR #1742). Each table edit is one undo step (PR #1745).
  - Tables draw on the CPU and GPU renderers and in PNG, SVG and PDF exports (PR #1773).
  - Tables can be edited on the canvas: select a cell and type, Tab and Shift+Tab between cells, row and column grips to select and reorder, a "+" on a line to insert, drag a line to resize, and a right-click menu to add, remove, merge and unmerge (PR #1815).
  - Cell text grows its row, clips on the line box and survives fonts that load late (PR #1814).
  - Agents can create, fill, edit and style a table with the `table` op of `designos_apply_batch`, in one batch and one undo step (PR #1797), and their table edits replay into the same table in a live session (#1796, PR #1822).
- **Colour.**
  - A document can be set to Display P3 colour from the Page title row. Colours are stored as authored, shown converted to sRGB on screen, and tagged as Display P3 in PNG, JPEG, SVG and PDF exports (#136, PR #1812).
  - One Colour format export setting (Hex, RGB or OKLCH) drives CSS, Tailwind, React, HTML and Dev Mode; Hex stays the default. In a Display P3 document, a colour outside sRGB is written as `color(display-p3 …)`. The colour picker can show and edit OKLCH (#137, PR #1827).
- **Variables.** Extended variable collections: a collection can extend an extendable parent and override any of its variables per mode, as in Figma. The panel, canvas, code generation, `.fig` and Figma REST import and export, and MCP `set_variables` all follow the same rules (#135, PR #1813).
- **Components.**
  - Boolean, text and variant properties of an instance can be bound to variables from the inspector, and by agents with `prop_vars` on `designos_update_node` (#134, PR #1833).
  - Swapping an instance or switching its variant carries your overrides into the new component the way Figma does, including nested swaps and a single text label (#1433, PR #1831).
  - Create component says "Create multiple components" when several layers are selected (#1472, PR #1825).
- **Text styles.** A text layer keeps its text style when you change its weight, italic or underline, or add a hyperlink, and shows a count pill on the style row. "Reapply style" restores the style's values in one undo step, and `.fig` round trips keep the style link and the overrides (#1448, PR #1749).
- **Export.** Native grid code for Android XML, Compose, SwiftUI and UIKit writes flexible columns as weights instead of fixed widths (#1456, PR #1828).
- **Motion.** The document gains a Motion model of timelines, tracks and keyframes, and timelines can be evaluated at a given time. There is no Motion UI yet (#140, PR #1811; #142, PR #1826).
- **Dev Mode and inspector.** Dev mode, the right panel header and its Inspect and Prototype tabs move onto the Untitled UI dark kit's roles and sizes (PR #1714, PR #1794, PR #1763, PR #1768).

### Fixed
- **`.fig` save.**
  - A `.fig` save keeps the overrides of instances nested inside a main component, including instances on pages that were never opened (#1649, PR #1664).
  - Effect style links on instances are kept through import and save; builds on PR #1685 by @NamAIEngineer (#1494, PR #1810).
  - A slot that lists only preferred components no longer gets an empty config on save, and a detached preferred component is saved (PR #1750).
  - A resized imported line keeps its stroke band instead of jumping down by half its stroke width (#953, PR #1752).
- **Rendering.** Imported inside strokes, stars, polygons and rings draw like Figma (#952, PR #1734).
- **Layout.** Content that overflows an End- or Center-aligned auto layout frame keeps the place Figma stores, and an MCP edit re-solves only the frames it touches (#1665, PR #1667).
- **Export.** Exported CSS and Tailwind grids put strokes, padding and tracks where the layout solver puts them (PR #1793).
- **Text styles.**
  - Typography variable bindings are kept when a text style is applied or created; carried from PR #1677 by @NamAIEngineer (PR #1733).
  - K scale, paste properties and re-picking a value detach the text style by the same rule as the other typography controls (#1432, PR #1830).
- **Canvas.**
  - Drag edits such as corner radius, spacing and padding are kept when a release and the next press land in the same frame (PR #1732).
  - Moving a layer that has an export row no longer redraws its export preview, which froze the canvas on large files (PR #1739).
  - Selection chrome no longer runs through connector labels and magnet dots (PR #1751).
  - A slot's dashed box and plus button stop at its clipping frame (#1583, PR #1760).
  - Board popovers, the Zoom menu and the layer picker pick on release, like the other menus (#904, PR #1708).
  - Whiteboard items on Design pages keep their kind and content through load, lint and save (PR #1701).
- **Colour picker.** Gradient stops bound through a style are locked, with a read-only pill (#1446, PR #1832). An open picker shows the document's colour after undo or redo; carried from PR #1630 by @NamAIEngineer (PR #1688).
- **App.** Toasts with an action are queued instead of replaced, and a failed Show in folder says so (#937, PR #1767).
- **Windows.** A save no longer fails when antivirus, an indexer or another save briefly holds the file; carried from PR #1646 by @NamAIEngineer (PR #1688).
- **Effects.** Background blur at the canvas edge no longer shows a smeared stripe on the GPU; carried from PR #1683 by @NamAIEngineer (PR #1688).

## [0.7.10] - 2026-11-07

A hotfix release from `main`: a save no longer writes a file that is missing the content of pages that were never opened, and it also brings live connectors to design files, a more faithful `.fig` save, and fixes carried from @NamAIEngineer.

### Added
- **Connectors.**
  - Design files get live connectors. The Connector tool sits in the shape dropdown next to Line and Arrow (shortcut X), sticks to two layers, and re-routes when either one moves. Decks do not offer it. A new connector is dark, or white where dark ink would be hard to read on the surface behind it (#1222, PR #1653).
  - Agents can draw a live connector between two layers with the `connect` op of `designos_apply_batch`, with magnets, routing, end caps and a label (#1227, PR #1623).
  - A move re-routes only the connectors it touches, and connectors are re-routed after auto layout has shifted other layers, in the same undo step (#1223, PR #1610).

### Fixed
- **Saving large files.**
  - A save no longer writes a file that is missing the content of pages that were never opened. This hit large `.fig` files opened with deferred pages: their component instances came in empty. A save that cannot load those pages now refuses and says why instead of writing an empty page. Large `.fig` files imported through Home before this fix: import them again from the original `.fig` (#1670, PR #1672).
- **`.fig` save.**
  - A `.fig` save keeps every variable the file uses, deleted ones included, and writes them back as deleted. The "hide from publishing" switch is kept (#1634, PR #1647).
  - Instance fill and stroke overrides are written as the file recorded them, and a paint an instance only inherits is not written. On a clone of the Untitled UI PRO v8.0 kit, missing variable bindings after reopen went from 23 to 0 (#1634, PR #1647).
  - Binding a variable by name matches live variables only, and lint suggestions no longer use deleted variables (#1634, PR #1647).
- **App.**
  - Quitting from the Dock, logout or shutdown on macOS asks "Save changes?" when a tab has unsaved edits, like Cmd+Q (#977, PR #1645).
  - Mouse wheel scrolls full-screen agent CLIs in the terminal, and the main screen keeps its scrollback (#1173, PR #1337).
- **Components and layout.**
  - Nested Stretch instances keep their size when an enclosing instance syncs; carried from PR #1641 by @NamAIEngineer (#1445, PR #1655).
  - A branch merge or live patch that would make a component hold an instance of itself is refused with a message naming the cycle; carried from PR #1633 by @NamAIEngineer (#1629, PR #1655).
  - Hug-width text with a max width wraps at the smaller of its natural width and the max width; carried from PR #1638 by @NamAIEngineer (#1362, PR #1655).
  - Swapping an instance or reparenting a layer that would make a component hold an instance of itself is refused with a plain message; carried from PR #1082 by @NamAIEngineer (#968, PR #1627).
- **Effects and images.**
  - Background blur behind a translucent child of a paintless group blurs at full strength, as in Figma; carried from PR #1632 by @NamAIEngineer (#1626, PR #1658).
  - Large or cropped image fills no longer allocate a full-picture buffer per tile; carried from PR #1609 by @NamAIEngineer (#1603, PR #1627).
- **Lint.** Text contrast findings no longer suggest binding a background variable onto the text layer; carried from PR #1199 by @NamAIEngineer (#1089, PR #1627).
- **CLI and MCP.**
  - `designos --screenshot` writes a non-empty PNG and exits 0, or exits 1 with a line saying why it could not; builds on PR #1453 by @NamAIEngineer (PR #1652).
  - An untitled document gets its own private MCP workspace. If it cannot be made, the agent still starts without the `designos` server and says why; carried from PR #1150 by @NamAIEngineer (#1074, PR #1644).
  - DTCG export keeps collection and mode files whose names differ only by case on case-insensitive filesystems; carried from PR #1374 by @NamAIEngineer (#1325, PR #1644).
- **Security.**
  - Private approval folders are refused when macOS gives them an extended ACL; carried from PR #1659 by @NamAIEngineer (#1651, PR #1687).
  - Prompt text containing "No, suggest changes" no longer locks the composer as if Gemini showed a dialog; carried from PR #1415 by @NamAIEngineer (#1651, PR #1687).
- **Linux.** The `.deb` declares `libxkbcommon-x11-0`, and a startup failure on Linux gives GPU driver recovery steps; carried from PR #1083 by @NamAIEngineer (#715, PR #1644).

### Changed
- A save of a large `.fig` now loads every page first, so it takes longer and writes a bigger file. On the Untitled UI PRO v8.0 kit (release build, Apple M4 Max), Save As `.designos` went from 6.6 s to 38.2 s and the stored file from 1.67 GB to 4.95 GB, because the file now holds all 877,261 nodes instead of 207,601 (#1670, PR #1672).

### Known issues
- On a `.fig` save, overrides set on instances nested inside a main component are still dropped (#1649).

## [0.7.9] - 2026-11-05

A feature release from `main`: image adjustments, a Project tab for docs, faster FigJam connectors that now survive `.fig` files, and a large set of fixes carried from @NamAIEngineer.

### Added
- **Images and effects.**
  - Image fills get Exposure, Contrast, Saturation, Temperature, Tint, Highlights and Shadows sliders with a Reset, and the image fill picker gets rotate buttons and a tile scale slider. Switching Fill or Fit to Crop keeps the framing you see (#163, PR #1595).
  - Custom shader effects and fills survive `.fig`, REST and `.designos` round trips. Studio does not draw them: the inspector lists each one as "Shader (not rendered)" with a visibility toggle and a remove button (#39, PR #1602). A `.designos` file that holds a shader cannot be opened by an older Studio.
- **Project docs.**
  - A new `designos-docs` crate reads and writes a project's Markdown docs and its `docs/docs-manifest.json`. Markdown Studio did not edit is written back byte for byte, a write refuses when the file changed on disk, and edits made outside Studio are reported within a second (#1523, PR #1571).
  - A Project tab with a document tree, a reading page and an "On this page" outline (#1524, PR #1617).
  - Read-only MCP tools let an agent list and read project documents; write tools and live co-editing come later (#1528, PR #1617).
- **FigJam connectors.**
  - Connector routes are cached and hit tests skip far connectors. On a board with 1,500 connectors, pan and zoom went from about 630 ms to 1.3 ms a frame, and dragging a node with 150 attached connectors from about 205 ms to 5 ms (release build, Apple M4 Max) (#1223, PR #1606).
  - In a design file, `.fig` and REST export write each connector as a vector (a group with a label chip when it has a label) with its arrowheads and a marker, and open brings it back as a live connector linked to both ends. Vectors without the marker stay vectors. Board files keep their connector records and the connector fields Studio does not model (#1599, PR #1614).

### Fixed
- **Gradient picker and inspector.**
  - With a gradient picker open, Delete and Backspace remove the selected stop instead of the layer, and a gradient keeps its last two stops; carried from PR #1514 by @NamAIEngineer (#1434, PR #1612).
  - With a picker open, Cmd+Z undoes the right step, and after an undo or redo the open fill or stroke picker takes the document's paint, so the next edit cannot bring an undone gradient back; carried from PR #1521 by @NamAIEngineer (#1434, PR #1612).
  - Mixed values are read from every selected layer, not the first 128; carried from PR #1558 by @NamAIEngineer (#1539, PR #1612).
- **Styles and variables.**
  - K-scale on a text layer detaches its linked text style; carried from PR #1507 by @NamAIEngineer (#1432, PR #1616).
  - Applying a stroke or effect style, pasting properties, and removing or reordering fills, strokes and effects no longer leave stale variable bindings; carried from PRs #1510, #1511, #1509 and #1513 by @NamAIEngineer (#1434, PR #1616).
  - Applying or publishing a text style without paragraph spacing or indent keeps the layer's own values instead of resetting them to 0; carried from PR #1393 by @NamAIEngineer (#1279, PR #1616).
  - A font weight set through a variable binding, the inspector, a spec, MCP or an importer moves a variable font's `wght` axis too, so the text draws at the new weight; carried from PR #1414 by @NamAIEngineer (#891, PR #1616).
- **Text.**
  - Typing after a bold, italic, underline or strikethrough toggle at a collapsed caret detaches the linked text style; carried from PR #1497 by @NamAIEngineer (#1428, PR #1616).
  - Consecutive different text style shortcuts in one text session undo as separate steps; carried from PR #1496 by @NamAIEngineer (#1430, PR #1616).
  - The text size shortcuts change the size once per press during a text session, not twice; carried from PR #1493 by @NamAIEngineer (#1429, PR #1616).
  - Text fields in the app start their text at the requested inset, not at the field edge; carried from PR #1423 by @NamAIEngineer (#1389, PR #1616).
  - Text strokes follow stroke alignment, Inside and Outside, on the CPU and GPU canvas, per style run. Outline stroke and Flatten match the live text; carried from PR #1607 by @NamAIEngineer (#1504, PR #1613).
  - Undo during an IME composition no longer brings the composition back as typed text; carried from PR #1133 by @NamAIEngineer (#1038, PR #1620).
- **Layout and layers.**
  - A group's bounds ignore hidden children when it is refit after an edit, so an edit inside a group keeps the box Resize to fit gave it; carried from PR #1596 by @NamAIEngineer (#1503, PR #1611).
  - Dropping several layers at once keeps their order; carried from PR #1079 by @NamAIEngineer (#938, PR #1620).
- **Effects.** Background blur on a layer with no visible fill or stroke no longer blurs what is behind it, as in Figma; carried from PR #1371 by @NamAIEngineer (#1328, PR #1624).
- **`.fig` and REST import and export.**
  - Saving to `.fig` keeps an instance's text style relink or detach; carried from PR #1383 by @NamAIEngineer (#1347, PR #1622).
  - DTCG import resolves aliases for duplicate manifest groups in token-file order and warns about the duplicate; carried from PR #1373 by @NamAIEngineer (#1326, PR #1622).
  - After an instance swap, nested instance layers take the new component's name unless they have their own name override; carried from PR #1418 by @NamAIEngineer (#1330, PR #1622).
  - Opening a file whose layers or styles use variables the file does not hold keeps those links and says in the open toast how many variables are missing; carried from PR #1512 by @NamAIEngineer (#1434, PR #1622).
  - SVG import names a `<text>` layer after its `id`, like other elements; carried from PR #1077 by @NamAIEngineer (#773, PR #1620).
- **Export.**
  - A Compose grid that hugs its width keeps that width; carried from PR #1584 by @NamAIEngineer (#1565, PR #1619).
  - CSS and Tailwind write the resolved stroke width when it is bound to a variable, and Tailwind keeps a bound zero-width stroke; carried from PR #1570 by @NamAIEngineer (#1552, PR #1619).
  - An export error toast names the path that was tried; carried from PR #1156 by @NamAIEngineer (#1152, PR #1620).
  - Export Frame as `.spec.json` refuses a link at the destination; carried from PR #1149 by @NamAIEngineer (#1073, PR #1620).
- **App.**
  - After a failed library save, Quit, closing, switching tabs and replacing ask first, "Don't Save" leaves the file on disk unchanged, and a reload from disk no longer overwrites unsaved library edits; carried from PR #1198 by @NamAIEngineer (#979, PR #1620).
  - `--select id:` after `--import-capture` no longer calls a captured layer an expanded instance, and the message for ids that exist only in the session is clearer; carried from PR #1364 by @NamAIEngineer (#1340, PR #1620).

### Known issues
- Colour pickers for text, shadow, page, grid and noise can still bring back an undone colour on the next edit (#1618).
- Outline stroke of an Outside text stroke paints a darker band where a chain of more than 128 touching glyphs is split (#1621).
- Image adjustments are not applied in SVG and PDF exports.
- An older Studio does not list a project's deleted doc pages in the trash, and Empty trash erases them.
- Not yet checked by opening a `.fig` with exported connectors in Figma itself (#1599).
- The two issues listed for 0.7.8, text strokes ignoring alignment (#1504) and group bounds counting hidden children (#1503), are fixed in this release (see Fixed). The known issues listed for 0.7.5 still apply. The Windows test suite stays advisory; releases are verified on macOS (#1158).

## [0.7.8] - 2026-11-04

A feature release from `main`: arrowheads and the Arrow tool, image crop and rotation, more glass and noise settings, corner and aspect controls in the inspector, and `.fig` round-trip fixes.

### Added
- **Strokes.**
  - Arrowhead caps on lines and vectors, drawn live and kept through `.fig` and REST (#859, PR #1591).
  - Start point and End point rows in the stroke panel, and the Arrow tool (Shift+L) (#859, PR #1592).
  - A dash cap for dashed strokes (#1478, PR #1543).
- **Images.** Crop, rotation and tile scale on image fills, kept through `.fig` and REST (#163, PR #1585).
- **Effects.** Glass splay and bevel, and noise and texture X/Y size, kept through `.fig` and REST (#38, PR #1593).
- **Inspector.**
  - Individual corner radius fields and a corner smoothing slider (#1466, PR #1533).
  - A lock aspect ratio toggle next to W and H (#1468, PR #1540).
  - Truncate text and max lines in Type settings (#1467, PR #1541).
  - Properties exposed from nested instances (#1471, PR #1559).
- **Edit menu.** Select same text properties and font, Paste and match style, and Swap fill and stroke, as in Figma (#1477, PR #1556).
- **Colour picker.** Arrow keys step the OKLCH and OKLAB values (#1459, PR #1554).

### Fixed
- **`.fig` round trip.**
  - Instance stroke and corner overrides keep their baked geometry (#1508, PR #1548).
  - An instance keeps its own aspect lock (#1542, PR #1547).
  - A variable's stroke colour scope survives a save and reopen (#1550, PR #1567).
- **Components.** Nested instance copies follow later changes to the main component's property values; carried from PR #1586 by @NamAIEngineer (#1564, PR #1589).
- **Vector.**
  - Resizing an imported vector keeps its stroke weight (#945, PR #1566), also when the stroke mixes alignments or joins (#1575, PR #1588).
  - Shift keeps the aspect on edge scale handles, with resize cursors (#477, PR #1562).
- **Variables.** STROKE-scoped colours are offered everywhere a stroke colour is bound (#1499, PR #1549).
- **Layout.** Column-locked grid children are placed the way Figma does (#1441, PR #1560).
- **Effects.** A fill-less frame's shadow knockout follows what its children paint (#1277, PR #1569). A shadow's blend mode survives `.pen` and SVG round trips (#1278, PR #1577).
- **Styles.** Deleting a style keeps the look it had when deleted (#1136, PR #1576).
- **Export.** CSS and Tailwind grid rows and gaps match the layout solver, variables included (#1431, PR #1553). SwiftUI Fill plus Fixed frames are valid, and absolute children stay out of stack flow (#1454, #1458, PR #1563).
- **App.** Copy and paste properties shortcuts work in a real window (#1557, PR #1568).

### Known issues
- Text strokes ignore stroke alignment (#1504). Group bounds count hidden children (#1503).
- The known issues listed for 0.7.5 still apply. The Windows test suite stays advisory; releases are verified on macOS (#1158).

## [0.7.7] - 2026-11-02

A fix and feature release from `main`: zoomed-out frames no longer go blank, `.fig` instance overrides survive a save and reopen, and Figma basics from the styles, layout, components and vector work land.

### Added
- **Colour picker.** OKLCH and OKLAB colour modes (#137, PR #1451).
- **Object menu.** Resize to fit shrinks or grows a frame to its visible content; Paste to replace keeps its own shortcut (#1470, PR #1488).
- **Inspector.** A variable bind icon is always visible on bindable fields, and Typography fields get bind entry points (#876, #133, PR #1452).
- **Canvas selection.** A click selects from the outside in like Figma, and Shift-click or Shift-drag adds the matching layers (#1482, PR #1489).
- **Vector.** Flatten and Outline stroke convert text to vector paths (#1469, PR #1491).
- **Masks.** Alpha, Vector and Luminance mask types (#1465, PR #1492).

### Fixed
- **GPU canvas.** A page viewed zoomed far out draws full detail once the view is still, instead of frames with most of their text and small content missing (PR #1485).
- **`.fig` round trip.**
  - An instance's fill and stroke overrides keep their own colour style instead of the main component's, so gradient swatches in the Untitled UI kit are no longer drawn purple (#1464, PR #1484).
  - Saving to `.fig` keeps an instance's own size, opacity and other root overrides (#834, PR #1505).
- **Export.** Native code export (SwiftUI, UIKit, Compose, Android XML) places grid children on the layout solver's tracks (#484, PR #1450).
- **Layout.** Wrapped Fill-width text re-measures its height after a width change (#856, PR #1360).
- **App.** A narrow window no longer panics (PR #1483). The Auto-save OFF switch shows its track edge (PR #1463).
- **Agents.** A second send while a prompt waits is queued behind it instead of replacing it (#1437, PR #1461).
- **`designos-mcp`.** A `set_slot` op in `designos_apply_batch` fills the instance's slot instead of answering ok and changing nothing; carried from PR #1352 by @NamAIEngineer (#1339, #1498, PR #1515).

### Known issues
- The two GPU canvas and instance-fill issues listed for 0.7.6 are fixed in this release (see Fixed).
- Text strokes ignore stroke alignment (#1504). Group bounds count hidden children (#1503).
- The known issues listed for 0.7.5 still apply. The Windows test suite stays advisory; releases are verified on macOS (#1158).

## [0.7.6] - 2026-11-01

A fix release from `main`: Untitled UI-style kits import with the right text sizes, instance swaps keep your overrides, and the inspector and the agent tools gain the work merged since 0.7.5.

### Added
- **Inspector.**
  - Bind variables on a layer's effect rows and on gradient stops (#132, PR #1425).
  - Instance properties are listed in the order the designer set in Figma, with the source component, its description, and an ellipsis for long values (PR #1403).
- **Agents.**
  - The Agents panel is simpler and uses the dark kit (#1144, PR #1438).
  - Approvals: an "Ask for risky only" mode, and a grant that lasts for the session for edits you have already allowed (PR #1312).
- **`designos-mcp`.**
  - Each tool publishes its risk tier and accurate tool annotations (PR #1310).
  - `designos_close_document` closes a document with an explicit policy for unsaved changes and an optional save path, so an agent is no longer stuck on a "Save changes?" dialog (#1282, PR #1353).
  - A `LOOP_SUSPECTED` hint when an agent repeats the same call, alternates between two, or keeps calling a failing tool (#1290, PR #1354).
  - Errors carry an `error_kind` (retryable, fatal or needs a person) (#1291, PR #1359).

### Fixed
- **`.fig` import.**
  - An instance text override that names a text style now links that style, so text in kits such as Untitled UI no longer comes out one size too big and clipped ("Overviev", "Faceboo") (#1327, PR #1348).
  - Design-token aliases resolve by their manifest group (PR #1323).
- **Components.** Swapping an instance keeps the overrides set on its layers, the way Figma does (#1427, PR #1449).
- **Typography.** Editing a Typography field that a text style owns detaches the style, so the panel never shows a style the text no longer matches (#1026, PR #1424).
- **Export.** CSS and Tailwind grids use the layout solver's track sizes and row heights (#484, PR #1426).
- **Controls.** The radio card's focus ring follows the card's corners (#1436, PR #1444), and a closed dropdown paints its keyboard focus ring (#1273, PR #1351).
- **CLI.** `--select` refuses a value that matches both a layer name and an id, and refuses ids that exist only in a session (#1319, PR #1334). Hug text sizes from a spec are whole pixels (#1321, PR #1332).
- **Windows.** The app binaries get an 8 MiB main-thread stack (PR #1413).

### Removed
- The plugin manifest import that nothing used, and the plugin claims that went with it (PR #1410).

### Known issues
- On the GPU canvas, a page viewed zoomed far out can show frames with most of their text and small content missing, even after the view is still. A fix that draws full detail once the view settles is in progress.
- In some `.fig` kits, an instance whose fill override names its own colour style draws the main component's colour instead, for example gradient swatches drawn purple in the Untitled UI kit (#1464).
- The known issues listed for 0.7.5 still apply. The Windows test suite stays advisory; releases are verified on macOS (#1158).

## [0.7.5] - 2026-10-30

**UI revamp, slice 1** ([milestone v0.7.5](https://github.com/jangtrinh/design-os-studio/milestone/57)): Studio gets a new dark look, painted from one shared set of controls. Colours, type, spacing, corner radii and control sizes follow one scale across Home, the window chrome, the left panel, popovers, dialogs, the command bar, `Assets`, `Variables`, `Libraries` and `Check designs`. Regions, panel widths and the order of controls stay where they were, and rows, buttons and text grow only where the new scale says so.

### Changed
- **Colours.**
  - Studio's chrome moves to a neutral dark palette: a near-black background, dark grey surfaces and dividers, Studio orange for brand actions, and an orange keyboard focus ring (#1142, PR #1343).
  - The canvas's inline auto-layout editor no longer draws a field box inside its frame (#1142, PR #1343).
- **Type, spacing and control sizes.**
  - Body text is 12 px with three text weights (regular, medium and semibold), and captions and labels take new sizes and line heights. Corner radii are 6, 8, 12 and 16 px with nested corners concentric, and controls and badges come in fixed size steps. Rows, fields and text grow where the scale says, and regions and panel widths stay (#1143, PR #1349).
  - Dev Mode shows a top-level frame's status once, as the badge beside its name, and the export scale chevron has a 24 px pointer target (#1143, PR #1349).
- **Controls.**
  - Buttons, fields, checkboxes, toggles, radio cards, tabs, rows, menu items, badges, tags, tooltips and progress controls take their colours, border, radius and focus ring from one shared look (#1139, PR #1369).
  - Menu items sit inset 8 px inside a panel with a 16 px corner radius, and the submenu chevron uses the item's icon colour (#1139, PR #1369).
  - A keyboard-focused control draws a 2 px focus ring, 2 px outside its edge, in every state and following the control's shape (#1139, PR #1369).
  - Popover checkboxes stay at the row's left edge. Colour swatches show the focus ring of the control that holds focus and never take the row's click (#1139, PR #1369).
  - A switch shorter than 24 px takes clicks in a 24 px band around its track (#1155, PR #1380).
- **Window chrome.**
  - The title bar, file tabs, left rail, floating toolbar, rulers, shell menus and toasts use the new dark look and the shared controls, with their hover, pressed, selected and focus states. Regions, widths and control order stay, and heights change only where a control or text size grew (#1119, PR #1376).
  - The tab bar drops the rose close-button hover, and small close buttons and chevrons keep a 24 px pointer target (#1119, PR #1376).
  - A 4 px press band beside each ruler makes the strip easier to grab. A click or release in the band without a drag drops no guide, and a guide you already grabbed and nudge inside the band is kept (#1119, PR #1376).
- **Popovers, dialogs and the command bar.**
  - Popovers, menus, dialogs, toasts, the command bar, Find, comments and version history use the new dark look and the shared controls (#1146, PR #1375).
  - The command bar has 36 px rows with separate hover and selected looks. Enter runs the row picked with the keyboard, and hovering does not move that pick (#1146, PR #1375).
  - The import path and save version dialogs use 14 px body text and 40 px footer buttons (#1146, PR #1375).
  - The context menu and the layer picker use 32 px rows with an 8 px inset, and Find, comments and version history use the extra-small buttons (#1146, PR #1375).
  - Brand buttons use a dark label on orange, and destructive buttons use a white label (#1146, PR #1375).
- **Pages and Layers.**
  - The left panel (file name, `Pages`, the Dev section and `Layers`) uses the new dark look and keeps its 240 px width, its section order and every control's place (#1120, PR #1377).
  - Page rows grow to 32 px, row fills are neutral greys, and hover, selected and focus states come from the shared rows. Icon buttons in the panel grow to 32 px (#1120, PR #1377).
  - Studio orange marks live state (drop targets and canvas hover), and component layers stay cyan (#1120, PR #1377).
  - After a click on a page row, the keyboard stays on the canvas (#1120, PR #1377).
  - The Dev list and the layer filter use the same rows and controls (#1120, PR #1377).
- **Home.**
  - Home takes its colours from the same dark palette as the editor. `+ Design file` is Studio orange with a dark label, replacing the blue (#1138, PRs #1350 and #1378).
  - The page and sidebar use dark neutral surfaces with hairline borders. Header and filter buttons are 36 px, and inputs, menus and badges use the shared controls (#1138, PR #1378).
  - The layout stays: the 264 px sidebar, the header with title and actions, the filter and sort bar, the file grid or list, and Trash at the bottom of the sidebar (#1138, PR #1378).
- **Assets, Variables, Libraries and Check designs.**
  - `Assets`, `Variables`, `Libraries`, `Plugins` and `Check designs` use the new dark look and the shared controls. Regions, widths, columns and order stay (#1155, PR #1380).
  - In `Assets`, the search field rests on the neutral border with no blue outline and shows the focus ring only while it has focus. The filter and view controls are 32 px icon buttons, and group rows have no chevron and take a hover fill (#1155, PR #1380).
  - In `Variables`, value cells draw nothing at rest, take a hover fill, and show a focused field with the focus ring while you edit. Booleans use the switch, alias pills are 24 px, and `Create variable` is centred in its bar. The edit popover closes with a cross and uses checkboxes (#1155, PR #1380).
  - In `Libraries`, the dialog uses the new dialog look with a 36 px field and buttons and 8 px gaps. Change labels such as `New` and `Changed` are status chips, names stop short of the update button, the update button fits its label, the switch is the shared switch, and the description hint sits inside its field (#1155, PR #1380).
  - In `Check designs`, rule rows use checkboxes, and the contrast choice is a segmented control (#1155, PR #1380).

### Fixed
- **Popovers, dialogs and the command bar.**
  - In a small window the command bar stops 8 px above the bottom toolbar and scrolls its rows instead of covering the toolbar (#1146, PR #1375).
  - The comment menu button no longer covers the first line of a message (#1146, PR #1375).
  - Placeholder text in text fields takes the placeholder grey (#1146, PR #1375).
- **Window chrome.**
  - The import toast shows `Details` once, as a button with a semibold label, and the duplicate word in the message is gone (#1119, PR #1376).
  - A click just past the left panel's resize band reaches the canvas (#1119, PR #1376).
- **Controls.**
  - Disabled controls inside a disabled panel dim once, not twice (#1139, PR #1369).

### Known issues
- Scrolling the terminal with the mouse wheel after a large agent edit is not fixed, and the terminal parts of #1173 were not smoke-tested. #1173 stays open on 0.7.1. The `changed layers` card scrolling from PR #1276 is in 0.7.0.
- Shadow cut-out of a frame with no fill cuts the wrong region when a child is a mask, a group, or paints nothing (#1277).
- A drop or inner shadow's blend mode is lost when saving or exporting to `.pen` or SVG (#1278).
- Editing a text style in a file imported before 0.7.0 resets its linked layers' paragraph indent and list spacing to 0 (#1279).
- Esc on the Other CLI menu or form also reaches Codex and puts it in rewind mode (#1097).
- A document with no path gives `designos-mcp` the whole disk as its workspace (#1074). In Ask mode each `designos-mcp` call asks first.
- A prompt that contains `No, suggest changes` can hold the automatic Enter, and refuse sends, in a Gemini Ask tab until the line scrolls off (#1066).
- Antigravity, Gemini CLI and Grok start MCP servers from copies of the session config that an agent can write, so a server command added there could run outside any sandbox (#1048). It needs an agent that can already write its session folder without asking, such as Codex under your own `workspace-write` setting. Studio checks and rewrites these copies when a tab starts. It does not stop an agent that writes later, and Grok may read an edited MCP config while a session runs [unverified: from Grok's source, not run]. Studio moves aside the sources that Grok's and Antigravity's docs and source name, and a later version of either CLI may read more. Workaround: in a window where an agent can write without asking, do not run a Grok or Antigravity tab in Ask mode. Planned for a later release: run Grok and Antigravity from a folder only that tab can write.
- Gemini CLI in Ask mode reads `tools.allowed` and other settings from `.gemini/settings.json` in the session folder, which an agent can write (#1047). Some keys there skip Gemini's confirmation prompt, and `hooks` and `tools.discoveryCommand` run a command when the next Gemini tab starts. Gemini 0.35 has no switch that ignores workspace settings for one run. Studio rewrites the file when a tab starts, and an Ask-mode Gemini tab refuses to start when the rewrite fails, so a read-only `.gemini` no longer keeps an agent's file. Planned fix: #1059.
- Ask covers agent tabs started from Studio. The `designos` CLI, which you run yourself, and a `designos-mcp` started by another host are not gated by Ask: that host approves its own MCP calls (#1101). A real MCP-level approval gate is planned for a later release.
- Codex, Grok, Antigravity and Gemini CLI ask for approval in their own terminal menu, and the panel shows `Open tab`; Approve and Deny in the panel exist for Claude only (#930). Clicking an option in Codex's own approval menu does nothing, so use the arrow keys and Enter (#1096).
- Gemini CLI and Grok in Ask mode: the status line may not show a waiting state for every approval prompt. For Gemini, the state comes from the confirmation dialog on the tab's screen or from Gemini's `ToolPermission` notification. The `ask_user` and `exit_plan_mode` dialogs draw no `No, suggest changes` option, and a panel narrower than 29 columns hides a cut label, so a wait there starts only from a hook record. The screen check is on for Gemini 0.35.x and 0.65.x, read from source only. For another Gemini version, a No answer leaves `Needs approval` standing until you type a prompt. Where Studio writes no Gemini hooks file (Windows, or Gemini already has system defaults), a wait starts only from the screen. For Grok, the state comes from its `permission_prompt` notification, and Studio never checked that payload against a real Grok [unverified]. No real Gemini or Grok approval prompt was run.
- Export Frame as `.spec.json` writes into `designos_specs` in the temporary folder when Studio finds no source tree, by path, so an agent that can write the temporary folder could plant a link at the file name. It needs you to run the export (#1073).
- The Windows test suite is advisory for 0.7.5. Releases are verified on macOS, and known Windows test failures are tracked in #1158.
- Windows (releases are verified on macOS only):
  - Stop on Windows is not checked by any test. The Stop tests fail there because their Python stub imports `tty`, which Python does not provide on Windows (#1066).
  - Palette auto-fix undo is unverified on Windows: eight tests show different auto-layout sizes after one undo, and the cause is not known (#1066).
  - The link checks for session files are not race-free on Windows (#1061, #1068), an Ask-mode Grok or Antigravity tab refuses an entry that is not Studio's instead of moving it (#1069), and no hooks file is written for Gemini, so its `Needs approval` comes from the screen only (#1058, #1066).
  - Windows was not built or run by hand for these changes.
- Not checked against a live model turn: the Ask-mode Claude launch (skill from a private plugin, Studio's hooks and MCP config from private copies), the Gemini approval state, and the Ask-mode Grok and Antigravity checks were tested with stubs and source reading, not with a real Claude, Gemini, Grok or Antigravity turn.
- Of the 38 GUI fixes from 0.6.1 listed in #1102, 16 were re-checked by tests. The rest are not re-tested yet, and U-17 (a new design file is named "Untitled") is still open (#1102).

## [0.7.0] - 2026-10-28

**Effects, styles and export** ([milestone v0.7.0](https://github.com/jangtrinh/design-os-studio/milestone/69)): Studio adds blend modes on drop and inner shadows, matches Figma's shadow cut-outs on fill-less frames and PNG/SVG export bounds, caps the `Effects` `+` menu per type without trimming imported effects, adds style deletion, `/` folder grouping and rename, style descriptions with hover tooltips, and local text alignment with paragraph indent, list spacing and leading trim on text styles, refines the icon-only left rail, resizable `Pages` / `Layers` split and scrollable `changed layers` card, and shows where exported files went without overwriting existing files.

### Added
- **Styles.**
  - Delete a style from the style menu (`Delete style`, after `Detach style`) while linked layers detach and keep their resolved fill, stroke, text, effect or layout guide values and non-overridden variable bindings in one undo step, with toast `Style deleted · Cmd+Z to undo` (`Ctrl+Z` on Windows and Linux). Read-only library styles disable `Rename style`, `Delete style` and `Edit style` with the tooltip `Edit in the library file` (#127, PR #1112).
  - Styles keep Figma's order across `.designos` and `.fig` round trips and group by `/` prefix into folders in style pickers and the Properties panel's `Styles` section. `Rename folder` in the style picker menu (or double-clicking a folder header in `Styles`) renames every style in that folder in one undo step (#129, PR #1153).
  - Edit a style's `Description` in the `Edit style` popover in one undo step (read-only on library styles with `Edit in the library file`), and style pickers and the Properties panel's `Styles` lists show non-empty style descriptions as hover tooltips (PR #1170).
  - The fill and stroke colour picker's `Libraries` tab shows local colour style descriptions on hover (#1178, PR #1203).
- **Effects.**
  - Drop shadows and inner shadows take a blend mode (`Blend` in the shadow popover: `Normal`, `Darken`, `Multiply`, `Linear burn`, `Color burn`, `Lighten`, `Screen`, `Linear dodge`, `Color dodge`, `Overlay`, `Soft light`, `Hard light`, `Difference`, `Exclusion`, `Hue`, `Saturation`, `Color` or `Luminosity`, showing `Mixed` when selected shadows differ), rendered on the canvas and in exports and preserved through `.fig` round trips (#161, PR #1271).
  - The `Effects` section's `+` menu caps each effect type at Figma's limits (8 drop shadows, 8 inner shadows, 1 layer blur, 1 background blur), disabling a type at its cap with a tooltip such as `Up to 8 drop shadows`; imported effects are never trimmed (#162, PR #1264).
- **Canvas and panels.**
  - A draggable horizontal separator between `Pages` and `Layers` in the left panel resizes the `Pages` list, remembers its height across launches, resets to the 5-row default on double-click or `Enter`, and supports keyboard resizing with the arrow keys, `Home` and `End` (#1108, PR #1137).

### Changed
- **Canvas and panels.**
  - The left rail is icon-only (`48 px` wide) without text captions under its buttons, and shows each button's name (`Main menu`, `File`, `Agents`, `Assets`, `Tools`, `Variables`, `Plugins`, `Updates & Shortcuts`) in a tooltip on pointer hover and keyboard focus (#1106, PR #1131).

### Fixed
- **Agents panel.**
  - Expanding the `changed layers` review card with `Show all` caps the card inside the Agents panel and scrolls the layer list with the mouse wheel and a scrollbar while keeping `Show less`, `Revert` and `Keep` reachable at the bottom (PR #1276; the terminal part of #1173 is not fixed, see Known issues).
- **Canvas and effects.**
  - On a frame with no fill or stroke and `Show behind` off, the drop shadow's cut-out follows the union of its visible children's shapes (clipped when the frame clips content) instead of erasing the whole frame rectangle (#520, PR #1263).
  - When a `Save changes to {name}?` dialog or another interruption commits an in-progress move before pointer release, Studio uses the last pointer position so a layer dragged out of a clipping frame reparents into the frame under the pointer (PR #1204).
- **Styles.**
  - Text styles no longer carry horizontal or vertical text alignment, so applying or editing a text style keeps each text layer's own alignment, and text styles now keep paragraph indent, list spacing and leading trim (#1031, PR #1270).
- **Import and export.**
  - PNG and SVG exports expand render bounds to match Figma for drop shadows, strokes and groups (#528, PR #1261).
  - After an export (`Exported {path}` or `Exported {n} files to {folder}`), a toast shows where the file or files went with a `Show in folder` action (or names the path and reason when a write fails), replaces `/` and `:` in exported layer names with `-`, and never overwrites an existing file (`Card-1.png`, `Card-2.png`, …) (#1196, PR #1281).
  - Fallback exports from the Export panel and menu when no save dialog is available always save to `Downloads` (creating it if missing), show a `Show in folder` toast, queue plain toasts behind an active reveal toast, and include the target path and OS reason when a write fails (#1095, PR #1148).
  - Atomic saves on macOS keep the file's ACLs, extended attributes and permissions, allow saving a read-only file when an ACL grants write access, refuse a FIFO planted at the target path, and remove only Studio's own temporary save files so your own `.tmp` files are left alone (#483, PR #1129).
- **CLI and MCP.**
  - A `--screenshot` run with unsaved edits closes after writing the PNG without stopping on `Save changes to {name}?` or writing an autosave or version (#1274, PR #1280).
  - `--apply-spec`, `designos_apply_spec` and `designos_apply_batch` centre hugging button labels inside `NodeSpec` presets and aligned auto-layout frames when the text has default alignment, and `designos_create_node` and `designos_update_node` keep explicit text box sizes across later text-only updates (#1098, PR #1151).

### Known issues
- Not in 0.7.0: Codex approvals answered from the Agents panel (#930) and the Ubuntu test-memory change (#1205) were reverted before release (#1284). Codex still asks for approval in its own terminal menu, and the Ubuntu test job passes without the change.
- Scrolling the terminal with the mouse wheel after a large agent edit is not fixed, and the terminal parts of #1173 were not smoke-tested. #1173 stays open on 0.7.1. The `changed layers` card scrolling from PR #1276 is in 0.7.0.
- Shadow cut-out of a frame with no fill cuts the wrong region when a child is a mask, a group, or paints nothing (#1277).
- A drop or inner shadow's blend mode is lost when saving or exporting to `.pen` or SVG (#1278).
- Editing a text style in a file imported before 0.7.0 resets its linked layers' paragraph indent and list spacing to 0 (#1279).
- Esc on the Other CLI menu or form also reaches Codex and puts it in rewind mode (#1097).
- A document with no path gives `designos-mcp` the whole disk as its workspace (#1074). In Ask mode each `designos-mcp` call asks first.
- A prompt that contains `No, suggest changes` can hold the automatic Enter, and refuse sends, in a Gemini Ask tab until the line scrolls off (#1066).
- Antigravity, Gemini CLI and Grok start MCP servers from copies of the session config that an agent can write, so a server command added there could run outside any sandbox (#1048). It needs an agent that can already write its session folder without asking, such as Codex under your own `workspace-write` setting. Studio checks and rewrites these copies when a tab starts. It does not stop an agent that writes later, and Grok may read an edited MCP config while a session runs [unverified: from Grok's source, not run]. Studio moves aside the sources that Grok's and Antigravity's docs and source name, and a later version of either CLI may read more. Workaround: in a window where an agent can write without asking, do not run a Grok or Antigravity tab in Ask mode. Planned for a later release: run Grok and Antigravity from a folder only that tab can write.
- Gemini CLI in Ask mode reads `tools.allowed` and other settings from `.gemini/settings.json` in the session folder, which an agent can write (#1047). Some keys there skip Gemini's confirmation prompt, and `hooks` and `tools.discoveryCommand` run a command when the next Gemini tab starts. Gemini 0.35 has no switch that ignores workspace settings for one run. Studio rewrites the file when a tab starts, and an Ask-mode Gemini tab refuses to start when the rewrite fails, so a read-only `.gemini` no longer keeps an agent's file. Planned fix: #1059.
- Ask covers agent tabs started from Studio. The `designos` CLI, which you run yourself, and a `designos-mcp` started by another host are not gated by Ask: that host approves its own MCP calls (#1101). A real MCP-level approval gate is planned for a later release.
- Codex, Grok, Antigravity and Gemini CLI ask for approval in their own terminal menu, and the panel shows `Open tab`; Approve and Deny in the panel exist for Claude only (#930). Clicking an option in Codex's own approval menu does nothing, so use the arrow keys and Enter (#1096).
- Gemini CLI and Grok in Ask mode: the status line may not show a waiting state for every approval prompt. For Gemini, the state comes from the confirmation dialog on the tab's screen or from Gemini's `ToolPermission` notification. The `ask_user` and `exit_plan_mode` dialogs draw no `No, suggest changes` option, and a panel narrower than 29 columns hides a cut label, so a wait there starts only from a hook record. The screen check is on for Gemini 0.35.x and 0.65.x, read from source only. For another Gemini version, a No answer leaves `Needs approval` standing until you type a prompt. Where Studio writes no Gemini hooks file (Windows, or Gemini already has system defaults), a wait starts only from the screen. For Grok, the state comes from its `permission_prompt` notification, and Studio never checked that payload against a real Grok [unverified]. No real Gemini or Grok approval prompt was run.
- Export Frame as `.spec.json` writes into `designos_specs` in the temporary folder when Studio finds no source tree, by path, so an agent that can write the temporary folder could plant a link at the file name. It needs you to run the export (#1073).
- The Windows test suite is advisory for 0.7.0. Releases are verified on macOS, and known Windows test failures are tracked in #1158.
- Windows (releases are verified on macOS only):
  - Stop on Windows is not checked by any test. The Stop tests fail there because their Python stub imports `tty`, which Python does not provide on Windows (#1066).
  - Palette auto-fix undo is unverified on Windows: eight tests show different auto-layout sizes after one undo, and the cause is not known (#1066).
  - The link checks for session files are not race-free on Windows (#1061, #1068), an Ask-mode Grok or Antigravity tab refuses an entry that is not Studio's instead of moving it (#1069), and no hooks file is written for Gemini, so its `Needs approval` comes from the screen only (#1058, #1066).
  - Windows was not built or run by hand for these changes.
- Not checked against a live model turn: the Ask-mode Claude launch (skill from a private plugin, Studio's hooks and MCP config from private copies), the Gemini approval state, and the Ask-mode Grok and Antigravity checks were tested with stubs and source reading, not with a real Claude, Gemini, Grok or Antigravity turn.
- Of the 38 GUI fixes from 0.6.1 listed in #1102, 16 were re-checked by tests. The rest are not re-tested yet, and U-17 (a new design file is named "Untitled") is still open (#1102).

## [0.6.5] - 2026-10-27

Patch for **AI design** ([milestone v0.6.5](https://github.com/jangtrinh/design-os-studio/milestone/52)): Studio ships the `DESIGN-OS Studio Starter` file, exports a single variable mode as one self-contained DTCG `.tokens.json` file with alias resolution and hex colors, addresses CLI nodes by name, id or Figma id, measures hug text with the renderer, and speeds up large multi-layer edits.

### Added
- **Starter file.**
  - A new library gets a `Getting started` project with the `DESIGN-OS Studio Starter` file (pages `Starter screens` and `About this file`), and an existing library gets it once on its first launch of 0.6.5; files you already have are not touched. `Open starter sample` in Home (`Templates & Specs` and `Resources & Libraries`) and `Help and account` › `Open starter sample` open a fresh copy in the current project (`DESIGN-OS Studio Starter (1)`, `(2)`, … when the name is taken), and launching with no file argument opens Home. The older built-in sample files, `--seed-library`, `--fixture` (in release builds) and the `examples/` folder are removed (#1212, PR #1252).
- **Import and export.**
  - `designos FILE --export-variables OUT.json --format dtcg --mode MODE` exports one variable mode across the file's collections (or the collections named with `--collection`) as a single self-contained DTCG `.tokens.json` file. When a token has no value in the chosen mode, Studio falls back to its collection's default mode (reported as `values_from_default_mode` in the CLI's JSON summary) and inlines cross-collection alias targets so every `{…}` reference resolves inside the file; an alias whose target has no value in any mode stops the export before writing. `--resolve-aliases` replaces `{…}` alias strings with resolved literal values while keeping `alias_of` under `$extensions` (with default-mode hops counted in `aliases_resolved_via_default_mode` in the CLI's JSON summary), and `--color-format` (`hex` or `object`) writes `#RRGGBB` / `#RRGGBBAA` strings when set to `hex`. Multi-file DTCG folder and `.zip` exports give same-named local and library collections unique group and file names and round-trip them through `tokens.manifest.json` (#1140, PR #1253).

### Changed
- **Canvas.**
  - Large multi-layer edits, such as command-palette auto-fix on a big design-system page, finish faster because Studio records only the layers that changed (#993, PR #1249).

### Fixed
- **Agents panel.**
  - When an agent CLI (such as Antigravity) redraws its screen and clears scrollback on a UI zoom or terminal resize, Studio clears that tab's scrollback buffer instead of appending another copy of the screen, and sizes the scrollbar thumb and wheel range from the real scrollback length (#1171, PR #1236).
  - A Codex tab whose turn ends quietly changes from `Working…` to `Done` after 1.5 s without output and hides the `Stop` button once no turn is running (#1194, PR #1238).
  - When the tab you are viewing is waiting for approval in its terminal, the approval banner shows `Show prompt` instead of `Open tab` and focuses that tab's terminal so `Enter` and `Esc` reach the CLI prompt directly. When another tab is waiting, `Open tab` switches to that waiting tab (or the oldest waiting tab when several tabs wait), and both buttons are keyboard-reachable with `Tab` and `Enter` (#1195, PR #1247).
- **Canvas.**
  - Renaming a component or frame from an agent session through `designos-mcp` repaints the canvas frame label when the live patch lands. Clicking a single-shape component on the canvas selects the component while double-clicking enters the shape, and renaming a component or its child shape keeps their names independent while updating unrenamed instances (#1193, PR #1237).
  - Frame, component and instance name labels on the canvas are measured with the app's own fonts and cut to fit with `…` inside the frame's on-screen width minus a 4.5 px right inset, the way Figma does; when a component or instance glyph plus `…` does not fit, only `…` is drawn (#1197, PR #1239).
  - Hug and fixed-width text layers created or updated from a `NodeSpec` (`--apply-spec`, `designos_apply_spec`, `designos_create_node`, `designos_update_node` and `designos_apply_batch`) are measured with the text renderer instead of a character-count width estimate, honour `font_family`, and return a `TEXT_MEASURED_WITH_FALLBACK_FONT` warning when a requested font falls back (#1154, PR #1254).
- **Import and export.**
  - Home › `Import` › `Enter a path…` trims whitespace and one pair of quotes, expands `~` and `~/…`, decodes percent-encoded `file://` links, refuses a relative path with `Type the full path to the file, starting with / or ~/.`, and logs `designos: warn: import path` on stderr when a typed open fails. Toast timers start on the toast's first painted frame so a slow frame cannot expire a toast before you see it (#1192, PR #1213).
  - `.fig` files with 64-bit values that use the top bit (such as some stroke seeds) import without corrupting the rest of the file, and Studio no longer writes an invalid 10-byte number on export (#1043, PR #1251).
  - Icons and vectors inside resized component instances in an imported `.fig` scale with the instance (stroke weight unchanged), and stroke-only icons keep their strokes (#1242, PR #1256).
- **Styles.**
  - Deleted, soft-deleted and orphan style records from imported `.fig` files are hidden from style pickers, Assets, library publishing, layer style chips, `designos_get_document` and `designos_search_design_system`, while layers keep their appearance and style links when you save back to `.fig`. Listed styles whose name is empty or whitespace-only appear as `Unnamed fill style`, `Unnamed text style`, `Unnamed effect style` or `Unnamed grid style`, with the raw ID in the tooltip (#1165, PR #1255).
- **CLI and MCP.**
  - `designos_open_file` and `designos_import_web_capture` enforce one 512 MiB (`536870912` bytes) file size limit. Files over the limit fail with `FILE_TOO_LARGE` at `/path`, `safe_to_retry: false`, and a `next_step` that points to DESIGN:OS Studio, and `designos-mcp` usage errors list `--version` (`Usage: designos-mcp --workspace DIRECTORY | --version | --list-lint-rules | --help`) (#1121, PR #1257).
  - `designos_save_file`, `designos_render_png` and `designos_export_fig_file` return `INVALID_NAME` when an output path is blank, has a whitespace-only file or folder name or stem, or contains a control or direction-control character. When any path component is over 255 UTF-8 bytes, they return `NAME_TOO_LONG` without suggesting `overwrite: true` (#1003, PR #1258).
  - `designos` and `designos-studio` `--select` accepts `id:<n>`, `fig:<figma id>`, `name:<text>`, `none` or a layer name (trying a bare number as a layer name before a node id, and expanding lazy pages once on a miss), supports `--zoom fit` to frame the selection, reports `--inspect-json` `selection` bounds in page coordinates with `source_id` on screens, rejects `--select ""` and `--select` with `--home` at parse time, and on a `--select` or `--edit-text` miss prints what was searched on stderr, exits 2, and writes no PNG or file change (#1127, PR #1259).

### Known issues
- Esc on the Other CLI menu or form also reaches Codex and puts it in rewind mode (#1097).
- A document with no path gives `designos-mcp` the whole disk as its workspace (#1074). In Ask mode each `designos-mcp` call asks first.
- A prompt that contains `No, suggest changes` can hold the automatic Enter, and refuse sends, in a Gemini Ask tab until the line scrolls off (#1066).
- Antigravity, Gemini CLI and Grok start MCP servers from copies of the session config that an agent can write, so a server command added there could run outside any sandbox (#1048). It needs an agent that can already write its session folder without asking, such as Codex under your own `workspace-write` setting. Studio checks and rewrites these copies when a tab starts. It does not stop an agent that writes later, and Grok may read an edited MCP config while a session runs [unverified: from Grok's source, not run]. Studio moves aside the sources that Grok's and Antigravity's docs and source name, and a later version of either CLI may read more. Workaround: in a window where an agent can write without asking, do not run a Grok or Antigravity tab in Ask mode. Planned for a later release: run Grok and Antigravity from a folder only that tab can write.
- Gemini CLI in Ask mode reads `tools.allowed` and other settings from `.gemini/settings.json` in the session folder, which an agent can write (#1047). Some keys there skip Gemini's confirmation prompt, and `hooks` and `tools.discoveryCommand` run a command when the next Gemini tab starts. Gemini 0.35 has no switch that ignores workspace settings for one run. Studio rewrites the file when a tab starts, and an Ask-mode Gemini tab refuses to start when the rewrite fails, so a read-only `.gemini` no longer keeps an agent's file. Planned fix: #1059.
- Ask covers agent tabs started from Studio. The `designos` CLI, which you run yourself, and a `designos-mcp` started by another host are not gated by Ask: that host approves its own MCP calls (#1101). A real MCP-level approval gate is planned for a later release.
- Codex, Grok, Antigravity and Gemini CLI ask for approval in their own terminal menu, and the panel shows `Open tab`; Approve and Deny in the panel exist for Claude only (#930). Clicking an option in Codex's own approval menu does nothing, so use the arrow keys and Enter (#1096).
- Gemini CLI and Grok in Ask mode: the status line may not show a waiting state for every approval prompt. For Gemini, the state comes from the confirmation dialog on the tab's screen or from Gemini's `ToolPermission` notification. The `ask_user` and `exit_plan_mode` dialogs draw no `No, suggest changes` option, and a panel narrower than 29 columns hides a cut label, so a wait there starts only from a hook record. The screen check is on for Gemini 0.35.x and 0.65.x, read from source only. For another Gemini version, a No answer leaves `Needs approval` standing until you type a prompt. Where Studio writes no Gemini hooks file (Windows, or Gemini already has system defaults), a wait starts only from the screen. For Grok, the state comes from its `permission_prompt` notification, and Studio never checked that payload against a real Grok [unverified]. No real Gemini or Grok approval prompt was run.
- Export Frame as `.spec.json` writes into `designos_specs` in the temporary folder when Studio finds no source tree, by path, so an agent that can write the temporary folder could plant a link at the file name. It needs you to run the export (#1073).
- The Windows test suite is advisory for 0.6.5. Releases are verified on macOS, and known Windows test failures are tracked in #1158.
- Windows (releases are verified on macOS only):
  - Stop on Windows is not checked by any test. The Stop tests fail there because their Python stub imports `tty`, which Python does not provide on Windows (#1066).
  - Palette auto-fix undo is unverified on Windows: eight tests show different auto-layout sizes after one undo, and the cause is not known (#1066).
  - The link checks for session files are not race-free on Windows (#1061, #1068), an Ask-mode Grok or Antigravity tab refuses an entry that is not Studio's instead of moving it (#1069), and no hooks file is written for Gemini, so its `Needs approval` comes from the screen only (#1058, #1066).
  - Windows was not built or run by hand for these changes.
- Not checked against a live model turn: the Ask-mode Claude launch (skill from a private plugin, Studio's hooks and MCP config from private copies), the Gemini approval state, and the Ask-mode Grok and Antigravity checks were tested with stubs and source reading, not with a real Claude, Gemini, Grok or Antigravity turn.
- Of the 38 GUI fixes from 0.6.1 listed in #1102, 16 were re-checked by tests. The rest are not re-tested yet, and U-17 (a new design file is named "Untitled") is still open (#1102).

## [0.6.4] - 2026-10-27

Not published: the tag was pushed before the version bump, so no packages were built, and its fixes ship in 0.6.5.

## [0.6.3] - 2026-10-26

Patch for **AI design** ([milestone v0.6.3](https://github.com/jangtrinh/design-os-studio/milestone/49)): Studio keeps every agent hook output in a private folder no agent grant reaches, refuses broken `--screenshot` inputs and writes readable spec errors, and offers `Enter a path…` from Home › Import on Linux when no file dialog can open.

### Fixed
- **Agents panel.**
  - Hook commands that Studio installs for Claude, Grok, Antigravity and Gemini no longer write or read files in the session folder. Hook outputs (`agent-status.json`, `agent-events.jsonl`, `turn-marks.jsonl`, `claude-transcript.json`) and Claude's prompt-hook reply files (`page-context-reply.json`, `bootstrap-context-reply.json`, `bootstrap-compact-reply.json`) now live in a Studio-private `hook-out` folder inside the session's approval channel folder in Studio's data directory (mode 0700), outside the session folder, `$TMPDIR`, `/tmp`, `/var/tmp` and every folder an agent CLI is granted. `doc-patch.jsonl` stays in the session folder, and the turn-mark hook reads its byte size with `stat -L` instead of `wc -c <`, so a link or FIFO planted in the session folder is neither written through nor waited on. With no private data folder, Studio installs no file-writing or file-reading hook, and a hook that cannot write its folder (for example under Gemini's or Grok's own sandbox) exits 0 with nothing on stderr (#1072, #1050, #1157, PR #1085).
  - When an Ask-mode Grok or Antigravity tab cannot move a non-Studio entry into `.designos-set-aside`, a locked entry says `is locked, so Studio can't move it. Unlock it, then try again.`, a permission or ACL refusal names both the entry and its folder, and a full `.designos-set-aside` says `every name Studio tried was taken. Close this file's other agent tabs, then try again.` (#1072, PR #1085).
- **CLI and MCP.**
  - `designos FILE --screenshot OUT.png` and `designos-studio FILE --screenshot OUT.png` on a missing, empty or corrupt file now exit with status 1, write no PNG, and print `{file} could not be opened: {reason}. Check the file, then try again.` on stderr instead of rendering a blank Untitled canvas and exiting 0. Valid files are loaded once (#1003 RT-04, PR #1117).
  - `--apply-spec`, `designos_apply_spec` and `designos_apply_batch` refuse invalid `NodeSpec` field values with a plain-words message that names the field path and what it must be (for example `children must be an array of NodeSpec objects. Pass each child as a JSON object.`) instead of serde's internal Rust type names. `--apply-spec` also refuses duplicate JSON keys and non-finite numbers (`must be a finite number`) and strips internal error-code prefixes (#1003 RT-07, PR #1117).
  - `README.md`, `crates/designos-mcp/rules/workflow.md` and `skills/design-os-studio/rules.md` note that Ask covers agent tabs started from Studio's Agents panel, while the `designos` CLI you run yourself and a `designos-mcp` started by another host are not gated by Studio; that host approves its own MCP calls (#1101, PR #1117).
- **Linux.**
  - Home › `Import` on Linux checks whether `org.freedesktop.portal.Desktop` provides `org.freedesktop.portal.FileChooser` on the session D-Bus (starting the portal on first use when installed) or whether an executable `zenity` is on `PATH`. When no file dialog can open, Studio logs `designos: warn: file dialog unavailable:` with the cause on stderr, shows `No file dialog. Install zenity or xdg-desktop-portal.`, and on Home › `Import` offers `Enter a path…` to open a `.fig`, `.designos`, `.json` or `.svg` file by path (#1192, PR #1201).

### Known issues
- Esc on the Other CLI menu or form also reaches Codex and puts it in rewind mode (#1097).
- A document with no path gives `designos-mcp` the whole disk as its workspace (#1074). In Ask mode each `designos-mcp` call asks first.
- A prompt that contains `No, suggest changes` can hold the automatic Enter, and refuse sends, in a Gemini Ask tab until the line scrolls off (#1066).
- Antigravity, Gemini CLI and Grok start MCP servers from copies of the session config that an agent can write, so a server command added there could run outside any sandbox (#1048). It needs an agent that can already write its session folder without asking, such as Codex under your own `workspace-write` setting. Studio checks and rewrites these copies when a tab starts. It does not stop an agent that writes later, and Grok may read an edited MCP config while a session runs [unverified: from Grok's source, not run]. Studio moves aside the sources that Grok's and Antigravity's docs and source name, and a later version of either CLI may read more. Workaround: in a window where an agent can write without asking, do not run a Grok or Antigravity tab in Ask mode. Planned for a later release: run Grok and Antigravity from a folder only that tab can write.
- Gemini CLI in Ask mode reads `tools.allowed` and other settings from `.gemini/settings.json` in the session folder, which an agent can write (#1047). Some keys there skip Gemini's confirmation prompt, and `hooks` and `tools.discoveryCommand` run a command when the next Gemini tab starts. Gemini 0.35 has no switch that ignores workspace settings for one run. Studio rewrites the file when a tab starts, and an Ask-mode Gemini tab refuses to start when the rewrite fails, so a read-only `.gemini` no longer keeps an agent's file. Planned fix: #1059.
- Ask covers agent tabs started from Studio. The `designos` CLI, which you run yourself, and a `designos-mcp` started by another host are not gated by Ask: that host approves its own MCP calls (#1101). A real MCP-level approval gate is planned for a later release.
- Codex, Grok, Antigravity and Gemini CLI ask for approval in their own terminal menu, and the panel shows `Open tab`; Approve and Deny in the panel exist for Claude only (#930). Clicking an option in Codex's own approval menu does nothing, so use the arrow keys and Enter (#1096).
- Gemini CLI and Grok in Ask mode: the status line may not show a waiting state for every approval prompt. For Gemini, the state comes from the confirmation dialog on the tab's screen or from Gemini's `ToolPermission` notification. The `ask_user` and `exit_plan_mode` dialogs draw no `No, suggest changes` option, and a panel narrower than 29 columns hides a cut label, so a wait there starts only from a hook record. The screen check is on for Gemini 0.35.x and 0.65.x, read from source only. For another Gemini version, a No answer leaves `Needs approval` standing until you type a prompt. Where Studio writes no Gemini hooks file (Windows, or Gemini already has system defaults), a wait starts only from the screen. For Grok, the state comes from its `permission_prompt` notification, and Studio never checked that payload against a real Grok [unverified]. No real Gemini or Grok approval prompt was run.
- Export Frame as `.spec.json` writes into `designos_specs` in the temporary folder when Studio finds no source tree, by path, so an agent that can write the temporary folder could plant a link at the file name. It needs you to run the export (#1073).
- The Windows test suite is advisory for 0.6.3. Releases are verified on macOS, and known Windows test failures are tracked in #1158.
- Windows (releases are verified on macOS only):
  - Stop on Windows is not checked by any test. The Stop tests fail there because their Python stub imports `tty`, which Python does not provide on Windows (#1066).
  - Palette auto-fix undo is unverified on Windows: eight tests show different auto-layout sizes after one undo, and the cause is not known (#1066).
  - The link checks for session files are not race-free on Windows (#1061, #1068), an Ask-mode Grok or Antigravity tab refuses an entry that is not Studio's instead of moving it (#1069), and no hooks file is written for Gemini, so its `Needs approval` comes from the screen only (#1058, #1066).
  - Windows was not built or run by hand for these changes.
- Not checked against a live model turn: the Ask-mode Claude launch (skill from a private plugin, Studio's hooks and MCP config from private copies), the Gemini approval state, and the Ask-mode Grok and Antigravity checks were tested with stubs and source reading, not with a real Claude, Gemini, Grok or Antigravity turn.
- Of the 38 GUI fixes from 0.6.1 listed in #1102, 16 were re-checked by tests. The rest are not re-tested yet, and U-17 (a new design file is named "Untitled") is still open (#1102).

## [0.6.2] - 2026-10-25

Patch for **AI design** ([milestone v0.6.2 AI design 2](https://github.com/jangtrinh/design-os-studio/milestone/47)): Studio keeps Ask-mode approvals and its own agent configs out of the agent's reach, Gemini CLI tabs show when they wait for approval, and a text style can be made from a text layer.

### Added
- **Styles.**
  - Create a text style from a text layer. The Typography section of the Properties panel has the style button that Effects and Layout guide already have. Its menu lists the file's text styles, checks the one the layer uses, and ends with `Create text style`, `Rename style`, `Detach style` and `Edit style`. The last three work while a style is linked. `Create text style` names the style `Text style {n}`, takes the layer's font family, size, weight, line height, letter spacing and the rest of its text style, links the layer, and is one undo step. It works in a file that has no styles yet (#875, PR #1024).
  - A text style belongs to the whole layer, not to a range. Text you are still typing is committed first, so the typing and the style are two undo steps (#875, PR #1024).
  - `Create text style` is off, with a tooltip that says why, when the layer's runs differ (`Text has mixed styles`), when several selected layers differ from each other (`Selected text has mixed styles`), and on a new text box that is still empty (`Type some text first`). Runs that differ only in color do not count as mixed. With several text layers that share the same type, it links all of them in one step (#875, PR #1024).
  - Picking a style on a new, empty text box keeps the box, so you can pick a style and then type. If other layers are selected with that box, only the box is styled, and a toast says `Only the new text box was styled` (#875, PR #1024).
- **Agents panel.**
  - Gemini CLI tabs in Ask mode show `Needs approval` and a card, `Gemini CLI needs approval in its terminal`, with an `Open tab` button, when Gemini asks to confirm a tool call. The card has no Approve or Deny, because Gemini keeps its own prompt in the terminal. `Open tab` moves focus there. Before, a Gemini tab showed no waiting state. Studio reads Gemini's confirmation dialog from the tab's own screen, because Gemini fires no hook when you answer No (#930, PR #1058).
  - While Gemini's dialog is on screen, the composer sends nothing into that tab and says `Answer the permission prompt in the terminal`. If Gemini opens a dialog after you send a prompt, Studio does not press the Enter that follows the paste: the prompt stays pasted for you to send (#930, PR #1058).
- **CLI and MCP.**
  - `designos-diff --version` and `-V` print `designos-diff {version}` (#1088, PR #1090).
  - When `designos-mcp` cannot read or append to a session file in the background (a setting, the idempotency ledger, a file it reads at start), the next tool reply carries `session_file_warnings`. When a reply's text is not its JSON (the metadata XML, a screenshot's images), the warnings come as one more text block (#1068, PR #1071).

### Changed
- **Agents panel.**
  - In Ask mode, Claude takes its settings from your own user settings only (`--setting-sources user`). The project and local settings and `.mcp.json` in the session folder are no longer loaded. Studio's skill loads from a private plugin, and Studio's hooks and MCP config come from private copies outside the session folder. Skip mode is unchanged. Claude's own "don't ask again" answer is expected to stop persisting in Ask tabs, because Claude saved it to the project settings. [unverified: expected from a probe, not seen in a live session] (#1044, #1046, PR #1045).
  - With no private data folder, an Ask-mode Claude tab starts without Studio's hooks, and Claude asks in its own terminal prompt (#1036, PR #1041).
  - Grok never evaluates a `.envrc`. Every Grok tab, Ask and Skip alike, starts with `GROK_ENVRC_TIMEOUT_SECS` set to `0`. A session folder never holds an `.envrc` you wrote (#1069, PR #1078).
  - An Ask-mode Grok or Antigravity tab starts only when what the CLI would load from the session folder is exactly what Studio wrote. Any other entry (hooks, MCP servers, plugins, agent definitions, `.envrc`) is moved, not deleted, into `.designos-set-aside/{stamp}/` in the session folder, and the status line says what moved. For Grok the move also takes `.claude/settings.json`, `.claude/settings.local.json`, `.claude/plugins` and `.claude/agents` away from a Claude tab of the same folder; they are kept in the same place. Skip mode starts as before. On a system without `openat`, such as Windows, the tab refuses to start instead of moving the entry (#1069, PR #1078).
  - An Ask-mode Gemini, Grok or Antigravity tab refuses to start when Studio cannot write its config in the session folder, or when what Studio wrote does not read back. The status line names the path and says what to do. Skip-mode tabs start as before (#1061, #1069, PR #1067, PR #1078).
  - Session folders for unsaved files, and the fallback folder for a file whose own folder cannot be used, are now in Studio's data folder: `<data dir>/designos/sessions` (macOS: `~/Library/Application Support/designos/sessions`), owner-only. They were in `$TMPDIR/designos-sessions`. Older folders there are left alone (#1057, PR #1060).
  - A tab that cannot start in a refused session folder says why and what to do, for example `Couldn't start {tab}: {path} is a link, not a folder. Delete the link, then try again.` A library file whose own agent folder is refused starts its agents in a temporary folder that is not kept, and says so: `Couldn't use this file's agent folder: {path} is a link, not a folder. This file's agents start in a temporary folder that is not kept. Delete the link and reopen this design file.` When Studio cannot update its session files, the status line starts `Couldn't update the agent files:` (#1057, #1061, PR #1060, PR #1067).
  - The skill Studio writes for agents, `AGENTS.md` and `GEMINI.md` no longer name the open document or page, and the MCP configs Studio writes for the CLIs no longer carry `DESIGNOS_DOC_NAME` or `DESIGNOS_TARGET_PAGE_NAME`. The agent reads the name from `$DESIGNOS_DOC_NAME` in its shell, or with `designos_get_app_state` (#1053, PR #1045).
  - The Stop button and Stop all do not count a Gemini CLI tab, and no Stop button shows on one. A second Ctrl+C exits Gemini, so Studio has no safe interrupt to send it (PR #1058).
- **Dev Mode.**
  - Code keeps one row per source line, so a token such as a CSS variable reference is no longer split and Android XML strings are no longer cut. A line wider than the box scrolls sideways: a trackpad swipe, Shift + wheel, dragging the code, or dragging the thumb under the last row. A mostly vertical swipe still scrolls the panel. Tabs show as four spaces, and Copy still puts the raw code on the clipboard. The MCP prompt box wraps to the panel width and grows to hold the whole prompt, link included (#887, PR #1033).
- **Properties panel.**
  - The Styles section of a selection also lists styles from enabled libraries when the file has no style of its own. It shows only the groups the selection can take: Text styles for a selection with a text layer, Grid styles for a frame. Library effect and grid styles are listed too. Picking a library style copies it into the file and applies it as one undo step (#1039, PR #1092).
  - The page panel's Export section is the working export section: `+` adds an export setting, and Export exports the page. A page exported as PDF writes its visible top-level frames (#1027, PR #1092).
  - The page panel's Styles header no longer has a `+`. It did nothing. A style is made from the layer it describes, through the style button of that layer's Typography, Effects or Layout guide section (#875, PR #1024).
- **Command palette.**
  - Rename layers semantically and Boost resolution take about one second per click on a very large page, where they took about four. The design fix from the palette takes about 1.7 s, where it took 2.3 s. What Studio records about your edits for an agent does not change. [unverified: timings come from PR #1025 and were not measured again] (#993, PR #1025).
- **CLI and MCP.**
  - `designos-mcp` no longer creates `DESIGNOS_SESSION_DIR`. A missing folder is an error: Studio makes the folder before it starts the tab. The session files `designos-mcp` writes are owner-only (mode 0600) (#1068, PR #1071).

### Fixed
- **Components.**
  - Editing a main component no longer renames its instances. A sync never writes the name of an instance, whether it is the root or sits inside another instance. Other layers inside an instance still follow the main's layer names unless you renamed them. Before, instances took the main's or the variant's name, also on pages that were not loaded yet, and the wrong name reached the saved `.designos` file (#983, PR #1091).
- **Export and CLI.**
  - SwiftUI code escapes `\`, `"`, line breaks, tabs and other control characters in text, component prop defaults and values, font names, token color names and image names. `\(` is escaped so Swift does not read it as an interpolation. `${y}` and emoji stay as they are. Before, the `$` of `${y}` came out with a backslash in front of it, which is not valid Swift and does not compile (#1003 RT-03, PR #1090).
  - Hook commands (status, prompt, skills reply and transcript capture) quote the session folder's path. A path with an apostrophe, such as a home folder named after a person, used to split the path or end the command in a syntax error (#1057, PR #1060).
- **CLI and MCP.**
  - MCP file writes (save, PNG render, `.fig` export) refuse a link at the file name, a name with control or bidi-override characters (#1003 RT-05), and a name over 255 bytes (#1003 RT-06). `designos_export_fig_file` takes an `overwrite` option that is off by default, so it refuses an existing file. An existing-file refusal says `already exists. Choose a new path, then try again.` (or `already exists. Pass overwrite: true to replace it.` when `overwrite` is supported). A path outside the workspace is no longer echoed in the error (#1008, #1087, PR #1103).
- **Opening a file from someone else could run commands or attach files through Studio's agent skill.** Studio wrote the document name and the page name into the skill it gives every agent CLI, and into `AGENTS.md` and `GEMINI.md`. A crafted name in a third-party `.fig` or `.designos` file, or set by an agent that switches the shown document, could add frontmatter keys to the skill, run a `!` command when the skill was invoked, or attach any readable file with `@path`. No agent write access was needed. In Skip mode a `!` command ran with no rule at all; in Ask mode it ran when one of your own allow rules covered it; an `@path` attached its file with no prompt in both modes. Studio now writes no document or page name into any file a CLI loads by itself (see Changed) (#1053, PR #1045).
- **Agents panel: what an agent can no longer do.** Each fix below needs an agent that can already write to its session folder or to `$TMPDIR`: for example a Codex tab under your own `workspace-write` setting, a command you approved, or another tab of the same file. They matter most in Ask mode.
  - Answer its own approval requests. In Ask mode, Claude's Approve and Deny waited for an answer file and ran a script that both sat in the session folder. They now live in a Studio-private folder outside the session folder, `/tmp`, `$TMPDIR` and every folder Studio hands the agent, and a request id is 128 random bits. An answer, a request or a script planted anywhere else is never read (#1023, PR #1035).
  - Replace Claude's hooks. In Ask mode the hooks file Claude reads is private too (#1036, PR #1041).
  - Add allow rules, hooks or MCP servers to an Ask-mode Claude through `.claude/settings.json`, `.claude/settings.local.json`, `.mcp.json` or `mcp-config.json` in the session folder (#1044, #1046, PR #1045).
  - Point another tab's session folder at a folder you own. Unsaved files' session folders sat in `$TMPDIR` under predictable names and were made with a call that accepts a link. Studio now makes every session folder itself, with an exclusive `mkdir` at 0700, and uses an existing path only when it is a real folder you own (#1057, PR #1060).
  - Make Studio write through a link the agent plants inside the session folder. Studio's own writes below a session folder (config for Gemini, Grok and Antigravity, usage cache, run record, session registry, lock, skill replies, snapshots) open each folder from its parent without following a link: a link at a folder name is refused, and a link at a file name is replaced, never written through. Reads of files an agent can plant a FIFO at no longer wait on it, so a FIFO at `live-spec.json` no longer freezes Studio (#1061, PR #1067). On Windows these calls check each entry and then act on its path, so they are not race-free there.
  - Make `designos-mcp` write or read through a link, or hang on a FIFO, at its session files (`canvas.designos`, `doc-sync.designos`, `doc-patch.jsonl`, `context.json`, `bootstrap.json`, `mcp-config.json`, `session-options.json`, `idempotency-ledger.jsonl`). It refuses a link, a second name for a file (a hard link), a FIFO, a socket and a folder. A refused edit fails the tool with an error that names the path and says what to do, for example `<path> is a link, not a file. Delete the link, then try again.` (#1068, PR #1071).
  - Add its own hooks, MCP servers, plugins, agent definitions or `.envrc` to an Ask-mode Grok or Antigravity tab (#1069, PR #1078).

### Known issues
- Hook commands that Studio gives Claude, Grok, Antigravity and Gemini write status files in the session folder, and follow a link an agent plants there (#1072).
- A broken file given to `--screenshot` exits 0 and writes a blank image, and a spec error with `children` as an array of arrays reads as a serde type error (#1003).
- Esc on the Other CLI menu or form also reaches Codex and puts it in rewind mode (#1097).
- A document with no path gives `designos-mcp` the whole disk as its workspace (#1074). In Ask mode each `designos-mcp` call asks first.
- A prompt that contains `No, suggest changes` can hold the automatic Enter, and refuse sends, in a Gemini Ask tab until the line scrolls off (#1066).
- Antigravity, Gemini CLI and Grok start MCP servers from copies of the session config that an agent can write, so a server command added there could run outside any sandbox (#1048). It needs an agent that can already write its session folder without asking, such as Codex under your own `workspace-write` setting. Studio checks and rewrites these copies when a tab starts. It does not stop an agent that writes later, and Grok may read an edited MCP config while a session runs [unverified: from Grok's source, not run]. Studio moves aside the sources that Grok's and Antigravity's docs and source name, and a later version of either CLI may read more. Workaround: in a window where an agent can write without asking, do not run a Grok or Antigravity tab in Ask mode. Planned for 0.6.3: run Grok and Antigravity from a folder only that tab can write.
- Gemini CLI in Ask mode reads `tools.allowed` and other settings from `.gemini/settings.json` in the session folder, which an agent can write (#1047). Some keys there skip Gemini's confirmation prompt, and `hooks` and `tools.discoveryCommand` run a command when the next Gemini tab starts. Gemini 0.35 has no switch that ignores workspace settings for one run. Studio rewrites the file when a tab starts, and an Ask-mode Gemini tab refuses to start when the rewrite fails, so a read-only `.gemini` no longer keeps an agent's file. Planned fix: #1059 (0.6.3).
- Ask covers agent tabs started from Studio. The `designos` CLI, which you run yourself, and a `designos-mcp` started by another host are not gated by Ask: that host approves its own MCP calls (#1101). A real MCP-level approval gate is planned for a later release.
- Codex, Grok, Antigravity and Gemini CLI ask for approval in their own terminal menu, and the panel shows `Open tab`; Approve and Deny in the panel exist for Claude only (#930). Clicking an option in Codex's own approval menu does nothing, so use the arrow keys and Enter (#1096).
- Gemini CLI and Grok in Ask mode: the status line may not show a waiting state for every approval prompt. For Gemini, the state comes from the confirmation dialog on the tab's screen or from Gemini's `ToolPermission` notification. The `ask_user` and `exit_plan_mode` dialogs draw no `No, suggest changes` option, and a panel narrower than 29 columns hides a cut label, so a wait there starts only from a hook record. The screen check is on for Gemini 0.35.x and 0.65.x, read from source only. For another Gemini version, a No answer leaves `Needs approval` standing until you type a prompt. Where Studio writes no Gemini hooks file (Windows, or Gemini already has system defaults), a wait starts only from the screen. For Grok, the state comes from its `permission_prompt` notification, and Studio never checked that payload against a real Grok [unverified]. No real Gemini or Grok approval prompt was run.
- Export Frame as `.spec.json` writes into `designos_specs` in the temporary folder when Studio finds no source tree, by path, so an agent that can write the temporary folder could plant a link at the file name. It needs you to run the export (#1073, 0.6.3).
- Windows (releases are verified on macOS only):
  - Stop on Windows is not checked by any test. The Stop tests fail there because their Python stub imports `tty`, which Python does not provide on Windows (#1066).
  - Palette auto-fix undo is unverified on Windows: eight tests show different auto-layout sizes after one undo, and the cause is not known (#1066).
  - The link checks for session files are not race-free on Windows (#1061, #1068), an Ask-mode Grok or Antigravity tab refuses an entry that is not Studio's instead of moving it (#1069), and no hooks file is written for Gemini, so its `Needs approval` comes from the screen only (#1058, #1066).
  - Linux and Windows were not built or run by hand for these changes.
- Not checked against a live model turn: the Ask-mode Claude launch (skill from a private plugin, Studio's hooks and MCP config from private copies), the Gemini approval state, and the Ask-mode Grok and Antigravity checks were tested with stubs and source reading, not with a real Claude, Gemini, Grok or Antigravity turn.
- Of the 38 GUI fixes from 0.6.1 listed in #1102, 16 were re-checked by tests. The rest are not re-tested yet, and U-17 (a new design file is named "Untitled") is still open (#1102).

## [0.6.1] - 2026-10-23

Patch for **AI design** ([milestone v0.6.1 patch](https://github.com/jangtrinh/design-os-studio/milestone/48)): the bottom terminal dock is gone, agents ask before acting by default, and Studio asks before it drops unsaved edits. Most other changes fix bugs found while testing 0.6.0.

### Added
- **Canvas and editing.**
  - Paste text and images from other apps. Cmd+V (Ctrl+V on Windows and Linux) and Edit > Paste turn plain text into a text layer, and a copied image into an image layer. Text wider than 400 px wraps at 400 px. An image is at most 400 px on its longest side, like the Image tool. Each paste is one undo step (#877).
  - Edit > Paste is never disabled. With nothing to paste, a toast says `Nothing to paste` (#877).
  - Drop PNG, JPEG or WebP files on the canvas. Each file becomes an image layer. A drop of several files is one undo step (#878).
  - A pasted or dropped layer lands in the frame under the point where it lands. Inside a frame that clips its content, as every frame imported from Figma does, it stays in the part that shows (#877, #878, #959).
- **Files and export.**
  - Closing a tab, closing the window, quitting, or opening another file over unsaved edits asks `Save changes to {name}?` with Save, Don't save and Cancel. This covers files outside the library. Library files autosave and never ask (#929).
  - On macOS, Cmd+Q and Quit in the app menu ask the same question (#974).
  - When another program changes a file that has unsaved edits in Studio, Studio asks `File changed on disk` with Use file and Keep mine. Enter, Esc and a click outside keep your edits (#933).
  - A save or export to `.fig` or `.pen` ends with `Comments left out: only .designos files keep them`, because those formats have no place for comments. Saves from the `designos` CLI print the same notice on stderr, and `--export-fig` and `designos_export_fig_file` report `omitted_comment_threads` (#851, PR #1006).
  - A `.fig` export toast shows the full path and a Show in folder action (#874).
  - On Linux, when no file dialog can open, Studio says so instead of doing nothing. Open, Save as, Import and image placement show `No file dialog. Install zenity or xdg-desktop-portal.` Export saves to `~/Downloads`, or to the home folder, under a name that is not taken, and shows `No save dialog. Saved to {path}`. The `.deb` recommends `zenity` or `xdg-desktop-portal` (#850).
- **CLI and MCP.**
  - `designos --version` and `-V` print the version. `designos-studio` and `designos-mcp` do the same (#845).
  - `designos_update_node`, `designos_create_node` and the batch `update` and `insert` operations accept `var:<collection>/<name>` for a fill or text color and bind the variable. An unknown name returns `UNKNOWN_VARIABLE` with the closest names. A variable of the wrong kind returns `VARIABLE_TYPE_MISMATCH` (#917).

### Changed
- **Agents panel.**
  - The bottom terminal dock is removed, and so is the `>_` button at the bottom of the left rail. The canvas keeps its full height. ``Cmd+` `` (``Ctrl+` `` on Windows and Linux) opens the Agents panel and closes it again. Opening it resumes the file's last session, or shows the new-session list. It never starts a plain shell (#838, PR #864).
  - Cmd+K agent presets, `Send selection to agent`, the Inspect `Send to agent` button and `--panel terminal` open the Agents panel. Sync Canvas and Apply Spec from Output are gone with the dock. The disk watcher still reloads a file that changes on disk, and a spec an agent writes to `live-spec.json` still applies (#838, PR #864).
  - **The default agent permission is Ask** (PR #861). A new or unset setting asks before acting, and so does a saved value that is unknown or cannot be read. A saved Skip permissions choice is kept, and Preferences > Agent permissions… still offers it. In 0.6.0 the default was Skip permissions (#841).
- **Canvas and editing.**
  - Paste, Ctrl+D and Alt-drag of a main component in the file it lives in make an instance, as in Figma. They used to add a second main. A paste into another file still pastes a real copy (#860).
  - A marquee over part of a main component or a component set selects it as a whole (#854).
  - The Arrow tool is hidden. It drew a plain line with no head. Shift+L selects Line. Arrows already in files open, render and save as before. The tool comes back when strokes have arrowheads (#858, #859).
  - On a machine whose GPU is a software renderer, such as Mesa llvmpipe in a VM, Studio starts on the CPU canvas. Preferences > Use GPU rendering says why. A saved choice, or `--gpu`, still wins (#840).
  - Home no longer shows the `+ Spec bundle` button or the `PRODUCT:OS Spec Bundle v2` card. Both opened a blank canvas (#867).
- **Files and export.**
  - One writer serves `.fig` export from the command palette, Home and the File menu. It never replaces an existing file, including the open one. A name already taken becomes `Name 2.fig`. A failed write shows `Export .fig failed: {reason}`. Files in the library export to the library `exports` folder, and files opened from disk export next to them (#874).
  - Menu labels: `Export .fig…` is now `Export .fig`, and `Save local .fig copy…` is now `Save .fig copy` (#874).
- **CLI and MCP.**
  - `designos FILE --export-fig PATH` refuses a path that does not end in `.fig`, an existing file unless you pass `--overwrite`, and a symbolic link. A refusal exits with status 2 and leaves the document alone. `--overwrite` replaces a regular, writable file only. Re-running an export onto the same path now needs `--overwrite` (#1000, PR #1005).
  - A FILE argument that is not on disk exits with status 2 and prints `No file named {arg}. Run designos --help.` It no longer opens an empty window. `--apply-spec`, `--import-capture` and `--import-variables` still create the file (#845).
  - Uncolored text in a `NodeSpec` takes `#000000` or `#F4F4F5`, whichever contrasts more with the nearest solid fill behind it. It was always `#F4F4F5`. An explicit color, a style or a `var:` binding still wins. Labels on `easeui-button-primary` are now black (#918).
  - `designos_open_file` checks the path before it checks unsaved changes. A path outside the workspace always returns `PATH_OUTSIDE_WORKSPACE` and is not echoed back. The unsaved-changes hint names `designos_save_file` and `discard_changes=true` (#919).
  - The live edit log is version 2. Studio refuses a log line from a newer build and says `Agent edit not applied. Update Studio and designos-mcp to the same version.` Builds up to 0.6.0 show their old message when they read a version 2 log. Use the `designos-mcp` that ships with this build (#824).

### Fixed
- **Undo.**
  - **Undo and redo of your own edits restore the document exactly.** Before, one edit and Cmd+Z could move or collapse frames on pages you were not looking at, and Cmd+Z could not take that back. On the Untitled UI kit, one edit and Cmd+Z left 80 layers changed, by up to 1,332 px (#973, PR #982).
  - After a font face arrives, and after an agent's edits, undo falls back to solving layout again. The re-solve stays for undo of agent edits and reverts, MCP edits, spec applies and web capture imports; for every step recorded before a font face arrived; for a step made while an earlier edit's layout was still queued; and for scrubbing a field on a document over 5,000 layers (#973, PR #982).
  - The command palette's design fix and its five Figma AI rows (Replace content, Boost resolution, Edit image with prompt, Make prototype flows and Rename layers semantically) are each one undo step. Before, they wrote with no undo step, and Cmd+Z undid the edit before them. Each toast names what it did, as in `Layers renamed · Cmd+Z to undo`. A run that changes nothing shows `Nothing changed`, adds no undo step and keeps the palette open (#984, PR #988).
- **Agents panel.**
  - After Revert, Keep or Dismiss on the review card, the typing caret returns to the prompt box. Before, the next letter picked a canvas tool (#922).
  - A dialog, the command palette or a menu gets Esc, Enter and the arrow keys before the agent terminal. The Esc that closes a dialog no longer interrupts a running turn, and Esc in the prompt box only leaves the box (#923). Popup menus and popovers do the same, including the color picker, the font dropdown and the Variables popovers (#960, #981). The Sessions popover does not yet (#1010).
  - Gemini CLI asks before each Studio canvas tool call in Ask mode. Studio no longer writes `trust` for its server in Gemini's settings (#924).
  - Return acts only on a row that has keyboard focus. Tab reaches the rows, and the arrow keys move between them. A bare Return no longer opens Other CLI (#885).
  - The `+` button opens and closes its menu in release builds (#925). The Other CLI caption no longer touches Cancel (#927). When an agent exits, its tab keeps its width and shows a hollow status ring, so the Sessions and `+` buttons stay under the pointer (#829).
  - A session rename takes the keyboard when it opens. The Other CLI form takes no keys while it is clipped out of view (#1001, PR #1007).
  - When an agent opens or creates another document, your unsaved edits stay in their own tab, and the agent's document never takes your file. Save on the agent's document asks where it goes (#966).
  - A rewritten `live-spec.json` with the same content changes nothing. A changed one replaces the layer its earlier content made. Before, each rewrite added another copy (#846).
  - With the Agents panel closed, an agent's edit shows up without a mouse move or key press first (PR #948).
- **Canvas and editing.**
  - On a slow frame, a drawn shape, a move, a resize and a rotate end where the pointer let go. Before, a quick drag made a 100 x 100 shape at the wrong spot, or left the layer short (#842, #862).
  - A quick drag in the Layers panel reorders the layer (#855). A toolbar menu picks the row under the pointer on release (#852).
  - Keys typed in the frame of a click on a field stay in the field, and Tab moves between fields in the order they are drawn (#843, #844).
  - Shift+D enters and leaves Dev Mode, as the toolbar tooltip says (#1012).
  - Ctrl+B draws bold in a downloaded font, and Bold follows a `wght` axis (#873).
  - Resizing a vector or a boolean group scales its shape, not only its box (#853).
  - Text draws with a font face as soon as it arrives. Text you sized while that face was missing is fitted to it. A text box that came from the file is not changed (#886).
  - Applying a fill style drops the fill's variable binding (#955).
  - The toast paints above the full Variables view (#830).
  - A property edit on an instance goes through when its stored variant would make a main hold itself (#989).
  - Studio no longer aborts when a main component holds an instance of itself through a pair or a ring of mains. Paste, Ctrl+D, Alt-drag, drops from Assets, variant switches and MCP batch `move` no longer put a main inside itself (#860).
- **Files and export.**
  - The unsaved dot clears when a save writes the document, returns with the next edit, and clears again when undo walks back to the saved state. A save as `.fig` or `.pen` leaves the dot lit, because those files drop comments (#847).
  - Switching tabs no longer replaces unsaved edits with the file on disk (#933).
  - The Comment tool no longer shows `Comments are not available in the offline editor`. Comments work, and the click opens the composer (#851, PR #1006).
- **CLI and MCP.**
  - A literal color written with `designos_update_node` or `designos_apply_batch` to a layer bound to a variable now drops the binding. Before, the call said `changed: true` and the canvas kept the variable's color. Linking a fill style with `set_style` drops the fill bindings too (#916).
  - `designos-diff compare` on images of different sizes exits with status 1 and prints `Images differ in size: {a} vs {b}. Render both at the same size.` It used to panic (#883).

### Known limits
- On very large pages (100,000 layers or more), Rename layers semantically and Boost resolution from the command palette can take several seconds. Recording them as one undo step costs time on a big page. The next release addresses this first (#993).
- On macOS, Quit from the Dock, logout and shutdown still skip the `Save changes` question (#977).
- A file where several main components hold instances of each other can still run Studio out of memory when you edit it. No Studio gesture or agent tool builds one. The guards cover pairs and rings of mains (#968).
- On Linux, paste from other apps and file drops work through X11 or XWayland only. A pure Wayland session sends no file drop event (#877, #878).
- A `var:` color that names a variable from an enabled library binds only through a direct `designos_apply_spec` call. `designos_create_node`, `designos_update_node` and the batch `update` and `insert` operations return an error that says so (#954).

## [0.6.0] - 2026-10-22

Pack **AI design** ([milestone v0.6 AI design](https://github.com/jangtrinh/design-os-studio/milestone/4)): agents can see a design, read its design system, write in one step and check their own work. The Agents panel gets sessions, review and stop.

### Added
- **Agents panel: sessions.**
  - Opening the panel resumes the file's last session. With none to resume, it shows the CLI picker. Studio no longer starts a shell by itself (#651).
  - A file keeps its sessions after a restart. A menu in the panel lists them, renames them and removes them from the list. Files opened from disk keep sessions too. Renaming or moving such a file starts a fresh history (#653).
  - A session that cannot resume says why and what to do next (PR #806).
  - The new-session picker lists installed CLIs first, with version and login state. The CLI you used last for the file comes first. A CLI that is not installed links to its install page. Rows keep their order while the checks finish, so a click lands on the row you aimed at (#652, #783).
  - Gemini CLI joins Claude Code, Codex, Grok and Antigravity in the new-session picker. OpenCode is not in the picker yet. A sample prompt starts it only when it is the only agent CLI installed, and it runs without Studio tools (#679).
- **Agents panel: talking to an agent.**
  - The composer sends into the tab's live CLI session. Each tab keeps its own draft and its last 100 prompts (#689).
  - A chip shows the current selection. Click it to hand the selection to the agent. It hides when nothing is selected (#654).
  - Type `@` to mention a layer, page, component, variable or style. The agent gets a reference it can resolve (#655).
  - The empty state offers sample prompts that fill the composer, and hints for attaching a selection (#688).
  - Attach bundled skills or your own `SKILL.md` files to a design run. Three skills ship with Studio (#573).
  - On macOS, Cmd+Z undoes and Shift+Cmd+Z redoes document edits while a terminal tab has focus. On Windows and Linux, Ctrl+Z stays with the CLI (#685).
  - AI image tools (remove background, isolate subject, expand, erase) run through the connected agent CLI. They turn off when no agent CLI is installed. Studio bundles no model weights (#177).
- **Agents panel: while an agent works.**
  - Each tab shows live status. Each turn ends with a receipt: how long it worked, how many tool calls, how many layers changed. A system notification tells you when an agent finishes in the background (#683).
  - Review a turn's edits, then keep them or revert them in one undo step. A revert leaves layers you edited yourself (#677).
  - Stop one agent, or all running agents, from the panel. Studio interrupts the CLI, closes the open edit batch as one undo step, and records that you stopped it (#684).
  - An optional ask mode (Preferences > Agent permissions… > Ask before acting) launches CLIs without their skip-permissions flags. Claude's requests show as Approve or Deny in the panel. Codex, Grok, Antigravity and Gemini CLI ask in their own tab. The default stays Skip permissions (#656, PR #817).
  - When a session fails, a banner names the cause and the next step: rate limit, expired login, billing or quota, provider error, full context, or no network (#686). A CLI that fails to start gets the same: not installed, not signed in, or a rejected flag. Any other early exit shows its exit code and the last lines it printed (#673).
  - Claude and Grok tabs show tokens and cost (Grok's cost is a list-price reference, marked ref.). Codex, Antigravity, Gemini CLI and OpenCode show none, because Studio has no usage source for them (#678).
  - Screen readers hear when an agent finishes, needs input or fails, and the panel regions have names. Cmd+Alt+J (Ctrl+Alt+J on Windows and Linux) moves focus between the composer and the canvas. When a text layer is selected or being edited, the chord still justifies text (#687).
- **Agents on the canvas.**
  - Each agent shows a named cursor, and a layer flashes when the agent writes it (#682).
  - When a turn ends with changes off screen, a chip at the canvas edge shows how many. The view does not move until you click it (#695).
  - When you edit inside a frame an agent is working on, a banner names the agent. You can duplicate the frame or edit anyway (#693).
- **MCP: see the design.**
  - `designos_get_screenshot` returns PNG or JPEG image content for a node, for up to 50 nodes on one page, or for a region (#238).
  - `designos_get_metadata` returns a sparse XML outline with bounds, layout, components, text previews and, with `lint: true`, issue counts (#239).
  - `designos_get_design_context` returns code, variable and style hints, and a screenshot for a node (#241). Dev Mode and the MCP server share the same data (#240).
  - Write calls can return before and after images of the region they changed, with `diff_image` (#593).
  - `designos_query_xpath` pages through large documents with a node budget, compact rows and cursors. It can select by lint problem, bound value, hard-coded value or in-progress state (#602).
- **MCP: know the design system.**
  - `designos_get_libraries` lists the local and enabled libraries (#247). `designos_search_design_system` searches components, styles and variables across them and returns instance-ready examples (#272).
  - `designos_get_guidelines` and the `create_design_system_rules` prompt write design-system rules from the open file (#248).
  - On its first turn, an agent gets the open file's outline, variables, components and style. `designos_get_app_state` returns the same on demand. The selection is sent only when the agent asks for it (#601).
  - A rules pack with a build, verify and fix workflow is generated from the tool schemas. It ships with the skill and with each session. `designos.enabled_tools` in the session config limits the tool list (#600).
- **MCP: write safely.**
  - `designos_apply_spec` places component instances and variants, and sets properties and overrides by path (#586). It fills slots (#587). It binds fills, strokes, numbers and styles to variables with `var:` references. The old `$easeui.*` tokens still resolve and warn as deprecated (#588). An agent undo keeps variables and collections you edited after the agent wrote them (PR #803).
  - `designos_apply_batch` applies up to 200 operations as one transaction. It supports `dry_run`, makes one undo step, and returns the created ids and a name-to-id map (#589).
  - A mutating call can carry an `idempotency_key`. A replay with the same key and arguments returns the first reply and writes nothing (#591).
  - `designos_set_in_progress` flags a frame while an agent works on it. The flag clears when the turn ends or you stop the agent. Flags are not undo steps (#592).
  - Every tool advertises an `outputSchema` and returns `structuredContent`. Errors carry a named code, a corrected example and a `safe_to_retry` flag (#590). See Breaking.
  - `designos_create_node` and `designos_update_node` take `fill` and `fills`. On a text layer the colour goes on its text style and runs (#593, PR #813).
  - On files above 25,000 nodes, agents keep the file's variables and styles, so `var:` and style references still resolve (#696).
- **MCP: check the work.**
  - `designos_lint` is one geometric linter with 27 rules, four presets (agent, check-designs, recommended and strict) and eight category presets. It finds overflow, clipping, sibling overlap, truncated and wrapped text, collapsed sizes and detached instances. Check designs shares its detached-instance and contrast checks with it (#594).
  - Contrast is measured against the real backdrop, at WCAG AA and AAA. A finding names the layer that covers the text (#595).
  - Off-scale and hard-coded value rules find spacing, radius and type sizes outside the file's scale, and raw values that match a token (#596).
  - Each mutating call returns a short `issues` block and marks problems that were already there. `designos_configure_session` sets the mode, the preset and the number of items (#597).
  - `designos_lint_fix` applies the deterministic fixes for auto-fixable findings on a page or on chosen layers, in one undo step. Its default preset is strict (#594).
- **MCP tool list.** `docs/mcp-parity.md` maps Figma's 41 MCP tools to Studio's. A test pins the tool list, and the tool tables in the README and the skill are generated from it (#243).
- **Web capture.**
  - A capture script in the source tree (`crates/designos-webcapture/js/capture.js`, run with Playwright) records a rendered page: computed styles, boxes, text runs and images (#276).
  - Studio imports a capture as editable auto layout frames with live text, in one undo step. Use the File menu, drop the file on the canvas, run `--import-capture`, or call `designos_import_web_capture`. A report lists substituted fonts and anything not captured (#277). If the import worker crashes, a toast says so (PR #806).
- **Styles hold variable aliases.** Colour, effect and text styles can point to variables (#131).
- **Variables as Figma JSON and DTCG.** Import and export variables in Figma's native JSON and in W3C Design Tokens (DTCG), from the Variables menu or with `--import-variables` and `--export-variables`. An import merges into the file. Studio reports how many variables it imported and lists any it skipped, and `--import-variables` prints the added, updated and skipped counts. The Figma REST format is unchanged (#138).
- **`.pen` 2.21.** Studio reads and writes JSON scene format 2.21 and migrates older files on import (#570). See Breaking.
- **Check designs.**
  - The rules match Figma's list. A rules popover turns each one on or off and sets contrast to AA or AAA (#158).
  - Ignore keeps a finding off and is saved in the file. Dismiss hides it for the session (#158).
  - Whole pages are scanned a little each frame (2 ms slices), so the canvas stays responsive, with a progress bar. The 20,000-layer cut-off is gone. Fix all waits until the scan ends (#157).
- **Slots.** Stretch on insert applies when you drag a layer into a slot, drop a library asset on it, or place an image, video or SVG in it. Drawn layers keep their size (#644).
- **Dev Mode codegen.**
  - HTML joins CSS, Tailwind, React, SwiftUI, UIKit, Compose and Android XML. It writes a full page or a fragment, from Dev Mode, `--dev html`, `--codegen html` and `designos_generate_code` (#250).
  - React components declare their variant and property props, with defaults and TypeScript unions for variants. Reserved words in prop and slot names are renamed the same way in React, Compose and SwiftUI (#605).
- **Branches in the library, engine only.** A file can keep branch copies and checkpoints (#343). A three-way diff compares nodes, styles, variables and components, and reports conflicts per layer (#344). A merge remaps ids and takes each conflict from main or from the branch (#345). Studio, the CLI and the MCP server do not use branches yet.
- **Vendored skills.** The MIT-licensed `jakubkrehel/skills` are vendored at a pinned commit, with attribution. A rule map ties each of its 273 rules to a lint rule (34), a rubric axis (70) or a skill section (58), or marks it web-only (111). `designos-mcp --list-lint-rules` prints the 27 lint rule ids (#636).
- **Evaluation harness.** `designos-eval` (in the source tree, not in the release packages) runs each design task three times by default. It reports bootstrap confidence intervals, keeps a held-out split, and writes JSONL. `DESIGNOS_MCP_TRANSCRIPT` logs every tool call (#613). A register and a CI check cover the licences of eval datasets, model weights and skills (#611).
- **UI copy standard.** `design/ux-writing.md` ports the VSF PCP writing rules. A gate fails new action labels over three words or with a count in parentheses (PR #804).

### Changed
- **DESIGN:OS Studio is source-available under PolyForm Shield 1.0.0** (PR #820). You can use and modify it, including for paid work. You cannot resell it or use it to build a competing product. Releases up to v0.5.0 were published under MIT OR Apache-2.0 and keep that license. Packages ship one `LICENSE` file instead of `LICENSE-MIT` and `LICENSE-APACHE`.
- **Large `.fig` imports have a global clone budget** of 8,000,000 nodes. Instances past it stay unexpanded, survive a `.fig` and `.designos` round trip, and a warning says how many (#5).
- **`designos_open_file` takes larger files:** up to 512 MiB and 1,200,000 layers. The limits were 64 MiB and 100,000 (#602).
- **UI copy follows the writing standard.** Labels and messages in the Agents panel, Check designs, the variable menus, AI image tools and the web capture report are shorter and verb-first. Tabs, the panel header, Sessions rows, screen-reader announcements and notifications use the same labels for the same agent states: Needs approval, Needs input, Working…, Done, Failed and Stopped. New action labels carry no count in parentheses, and failures never show raw codes (PR #814, PR #817).
- **The Agents panel shows only real output.** Send no longer inserts a template. The canned "Executed via" line and the microphone button are gone (#681). The empty-state category cards and the More button that collapsed them give way to sample prompts (#688, PR #807).

### Fixed
- **Code export.**
  - A child that fills the counter axis of a hugging parent exports its solved length, so a row of fill children no longer collapses to 0 px in CSS or Tailwind (#578).
  - A grid child with vertical fill exports the height the layout solver computed (#639).
- **Constraints.** `.fig` import and the editor use one constraint solver. Rotated children, rounding and sizes under 1 px match Figma (#691).
- **Components and slots.**
  - Migrate to slot on an unbound instance in one variant no longer adds a slot property to the whole set that the other variants never get (#645).
  - Slot content written through MCP, live patches or `.fig` import is marked, so a main-component sync keeps it (#582).
- **Auto layout.** A direction change made through MCP before the first solve no longer stretches a child twice (#583).
- **`.fig` export.** Component keys no longer collide, and half-null GUIDs survive a round trip (#669).
- **Check designs inside instances.** Layers that only mirror their main component are skipped, which cuts the work on large pages. A contrast problem that an override creates inside an instance is reported on the instance (#663).
- **Agents.**
  - Session files live in a per-session folder instead of shared paths under `/tmp`. The scratch folder is removed when the last tab or the window closes. Folders of dead sessions older than 7 days are swept at startup. The live document is written by a worker thread, not the UI thread (#690).
  - On macOS, a terminal tab no longer drops the output of a command that exits at once (#782).

### Breaking
These change what agent integrations and exported files see. No MCP tool is renamed or removed. The old names `export_fig_file`, `query_nodes` and `check_design` still work.
- **MCP errors are named errors (#590).**
  - `structuredContent.error` was a string. It is now an object with `code`, `message`, `path`, `safe_to_retry` and `next_step`, plus `expected`, `valid_options` and `corrected_example` when they apply. The same fields are copied to the top level of the reply and into `content[0].text`. `path` is a JSON Pointer.
  - If you matched on the error text, read `error.code`. Read `error.safe_to_retry` before you retry. Use `corrected_example` and `valid_options` to fix the call.
- **Unknown keys in a spec fail (#590).** `designos_apply_spec` and batch specs reject keys the schema does not list in `NodeSpec` and `LayoutSpec` (and in the new `OverrideSpec`). The error is `UNKNOWN_FIELD`. Remove the key. Documented aliases still work, such as `layout.dir`.
- **`designos_query_xpath` and `query_nodes` return pages (#602).** The reply was `{selector, page_id, nodes, total}` with every match. It is now `{columns, rows, count, scanned, scanned_total, scope_size, remaining, done, complete, cursor}`. A call returns at most 100 rows by default, and at most 500. Call again with `cursor` while `done` is false. Send `format: "objects"` to get objects instead of rows. Old selectors still work.
- **`designos_lint` and `check_design` replies changed (#594).**
  - The reply was `{page_id, findings, total, passed}`, and a finding was `{rule, severity, node_id, name, message}`.
  - It is now `{findings, total, by_severity, by_rule, skipped_rules, scanned, complete, elapsed_ms, cursor}`. A finding carries `node_name`, `xpath`, `bounds` and `fixable`, not `name`. There is no `passed`. A call returns 100 findings by default, so follow `cursor`.
  - Findings use canonical rule ids. `zero-bounds` is now `collapsed-size`. The old ids still work as input.
  - The default preset is `agent`. It no longer reports `default-layer-name`. Pass `preset: "recommended"` to get it.
- **The global live-session files are gone (#690).** Studio and `designos-mcp` no longer use `/tmp/designos-live-context.json`, `/tmp/designos-live-canvas.designos`, `/tmp/designos-live-agent-out.spec.json`, `/tmp/designos-live-doc-sync.designos` or `/tmp/designos-live-selection.spec.json`. Use the folder named by `DESIGNOS_SESSION_DIR` (or the `DESIGNOS_LIVE_*` path variables). Without them, `designos-mcp` leaves those paths unset.
- **Use the `designos-mcp` that ships with this build.** The live edit log has new operations for variables, styles, library state, images and in-progress flags, and new `label`, `session_id` and `agent_label` fields (#586, #589, #592, #696).
- **`.pen` export is version 2.21 (#570).** Studio wrote 1.0. A reader that accepts only 1.0 must read 2.21. Studio refuses `.pen` files newer than 2.21 and names both versions in the error.
- **`DESIGNOS_FORCE_LOCAL_SYNTH` is removed (#681).** Send no longer builds a template, so nothing replaces it.

## [0.5.0] - 2026-10-19

Pack **Components** ([milestone v0.5 Components](https://github.com/jangtrinh/design-os-studio/milestone/3)): slots and a stronger Check designs.

### Added
- **Slots.**
  - Create a slot on a component and set its rules in the slot settings panel: minimum and maximum children, preferred components, stretch on insert, and show empty by default (#32).
  - Edit slot content in an instance without detaching it. The limits hold on every edit: draw, paste, drag, drop, group, ungroup, and placing images or video (#34).
  - An empty slot shows a placeholder with a **+** that opens the preferred-instance picker. Inserting stretches the child to the slot when the slot asks for it (#150).
  - **Migrate to slot** turns an instance-swap or boolean region into a slot and keeps each instance's content, in one undo step (#151).
  - Dev Mode codegen emits slots: one parameter per slot with the slot's own layers as the fallback, in React, Tailwind, SwiftUI and Compose (#152).
- **Component playground** in Dev Mode: try a component's properties and variants without editing the file (#159).
- **Check designs.**
  - Flags hard-coded numbers that exactly match a number variable (#154).
  - Suggests variables and styles from enabled libraries (#155).
  - Audits text and overrides inside component instances. A finding inside a component is reported once, on the main component (#52).
  - Contrast checks use an opaque parent or sibling as the backdrop (#65).
- **Chrome design standard.** One spacing ladder, concentric radii (frame 14, control 6, inset 8), pills for chips and dialog buttons, a neutral selection fill, 10 px captions, and Roboto Mono for code (#580).

### Fixed
- **`.fig` files.**
  - Export uses the field numbering, types and enum values from Figma's own files (#110).
  - Slot preferred values are read and written as Figma's real object (#107).
  - Slot content ids are stored per property, and older files load unchanged (#108).
  - The slot-list parser rejects the raw-number fallback (#111). Unknown slot settings keys are kept (#112).
  - Nested instance swaps update the shell's fills, strokes and properties (#525).
  - Instance text overrides with several paragraphs or mixed styles are kept (#518).
  - Resized instances apply Stretch and Scale constraints to their children (#522).
- **Paste from Figma** remaps slot and instance-swap ids and drops unknown ones (#109).
- **Styles.** Applying a colour style recolours every text run, in one undo step (#68).
- **Agent tabs.**
  - Antigravity and Grok tabs start again: Studio registers its MCP server in the way each CLI accepts (#642, #646).
  - Claude, Grok and Antigravity tabs load the session hooks; Codex tabs keep the existing status detection (#647). The designos-studio skill and rules are reachable from every tab (#648).
  - Codex tabs and Agent Chat pass the session folder and workspace to designos-mcp (#649).
  - A tab that fails to start shows the cause and a way to continue (#650).
- Windows CI: terminal tests use the system temp folder (#581).

## [0.4.1] - 2026-10-19

Patch for **Layout 2.0** ([milestone v0.4.1 patch](https://github.com/jangtrinh/design-os-studio/milestone/46)): fixes found after 0.4.0, many of them from rendering the Untitled UI PRO v8.0 kit against Figma.

### Fixed
- **Grid and auto layout.**
  - Grid placement pins and spans in Dev Mode now match the solver (#531).
  - An instance that restates its grid track list keeps its children in the right cells and its track sizes in the right order (#539).
  - Older grid alignment data migrates on load. Unoverridden alignment axes stay in sync with the main component, and large margins centre correctly (#541).
  - Fill children are measured the same way in flow and grid layouts (#530).
  - Changing a frame's direction no longer stretches a child twice, and an all-stretch line shrinks back. CSS export writes `align-self`, and a grid child exports its solved size (#532).
  - A bound stroke weight is resolved during relayout (#507).
  - Undo and redo re-solve only the layouts they touch, so stroke colour, move and effect edits cost no relayout (#509).
  - The delete action is hidden on custom-sized implicit grid rows (#562).
  - Preset frames get the catalogue corner radius (#498).
- **Rendering.**
  - Frames with "First on top" canvas stacking paint and hit-test in Figma's order on the canvas, in exports and in prototypes. The Layers panel keeps layer order, as Figma does (#517, #527).
  - Strokes on per-side borders follow corner radii (#526). Outside-aligned strokes clip with imported stroke geometry (#524).
  - Radial, angular and diamond gradients use centred handles (#529).
  - Soft line breaks (U+2028) break the line without adding paragraph spacing (#523).
  - A bound colour variable's alpha is applied once (#516).
  - The mask stand-in no longer overdraws or disappears at sub-pixel sizes (#500).
  - Children of unknown layer types paint and can be selected (#511).
- **`.fig` import and export.**
  - Per-side stroke weights are imported only when they differ, and partial overrides merge (#515).
  - Style records no longer turn into canvas nodes in libraries, and instance layout alignment survives a round trip (#546).
  - An instance whose auto layout was removed writes `stackMode: NONE`, so the removal survives a round trip (#539).
- **Files and libraries.**
  - Non-finite numbers in imported files are cleaned up in memory, and a failed write rolls back (#56). The open document, hot reload and file switch stay consistent with the notice (#506).
  - Replacing a repaired original releases its cache (#505).

## [0.4.0] - 2026-10-18

Pack **Layout 2.0** ([milestone v0.4](https://github.com/jangtrinh/design-os-studio/milestone/2)): auto layout and grid behave the way Figma does.

### Added
- **Grid auto layout.**
  - More than 8 rows and columns (#24).
  - Auto-placed items and auto rows. `.fig` files keep their version 106 track maps (#25).
  - Children align horizontally and vertically inside their cell (#145).
  - Dev Mode prints CSS, Tailwind and React grid code from the real tracks and placement (#146).
  - Edit tracks on the canvas and in the Properties panel: insert, delete, resize, and drag a track to reorder it in one undo step (#144).
- **Auto layout.**
  - Child margins from `.fig` files (#29).
  - Per-child align-self in auto layout version 1 (#148).
  - Version 1 semantics, with borders inside the box. An **Update auto layout** action upgrades an older frame (#26).
- **Layout guides match Figma** (#147).
  - Alignment: Stretch, Left, Center and Right.
  - A fixed column or row width.
  - Auto count.
  - Show or hide each guide.
  - All of it round-trips through `.fig`.
- **Check designs** (#35).
  - A panel that audits the page for low contrast, unbound colours, detached styles and detached instances.
  - A one-click Fix per finding, and Fix all.
  - Contrast fixes skip text when something painted under it would make the check unreliable.
  - Text runs that have their own colour keep it when a variable or style is applied.
- **Vectors.**
  - Offset path (#195).
  - Scale several selected points with a bounding box (#186).
- **Export.**
  - PDF export draws shapes as vectors, and its text is selectable. In 0.3.0, PDF pages were images (#40).
  - Lossless WebP (#215).
- **Frame and device presets** match Figma's current list (#149).
- **Board timer** for facilitators in the board toolbar (#338).
- **Layers panel** shows a label for each layer type (#405).
- **Variables** from a library are grouped under *From library* and are read-only (#279).
- **Style editing.** Drag the label prefix to scrub number fields in the style popover (#69).
- **`.fig` round trip.**
  - Noise, texture and glass effects survive import and export (#106).
  - Fields and layer types Studio does not draw yet are kept and written back on export (#174).
  - Slots are read and written (#31).

### Fixed
- **An open library file could be emptied (#504).** Switching tabs or going Home could write an empty document over the open file and store an empty version. Unsaved edits are now saved before the tab is parked.
- **Crash on large pages at rest (#467).** Some large pages with many effects crashed about 1 s after opening. The full-quality refine is now split over several GPU submissions; Metal allows about 2,047 render passes per submission.
- **Security: copied or selected content is never written into a Shell tab (#513).**
- **Rendering.**
  - A mask frame without its own fill takes its alpha from its children (#448).
  - Hidden icons inside button components stay hidden (#450).
  - `--screenshot --gpu` captures the full-quality frame at rest (#449).
- **Layout correctness.**
  - Fill siblings share the space a clamped child cannot use (#60).
  - Grid tracks size from clamped children (#61).
  - The grid axis convention is the same in layout, export and the Properties panel (#63).
  - Variable-bound sizes resolve in headless relayout (#62).
  - Deep re-solves have a budget, and nothing past it goes stale silently (#59, #489).
- **Files and saves.**
  - Studio tells you when numbers that were not numbers (NaN or infinity) were replaced (#57).
  - Malformed native and library files open with a warning and keep a backup of the original (#7, #104). An existing `.bak` is never overwritten (#102).
  - Repaired original bytes are released when the tab closes (#103).
  - Zip bombs in `.fig` files are rejected before they are buffered (#9).
  - Atomic save edge cases are handled (#4).
  - NaN and infinity never reach grid track strings, variables or the clipboard (#71).
- **Components and styles.**
  - A per-instance style override survives component sync (#73).
  - A text in Height mode no longer records a Size override when a style changes (#72).
  - Library styles cannot be renamed, and the menu says why (#67, #100).
  - Effect and style popovers no longer show duplicate prefix letters (#101).
- **Agents and live sync.**
  - Each Studio window has its own agent session folder (#80).
  - A retried document switch does not log the same edit twice (#98).
  - Dropped edits are no longer counted twice (#97).
  - Comment edits update the canvas (#78).
  - Edits that would break builder-replaced layers are refused (#95, #96).
- **Idle CPU is back to 0%** (#12).

### Changed
- **One auto layout solver.** The editor, the exporters and the MCP server share it (#30).
- **The GPU refine has a program ceiling, and its VRAM budget counts the root target** (#83).
- **The homepage** is a dark landing page with the 0.3.0 film.

### Under the hood
- The document model has comment author identity (#351) and 15 easing types (#139). The editor does not write comment author identity yet (#472).

### Known issues
- Writing a large `.fig` and reading it back adds nodes on every cycle, and one nested instance can lose its centred alignment (#546). In 0.3.0 such files could not be written at all.
- The Windows test job is red (#487). Windows and Linux builds are tested by their users; releases are verified on macOS.
- Follow-ups from the Layout 2.0 reviews ship in [v0.4.1](https://github.com/jangtrinh/design-os-studio/milestone/46).

## [0.3.0] - 2026-10-17

Pack **Paste & Styles** ([milestone v0.3](https://github.com/jangtrinh/design-os-studio/milestone/1)).

### Added
- **Paste from Figma (#27).**
  - Copy a selection in Figma desktop and press Cmd+V on the canvas.
  - The layers land centered in the view as one undo step, images included.
- **Effect and grid styles (#27).**
  - Create, apply, detach and rename them from the Effects and Layout guide sections.
  - They also show under local styles.
  - `.fig` files bring their effect and grid styles with them.
- **Edit a style from the style picker (#44).** Changing a style's values updates every layer that uses it, as fill or as stroke, in one undo step. Library styles stay read-only.
- **PDF export (#27).** Selected frames export as one multi-page PDF from the Export panel. Pages are raster images for now.
- **Auto layout (#27).**
  - Space around and Space evenly.
  - Wrap in the vertical direction, with wrapped lines that can spread.
- **Text wrap Balance and Pretty (#27).**
- **Variables (#27).**
  - Paint and effect opacity can be bound to a number variable.
  - Variables keep their code syntax (Web, Android, iOS), which Dev Mode prints the way Figma does.
- **Dev Mode: UIKit and Android XML (#27).**
- **Clean captures:** `--no-toast` and `--no-caption` for `--screenshot` (#13).

### Fixed
- **Agent edits keep the open document intact (#28).**
  - `designos_apply_spec` and other edits made through the bundled `designos-mcp` server no longer replace the open document. On large files, the other frames used to become empty boxes and the Text styles list emptied.
  - Edits are replayed as node operations: one undo step per batch, the new layer is selected, and the view stays where it was.
  - Agent edits wait while you drag or edit text, then land, and they survive switching documents.
  - Undo through the MCP server is refused once the canvas has reloaded, so it can never remove your own layers. Undo in Studio works as before.
  - Update `designos-mcp` and Studio together; the bundled server matches the app.
- **Files with broken numbers open again (#42, #6).**
  - A value that was not a number (NaN or infinity) used to be saved as `null`. The file then refused to open with `invalid type: null, expected f32`.
  - Such values are now never written. Files that already contain them open, with each one replaced by a neutral value.
- **Large pages render at low zoom (#13).** The GPU canvas renders each frame in its own submission. Pages such as a 68,698-layer `.fig` at 3% zoom no longer show an empty canvas.
- **Untrusted files and saves (#1).**
  - `.fig`, kiwi and `.pen` decoding is bounded (total inflate budget, entry and depth caps).
  - Saves are atomic (temp file in the same folder, fsync, rename).
  - Documents with dangling or cyclic layer references are repaired on load instead of crashing; the MCP `open_file` tool refuses repaired documents.
- **The app reports its real version.**
  - The workspace version was still `0.1.0` in v0.2.0, so the About dialog and the updater misreported it.
  - The release workflow now fails if the tag and `Cargo.toml` disagree.

### Changed
- **Variable references in CSS, React and Tailwind (#27)** keep the variable's case and print a fallback, for example `var(--Color-Primary, #0D99FF)`.
- **Idle full-quality GPU refine (#1).**
  - After 250 ms without changes, the canvas renders one uncapped frame (masks, blur, blend modes, group opacity), bounded by a 512 MiB VRAM budget.
  - Any interaction returns to the capped interactive path.
- **Lower memory on `.fig` import.** Peak RSS on large files is about 6.4 GB lower, and Phase 2 engine optimizations were added.
- **Removed UI that did nothing (#1).**
- **The README and the site open with the 70-second promo film (#48).**
- **Releases are verified on macOS.** A failed Windows or Linux build no longer blocks the macOS release; those packages are attached when their builds pass.

## [0.2.0] - 2026-10-15

### Added
- **Cross-Platform Installers (`DIR-22`)**: Added one-command release builder (`scripts/build-release.sh`) producing macOS `.dmg` (`designos.app` + `install-cli.command`) + `.tar.gz`, Windows NSIS `-setup.exe` + `.zip` (`install.ps1`/`uninstall.ps1`), Linux Debian `.deb` + portable `.tar.gz` (`install.sh`), `dist/release-manifest.json`, and `dist/SHA256SUMS`.
- **Native Agentic PTY Chat UI Wrapper (`DIR-21`)**: Added structured User/Assistant chat turn bubbles (`AgentChatTurn` / `AgentChatRole`) inside the `Agents` Left Rail (`RailItem::Agents`) wrapping live `portable-pty` + `vt100` sessions for Claude Code (`claude`), Codex (`codex`), and Antigravity CLI (`agy`) with inline `NodeSpec` detection and 1-click Canvas apply.
- **100% Unified Phosphor Icons System (`DIR-19`)**: Unified all UI3 chrome, toolbars, left rail, inspector, and Spec Reader surfaces on official Phosphor Icons (`@phosphor-icons/core` `256x256` SVG rasterized via `resvg`).
- **11-Stage `PRODUCT:OS Spec Driven v2` Reader (`DIR-18`)**: Full in-app Spec Reader (`--panel specs` / `--rail specs`) rendering `US-01..US-15` User Stories, Gherkin `Given/When/Then` scenarios, and `g1..g5` verification gates.
- **1:1 Figma `126.9.13` UI3 & `.fig` v106 Parity (`387 / 387 — 100.00%`)**: Complete visual chrome, typography, Auto-Layout 5.0, Variables, Components, Prototype flows, Dev Mode codegen, and native `.fig` v106 binary read/write parity verified by `823` automated tests.
- **AI-Ready Repository Delivery & 9-Tier GitHub Pages (`DIR-23`)**: Added `.claude-plugin`, `.codex-plugin`, `.agents/plugins`, `.factory-plugin`, `skills/design-os-studio/SKILL.md`, `/doctor`, `scripts/self_check.py`, 12-section editorial `README.md`, and 9-tier `docs/index.html` with 15 visual proof captures.
