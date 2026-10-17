# Figma product catalog as of 2026-10-17 (Sonnet researcher; official sources up to 2026-10-15)

Sources: release notes https://www.figma.com/release-notes/ (RN, cited by date), plugin updates https://developers.figma.com/docs/plugins/updates/ (PU), REST changelog (RC), MCP docs https://developers.figma.com/docs/figma-mcp-server/, Code Connect docs, Config 2025/2026 forum recaps, and Help Center categories.

Status codes: GA · GA* (long-standing; not re-verified at article level) · open beta · closed beta · preview · early access · LOW-CONF.

## 1 Canvas & tools
- UI3 only (Apr 2025), with a left nav for library, variables and search. GA.
- Toolbar modes Design / Draw / Dev / Motion. Motion is open beta.
- Core tools, view options, nudge, actions menu, full keyboard model. GA*.
- Accessible keyboard canvas controls and screen reader support (RN 2025-07/10). GA.
- Eyedropper that samples anywhere on screen (macOS, RN 2026-03-27). GA.
- Desktop tabs: groups, muting, agent chat in a separate window (RN 2026-03…08). GA.
- On-canvas video playback, 300 MB uploads (RN 2026-04/06). GA.
- Mobile app (RN 2026-09-30). GA.
- Enhanced-contrast accessibility mode. GA.

## 2 Layers & selection
- Layer types include slots, text-path and transform group (PU 123). GA.
- Select matching; multi-edit. GA*.
- Layer-type labels; Slides sections in the layers panel (RN 2026-04/05). GA.
- Masks, flatten, outline stroke, text to paths. GA*.
- Aspect ratio lock (RN 2025-02-06). GA.
- Prompt-to-edit layers, now folded into the Figma agent (RN 2026-05-20).

## 3 Shapes / vector / boolean / Draw
- Vector networks. GA*.
- Rewritten boolean ops (RN 2025-05-07). GA.
- Shape builder, lasso, multi-edit vector. GA.
- Vector bounding box for multiple points (RN 2026-02-03). GA.
- Cut tool, offset vector, simplify vector (RN 2025-07/09). GA.
- Eraser and paint bucket (RN 2026-08-24). GA.
- Text on a path, `TEXT_PATH` (RN 2026-04-29). GA.
- Variable-width / dynamic strokes, stretch and scatter brushes (RN 2025-05…08). GA.
- Pattern fills and repeaters via transform groups (linear/radial). GA.
- Stroke sampling. GA.
- Vectorize (AI image to vector, RN 2026-02-04). GA.
- Arcs, polygons, stars, per-corner radius and smoothing. GA*.
- Shaders: agent-built WebGPU custom fills/effects (Config 2026, RN 2026-09-01). Open beta.

## 4 Text & typography
- Text properties and styles. GA*.
- Uploaded local fonts; Apple fonts available. GA.
- Variable font axes (Plugin API). GA.
- Smart symbols. GA.
- Text wrap Balance/Pretty (RN 2026-08-14). GA.
- Text animation in Motion (RN 2026-09-30). Open beta.
- Text to paths. GA.
- AI autosuggest and rewrite. GA.

## 5 Auto layout / grid / constraints
- CSS-aligned auto layout update (RN 2026-07-24). GA.
- Vertical wrap (RN 2026-09-25). GA.
- Auto spacing Between / Around / Evenly (RN 2026-08-21). GA.
- Grid auto layout: fr units, spans, drag-reorder tracks, auto rows, gap variables, CSS grid in Dev Mode (RN 2026-05-22). GA.
- Min/max, fill/hug/fixed, absolute position. GA*.
- Constraints, layout guides, device frames incl. iPhone 17. GA.
- Responsive web import via Chrome extension and MCP (RN 2026-06/07). Beta.

## 6 Components
- Components, variants and properties (boolean/text/instance-swap). GA*.
- Slots: freeform child area in instances, min/max, preferred instances (GA Jun 2026, `SlotNode`). GA.
- Swap instances, migrate a library to slots. GA.
- Component playground in Dev Mode. GA.
- Check designs: hard-coded values, WCAG contrast, detached components, one-click fix (RN 2026-06-04, Org/Ent). GA.

## 7 Styles & variables
- Color, text, effect and grid styles. GA.
- Variables with collections and modes (10/20 per collection). GA.
- Scopes, aliases, code syntax. GA.
- Extended collections for theme/brand inheritance (RN 2025-11-20, Ent). GA.
- Variables import/export JSON (RN 2025-11-20). GA.
- Opacity variables (RN 2026-09-03). GA.
- Easing/timing variables for Motion. Open beta.
- Variables in effects, grid gaps and prototypes. GA.
- Color spaces P3, OKLCH, OKLAB, HSL, HSB: Plugin API GA; UI LOW-CONF.
- Gradient variable binding. GA.

## 8 Effects & paints
- Shadows and blurs; limits of 8 drop, 8 inner, 1 layer blur and 1 background blur per layer. GA.
- Progressive blur (Config 2025). GA.
- Noise and texture with independent X/Y. GA.
- Glass with splay and variable binding (GA Jan 2026).
- Fills: solid, gradients, image, video, pattern. GA.
- Blend modes and complex strokes. GA.
- Custom shaders. Open beta.
- Image tools: crop, expand, isolate, erase, remove background (AI) (RN 2025-12-10). GA.
- 3D transforms. Early access.

## 9 Prototyping / Motion
- Flows, triggers, overlays, scroll/fixed, device. GA.
- Smart animate. GA.
- Variables, expressions, conditionals, multiple actions, set-variable. GA.
- Video and GIF in prototypes. GA.
- Offline present, mobile view, comments on prototypes. GA.
- AI "Add Interactions" (RN 2025-03-28). GA.
- Embedded Make prototypes. GA.
- Motion timeline: keyframes, presets, spring, anchor point, motion path, path trim (`figma.motion`). Open beta.
- Animation styles published to libraries, audio on the timeline, text animation. Open beta.
- Motion export: CSS, JSON, React, MP4, WebM, animated SVG, GIF, Lottie. Open beta.
- Code layers: code-backed interactive layers from a frame, repo or prompt (RN 2026-10-15). Open beta.

## 10 Dev Mode / MCP / Code Connect
- Inspect, measurements, annotations. GA.
- Ready for dev, statuses, notifications, webhooks. GA.
- Compare changes. GA.
- Focus view and interactive inspection. GA.
- Dev resources (GitHub, Jira, Storybook). GA.
- Codegen plugins. GA*.
- VS Code, Jira and Google Chat integrations. GA.
- Code Connect: CLI and UI, React, HTML/Angular/Vue, SwiftUI, Compose, multiple mappings per component. GA.
- Remote MCP server with 41 tools (design context, variables, screenshots, download_assets, search_design_system, get_motion_context). GA.
- MCP writes to the canvas via `use_figma`. Beta.
- MCP code-to-canvas: `generate_figma_design`, `html_to_figma`. GA/beta.
- Desktop MCP. GA.
- Specs: a native home for written intent, for teams and agents (RN 2026-10-15). Open beta.

## 11 Collaboration
- Live multiplayer, cursors, cursor chat, spotlight, observation. GA.
- Comments, including on prototypes. GA.
- Viewer history. GA.
- Notifications via Slack, Jira and Google Chat. GA.
- Agent collaboration: watch teammates' agents (RN 2026-10-06). GA.

## 12 Import / export
- Export PNG/JPG/SVG/PDF with suffix, color profile, outline text, simplify stroke. GA.
- Video export (MP4/WebM/GIF via Motion). Open beta.
- Import .fig and .sketch. GA.
- Web capture via Chrome extension / html_to_figma. Beta.
- Make output to design layers. GA.
- PPTX import/export in Slides. GA.
- Paste SVG/vector from other tools. GA*.

## 13 Files / libraries / branching / versions
- Nested folders, 10 deep (RN 2026-08-03). GA.
- Folder preview and workspace sharing. GA.
- Libraries: publish, subscribe, review updates, swap. GA.
- Library guidelines markdown read by the agent (RN 2026-10). Open beta.
- Centralized design systems that bundle code libraries (RN 2026-10-15). Preview.
- Branching and merging: branch, review request, pull from main, merge. GA.
- Version history with auto, named and branch checkpoints. GA.
- Community: files, plugins, widgets, Apps, skills, Weave. GA.

## 14 Plugins / API
- Plugin API (update 141, Oct 2026). GA.
- Generative plugins built by the agent. GA.
- Widgets. GA.
- Dev Mode plugins. GA.
- REST API: files, images, versions, comments, folders, variables, dev resources, webhooks V2, analytics, oEmbed, AI usage. GA.
- Plan Access Tokens. GA.

## 15 AI
- Figma agent: generate/remix, bulk edit, critique, file search, image model picker, web search, custom skills, AI credits (GA 2026-10-06).
- Agent skills published to Community. GA.
- Agent in FigJam/Slides. Early access.
- Rename layers, visual search, Make/Edit Image, Expand/Isolate/Erase/Vectorize. GA.
- First Draft: replaced by the agent? LOW-CONF.
- Weave: node-based AI workflows. Beta/GA mixed.
- Figma Make: prompt to app, plan mode, point-to-edit, model picker, version history (GA Jul 2025+).
- Make backends: Supabase and Payload. GA / closed beta.
- Make kits and connectors (Notion, Linear, Atlassian, Zapier, GitHub). GA.

## 16 FigJam
- Sections, mind maps, quick create, templates.
- Meetings tools: timer, voting, spotlight, cursor chat, stamps.
- Tables with merge.
- Diagramming: arrows, Mermaid paste, generate_diagram, ERD.
- AI summaries and integrations.
- Import from Mural, Lucid and Jamboard.
- iPad, widgets.

## 17 Slides
- Decks, design mode, libraries, templates.
- Sections.
- Interactive prototypes, object animations.
- Presenter notes, skip, spotlight.
- PPTX import/export.
- MCP use_figma.
- AI image tools.

## 18 Sites (beta)
- Pages, breakpoints, blocks, 50 templates.
- Marquee, parallax, video fill.
- React code layers.
- CMS with collections and CSV (public beta).
- Publish with custom domains, password, SEO.

## 19 Buzz (beta)
- Brand-locked templates; focused and freeform editing.
- Bulk create/resize from a spreadsheet.
- Approvals, video, Unsplash, AI tools.

## 20 Admin
- Seats and plans, governance, GitHub orgs, regional hosting.

## Top 15 for professional UI designers (INFERENCE)
1. auto layout · 2. components/variants/props · 3. variables/modes · 4. styles · 5. frames/constraints/grids/responsive · 6. vector/boolean/masks · 7. libraries · 8. prototyping · 9. comments + multiplayer · 10. Dev Mode + Code Connect/MCP · 11. text engine · 12. export · 13. plugins · 14. effects · 15. version history + branching

## Unresolved
- Is OKLCH/P3 exposed in the UI?
- Sites/Buzz beta labels.
- Is MCP code-to-canvas GA or beta?
- Is First Draft fully replaced?
- Which plans get branching?
- WebP export, `.deck`/`.jam` formats.
