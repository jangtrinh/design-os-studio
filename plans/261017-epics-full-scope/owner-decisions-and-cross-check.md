# Consolidated owner decisions (E4, E6–E10) and cross-epic gap check

2026-10-17. Answers are recorded below as they arrive.

**Next action:** ask Round 1 below with AskUserQuestion. Every recommended option is listed first.

### Epistemic Ledger
- **FACT:** I read both full-scope files: their decision sections, task-title lists and cross-epic notes. Counts: 87 tasks in the E4/E6/E9 file and 85 in the E7/E8/E10 file. I also read gap-list.md, the catalog and the REA files.
- **FACT:** I fetched Figma's public MCP tools page this session. It lists 32 tools plus 9 Weave tools, which is 41. That matches the catalog's "41". The E7/E8/E10 file still calls this list unknown (its :21, :619).
- **FACT (from grep this session):**
  - Nested folders already exist (`Folder.parent`, `designos-library/src/store.rs:30`).
  - The paint model has no GIF (`paint.rs`).
  - `present.rs` has no presenter-view code.
- **INFERENCE:** what blocks what, and in which order. This comes from the two files' pack orders.
- **Not verified:** the cross-epic gaps were found by keyword grep over the two files, not a line-by-line read. A gap could still sit inside a task body under a different word.
- **GitHub REST calls:** none.

---

## 1. Consolidated owner decisions (18 questions, 5 rounds)

**Dropped because the full-scope ruling answers them:**
- E4 Q3, E7 D7.4: which pack first.
- E6 Q2: which exports go in M1.
- The draft-era questions on Mermaid scope, mind maps, presets, D7.4 codegen and D10.1 "who are branches for".

**Dropped because they are technical, and the controller decides:**
- PPTX ownership: both files already say E8.
- The MCP HTTP port: the fixed-port contract allocates it from the registry.
- The `deny.toml` gate: this is task X3.

**Merged, because the same theme appears in several epics:**
- Multiplayer: E9 Q1, D10.1 and D8.1.
- Hosted services: D7.1, D8.1 and D10.1.
- Remote AI: E4 Q2, E9 Q1 and E9 Q2.
- Built-in content (Figma IP): E4 Q1 and E9 Q3.

### Round 1: blocks the plugin spike T8.3 and pack E7.1
1. **Plugin compatibility corpus** (the T8.11 gate; T8.3 needs it)
   - **(a) Recommended: the controller proposes 8 open-source Figma plugins and you approve the list.** You don't have to come up with names, and the API surface gets fixed now.
   - (b) You name the plugins yourself. Most accurate, but slower.
   - (c) No corpus; the gate is 100% `@figma/plugin-typings` coverage only. Easy to measure, but it can pass while real plugins still break.
2. **How do Figma plugins get in?** (D8.2)
   - **(a) Recommended: "Import plugin from manifest…", from a local folder or repo.** This is Figma's own dev path, with no ToS exposure.
   - (b) Repackage plugins into the native registry. Curated, but needs a licence review per plugin.
   - (c) Both.
3. **Where does a plugin's UI show?** (D8.3)
   - **(a) Recommended: an embedded webview (wry/WKWebView), with the system browser as fallback.** It gives the floating panel Figma has. Cost: Linux needs WebKitGTK.
   - (b) System browser only. No new dependencies, but users switch windows.
4. **Should MCP tools default to the editor selection?** (D7.3; confirm only, because the parity rule already points to (a))
   - **(a) Recommended: yes, when attached to a Studio window.** This is how Figma's desktop MCP works. It means T7.5 has to wait for live sync (#75–#81).
   - (b) Always require `node_id`.

### Round 2: blocks E8.2, E10.1 and S2
5. **Plugin network access** (D8.4)
   - **(a) Recommended: only the manifest's `allowedDomains`, after a one-time consent per plugin.** Mirrors Figma, and offline stays the default.
   - (b) Block all network. Breaks Unsplash/Iconify-type plugins.
   - (c) Allow all.
6. **Merge conflict granularity** (D10.2)
   - **(a) Recommended: per layer.** Different properties merge on their own; you pick main or branch only for a layer where both sides changed the same property. Fewest false conflicts.
   - (b) Pick a whole frame. Simplest, but loses more work.
   - (c) Pick per property. Most control, heaviest UI.
7. **Where branches appear and what happens after a merge** (D10.3 + D10.4; confirm only, since parity and Law 1 already point to (a))
   - **(a) Recommended: branches live inside the file, with a title-bar switcher; a merged branch is archived, read-only and restorable.** Figma parity, and nothing vanishes.
   - (b) Branches show as separate Home files; merged branches are archived.
   - (c) Branches live inside the file; merged branches are deleted.
8. **Built-in content: brushes and templates** (E4 Q1 + E9 Q3)
   - **(a) Recommended: ship our own original brush set and templates, render brushes from imported `.fig` files, and add "Save as template".** Same behaviour as Figma, no IP risk.
   - (b) Render imported brushes and offer "Save as template" only. Cheapest, but Draw and Home start empty.
   - (c) Copy Figma's 25 brushes and Community templates. Exact look, but a legal risk.

### Round 3: blocks E7.3, E8.3 T8.19 and X3
9. **Code Connect: which side wins when a code repo is linked** (D7.2)
   - **(a) Recommended: the repo files win.** The document keeps a synced copy, and the UI shows "Edit in code". Figma parity, and the same pattern as ruling A13.
   - (b) The document wins, and the CLI only seeds it. Designer-owned, but it can silently drift from the code.
10. **Figma-hosted services** (D7.1 + D8.1 + D10.1 hosted part). Covers Community install/publish, plugin payments, REST/webhooks/PATs, hosted MCP, hosted notifications, workspace sharing, viewer history and Make import.
    - **(a) Recommended: exclude all of them and ship the local equivalents already tasked.** Those are: the local notification inbox, the outgoing status hook, opt-in Jira/Google Chat/VS Code add-ons, and plugin stubs that report "single local user, free". Offline stays the default.
    - (b) Strict: exclude them, have plugins throw a named "unsupported" error, and drop Jira and Google Chat.
    - (c) Option (a), plus the existing native plugin registry as a local "community" channel.
11. **Multiplayer-only features** (E9 Q1 + D10.1 + D8.1). Covers live cursors, cursor chat, spotlight, observation, live voting and live slide polls.
    - **(a) Recommended: exclude them and rely on the async equivalents already tasked:** branch review state, git merge driver, a synced-folder library root, and comments that merge across branches. These are the only team channels that fit offline-first.
    - (b) Option (a), plus solo variants: local vote tallies, and "presenter-follow" spotlight in Present. Costs about 2 M tasks.
    - (c) A self-hosted sync server. This breaks offline-first.
12. **Licence policy: MPL-2.0 `symphonia`** (audio decode for M5; it shapes the X3 allowlist)
    - **(a) Recommended: allow MPL-2.0 as an explicit exception in X3.** Its copyleft is file-level, so it does not affect MIT/Apache distribution (INFERENCE).
    - (b) Allow MIT/Apache/BSD only, and decode audio through the OS (AVFoundation/MF). Clean licences, but code per platform.

### Round 4: blocks the later packs (M3, M4, E8.6, D6, J3)
13. **Remote-AI features** (E4 Q2 + E9 Q1/Q2, plus the orphans `generate_image` and Weave)
    - **(a) Recommended: use local non-AI equivalents where they exist, and route the rest through the Studio Agent runtime (the user's own CLI).** Vectorize becomes an offline tracer (vtracer; its licence is an ASSUMPTION). Summaries and the agent in FigJam/Slides run through the agent runtime. AI image tools, `generate_image` and Weave are excluded. No model is built into the app.
    - (b) Route everything, vectorize included, through the agent runtime. Quality depends on the user's CLI.
    - (c) Exclude all AI features. Zero cost, but the catalog gaps stay open.
14. **How to encode MP4 (H.264 + AAC)** (E6 Q1)
    - **(a) Recommended: the OS encoders (VideoToolbox/AudioToolbox on macOS, Media Foundation on Windows), with AV1 + Opus on Linux.** Licence-clean and plays everywhere. Cost: code per platform.
    - (b) Compile `openh264` in. Cisco's patent cover applies only to its downloaded binary, which needs the network.
    - (c) Use the user's ffmpeg when installed. Missing for most users.
    - (d) AV1 everywhere. Pure Rust, but QuickTime and PowerPoint play it poorly.
15. **Web capture vs your "never html-to-figma" rule** (blocks E8.6 and T7.14)
    - **(a) Recommended: the rule covers only the Figma write workflow; Studio's own web-capture importer is allowed.** That keeps catalog §12 and two MCP tools in scope.
    - (b) The rule covers the importer too: drop E8.6 and T7.14 (5 tasks).
16. **React motion export flavour** (E6 Q3; confirm only, since the parity rule already points to (a))
    - **(a) Recommended: whatever Figma emits, after an REA read, with CSS-in-JSX as the fallback.** Parity.
    - (b) Always emit `motion` (framer-motion) code. Idiomatic, but adds a dependency to the user's project.

### Round 5: no early pack depends on these
17. **Board import from other tools** (E9 Q4)
    - **(a) Recommended: Lucid and Mural importers in J4; drop Jamboard.** INFERENCE: Jamboard is discontinued and exports PDF only.
    - (b) Move the importers to E8.
    - (c) Option (a), plus importing Jamboard PDFs as images.
18. **iPad and mobile apps** (E9 Q1 iPad, plus the orphan catalog §1 mobile app and gap-list P3 mobile prototype preview)
    - **(a) Recommended: keep them out of all six epics, and decide later as a separate platform epic.** It is a different platform, not parity work inside the desktop app.
    - (b) Exclude them permanently.
    - (c) Add an iPad target to E9.

---

## 2. Cross-epic gap check (in neither file, and not E1/E2/E3/E5)

**Duplicates between the two files (fix before filing issues):**
- `.deck`/`.jam` import: E9-6 and E9-22 vs T8.30.
- FigJam widgets: E9-26 vs T8.20a–c.
- Fix: keep the E8 tasks, and turn the E9 ones into dependencies.

**Gaps to plan:**
1. **MCP tools with no task** (FACT, from the docs page): `whoami`, `create_new_file` (partial: only `new_document` exists), `upload_assets`, `get_libraries`, `get_context_for_code_connect`, the `create_design_system_rules` prompt, and 5 shader tools (`get_shader`, `list_shaders`, `list_file_shaders`, `create_shader`, `update_shader`; these need the E5 shader node).
   - `generate_image`, `send_code_to_figma` and the 9 Weave tools go under decisions 10 and 13.
   - Give T7.6 the full list.
2. **Prototyping has no epic.**
   - The gap list shows Prototyping at 5 MISSING and 1 PARTIAL.
   - Missing pieces: video/GIF in prototypes (GIF paint: 0 hits), comments on prototypes, mobile view, AI "Add Interactions", embedded Make prototypes, code layers.
   - E9-9 covers slides only.
3. **Canvas, accessibility and text have no epic.**
   - Canvas & tools: screen reader and keyboard canvas controls, enhanced-contrast mode, screen-wide eyedropper, on-canvas video playback with 300 MB uploads, and the tab groups' "agent chat window".
   - Text: uploaded local fonts, Apple fonts, variable font axes, smart symbols. These may already exist; nobody has checked.
4. **Export options:** export suffix, color profile, outline text and simplify stroke (catalog §12). Frame PDF is tracked in #40 / PR #88, outside the epics.
5. **Libraries:**
   - Library guidelines markdown read by the agent (gap-list P3).
   - Centralized design systems that bundle code libraries (preview).
   - INFERENCE: both fit E7 agent context unless E3 claims them.
6. **AI area:** the on-canvas Figma agent (gap-list P2 L), rename layers, visual search, prompt-to-edit. E9-23 covers board and deck modes only. The remote-AI decision (13) applies here too.
7. **Sites** (5 MISSING; gap list says only a static-HTML slice is plausible) and **code layers:** no epic and no exclusion decision.
8. **Small items:**
   - Slack notifications are missing from D7.1, which lists only Jira and Google Chat.
   - "Watch teammates' agents" belongs under decision 11.
   - Layer-type labels and Slides sections in the Layers panel.
   - Device frames such as iPhone 17: unclear whether E2 owns them.
   - Non-AI image crop and video fill: confirm with the E5 auditor.
9. **Repo honesty fixes from gap-list "overstated claims" #1, #2, #5, #6:** the README says 487/487 vs 387/387, the parity meter only regex-matches (and passes features that were removed), the frozen "100%" list, and "Make Design" has no LLM behind it. These are not capabilities, but no task owns them.
10. **Two hot-file queues.** X1 (`fig.rs` passthrough, first in the E4/E6/E9 file) and the E7/E8/E10 queue (T7.19 → T7.23 → T8.4 → T8.20c → T8.29) are sequenced separately. They need one merged queue.
- **Out of gap-list scope, noted only:** Buzz, Admin, and Weave as a product.

## Unresolved questions
- Gap items 2, 3, 7: open new epics, or fold them into existing ones? That's for the controller or owner.
- Gap item 3 (text and canvas): these need a code check of what already exists before anyone sizes them.

Status: DONE_WITH_CONCERNS
Summary: The owner decisions from both full-scope files are deduped into 18 questions over 5 rounds, ordered by which pack each one blocks, with confirm-only questions marked. The gap check found 2 cross-file duplicates, 11 MCP tools with no task, 3 areas with no epic (prototyping, canvas/accessibility/text, AI/Sites), and two hot-file queues that need merging.
Concerns/Blockers: The gap check was a keyword grep, not a full read. Questions 4, 7 and 16 lean on a "follow Figma" owner rule that the draft cites, but I did not find it in a decisions ledger; check that before treating them as confirm-only. Assumption: Jamboard is discontinued (medium confidence). Evidence that would flip it: a live Jamboard export API.

## Owner answers

### Round 1 (2026-10-17)
1. Plugin compatibility corpus: **the coordinator proposes 8 open-source Figma plugins and the owner approves them.**
2. Getting Figma plugins in: **"Import plugin from manifest…" from a local folder or repo.**
3. Plugin UI: **an embedded webview (wry/WKWebView), falling back to the system browser.**
4. MCP default target: **behave like Figma MCP (the editor selection is the default when attached), but more open, so users can customise it. Expose everything** (owner: "Giống Figma MCP nhưng phải mở hơn cho user tự tuỳ biến nếu thích. Open tất cả."). Implementation:
   - The selection default is configurable per user and per session: selection / require node_id / page.
   - Every tool and every option is exposed, with no hidden or Figma-only restrictions.

### Round 2 (2026-10-17)
5. D-E2-1. New frames use **auto layout version 1 (CSS-aligned)**. Imported frames keep their own version.
6. D-E3-1. The owner supplied the real file `/Users/jang/Downloads/VSF - PCP.fig` (124 MB) as the reference for slots.
   - **Verify first that it contains slot data** (`isSlot=463` / `isSlotContent=495` / SLOT props).
   - If it has none, report back to the owner.
   - Do not commit this file to the repo; it is the owner's private file.
7. D-E3-3. Check designs number suggestions: **exact value matches only**. Ambiguous matches are reported without a fix.
8. D-E5-1. Custom shaders: **user-authored WGSL**.
   - The GPU renders them.
   - The CPU and headless paths use a cached GPU readback.
   - Shader nodes are excluded from the CPU=GPU bit-exact suite.

### Round 3 (2026-10-17)
9. **Colour space (D-E1-1): store and render Display P3 per document.** This includes OKLCH/OKLAB picker modes and CSS `oklch()` / `color(display-p3 …)`. sRGB files stay bit-exact.
10. **Variables JSON (D-E1-2): Figma-native and W3C DTCG design tokens.**
11. **Video encode/decode (E6 Q1 + D-E5-4):**
    - macOS: VideoToolbox, AVFoundation and AudioToolbox.
    - Windows: Media Foundation.
    - Linux: AV1 + Opus.
    - This covers MP4 export and video fills.
12. **3D transforms (D-E5-3).** Owner: "Reverse engineer Spline? Rive? và schema 126.9.13 rồi tự làm? sau đó Figma có ra thì học hỏi thêm?"
    - Do not wait for Figma GA.
    - Task 1: an REA study with `rea` (global hard rule) of the Spline and Rive desktop apps, plus the 3D fields in the Figma 126.9.13 kiwi schema.
    - Task 2: our own 3D transform design and implementation, informed by that study.
    - Task 3: a follow-up to align with Figma once its 3D reaches GA.

### Round 4 (2026-10-17)
13. **Remote-AI features: everything goes through the agent CLI** (the Studio Agent runtime, i.e. the user's own CLI).
    - Covers remove background, isolate, expand, erase, vectorize, AI summaries, the agent in FigJam/Slides, generate image and the AI "add interactions".
    - No model is bundled in the app, not even a segmentation model or a tracer.
14. **Figma-hosted services: excluded, with local equivalents.**
    - The local equivalents are a local notification inbox, an outgoing status hook, and opt-in Jira / Google Chat / Slack / VS Code add-ons.
    - Plugin payments and multiplayer APIs return "single local user, free", so plugins still run.
15. **Multiplayer-only features: async channels plus solo variants.**
    - Async channels: branch review, git merge driver, synced-folder library root, comment merge.
    - Solo variants: local vote tallies, and spotlight as presenter-follow in Present.
16. **MPL-2.0 is allowed** as an explicit exception in the X3 licence gate (`symphonia`).

### Round 5 (2026-10-17)
17. **Plugin network (D8.4): only the manifest's `allowedDomains`, with a one-time consent per plugin.**
18. **Merge conflicts (D10.2): resolved per layer.**
19. **Branches (D10.3 + D10.4): live inside the file, with a title-bar switcher and a Home branch count. A merged branch is archived, read-only and restorable.**
20. **Built-in content (E4 Q1 + E9 Q3): copy Figma's 25 built-in brushes and its templates** (the owner chose this knowingly over the IP-risk warning).
    - The coordinator adds a licence-review gate task before release. It records each asset's source and licence:
      - Figma Community files are usually CC BY 4.0, which means attribution;
      - the built-in brushes are Figma's proprietary assets.
    - It adds attribution where required, and reports any real legal blocker to the owner before shipping.
    - The gate does not reverse the decision.

### Round 6 (2026-10-17)
21. **Code Connect (D7.2): repo files win.** The document keeps a synced copy, and the UI shows "Edit in code" for mappings that come from the repo.
22. **Web capture: the "never html-to-figma" rule covers only the Figma write workflow.** Studio's own web-capture importer (E8.6, T7.14) stays in scope.
23. **React motion export (E6 Q3): exactly Figma's flavour, after an REA read.** CSS-in-JSX is the interim fallback.
24. **Board import (E9 Q4): Lucid and Mural; Jamboard is dropped** because it is discontinued.

### Round 7 (2026-10-17)
25. **New epics for areas that had none: all four.**
    - **E11 Prototyping:** video/GIF in prototypes, prototype comments, mobile view, AI "add interactions" through the agent CLI, embedding.
    - **E12 Canvas, accessibility & text:** screen reader and keyboard canvas control, enhanced contrast, a screen-wide eyedropper, video on the canvas, local and variable fonts, smart symbols, export suffix / colour profile / outline text.
    - **E13 Sites & code layers:** a local Figma Sites that exports static sites with no hosting (per ruling 14), plus code layers.
    - **E14 iPad / mobile app:** an iPad app and a mobile prototype preview.
26. **The component playground (D-E3-2) is built (E3-14).** No question was asked, because the full-scope ruling already answers it.

**All decision rounds are done.** Next: full-scope drafts for E11–E14, then the master task list with pack sequencing, then filing the issues.

### Round 8 (2026-10-17)
27. **D11.1 Prototype export renderer: our own WASM build** of `designos-render` + `designos-proto`, pixel-identical to Present.
28. **D11.2 Prototype export packaging: one self-contained `.html` file.** Above about 100 MB the user can choose a folder instead.
29. **D12.1 Contrast mode: a manual toggle that also follows the OS "Increase contrast" setting.**
30. **D12.2 Font embedding: embed when the font licence allows it** (OS/2 fsType). Fonts that forbid embedding are refused with a named reason.

Coordinator (technical): the master list for E1–E10 is `scratchpad/master-tasks.json`, with 216 tasks and 33 packs. The planner reordered the hot-file queue in 3 places to follow the pack order (merge only the current pack); this is accepted.

### Round 9 (2026-10-17)
31. **D12.3 Large video: stored inside the file and streamed, not loaded into RAM.**
32. **D13.1 Sites: their own file type in Home.**
33. **D13.2 Code layers: React/TSX only.**
34. **D13.3 JavaScript in exported sites: zero JS by default; small scripts only where an interaction needs them.** The owner confirmed this on a re-ask after a mis-click.

### Round 10 (2026-10-17)
35. **D14.1, D14.2 and D14.3 (iPad distribution, AI on iPad, native mobile platforms): deferred to backlog** ("Đưa vào backlog đã").
    - E14 is filed as an epic with all of its tasks, in pack **Later** with Status Backlog.
    - These three decisions stay open on the E14 epic issue.

**Decision rounds are complete for E1–E13.**
