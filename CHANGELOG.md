# Changelog (`design-os-studio`)

All notable changes to `design-os-studio` (**DESIGN:OS Studio**) are documented in this file.

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
