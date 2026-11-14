# Q Research: can a public PolyForm Shield repo ship Untitled UI Icons?

| Field | Value |
|---|---|
| Issue | #1122 (epic #1118, spec key Q) |
| Kind | Research note (no code). Size S: answered in this note; the owner confirms one item. |
| Depends on | none |
| Unblocks | Any later issue that would replace Studio's icon set. Owner ruling R6 keeps the current icons for this revamp, so nothing in the revamp waits on this note. |
| Touches | Nothing in this note. A follow-up would touch `docs/eval-licences.md` → `THIRD_PARTY_LICENSES.md` (see Side finding 2). |

Research date: 2026-10-25. Repo baseline: `origin/main@ec47da26`. Web sources were read on that date; licence pages change, so re-read them before acting on this note.

## 1. Question

Studio's repository is public and source-available under PolyForm Shield 1.0.0 (`LICENSE:1-5`). The question is whether Studio may vendor SVGs from Untitled UI Icons into `crates/designos-app/assets/icons/` and ship them inside the release binaries. Two sets are in scope: the free set (about 1,100 line icons) and PRO (4,600+ icons in 4 styles).

## 2. What Studio ships today (FACT, read at HEAD)

- Icons are the Phosphor set, MIT, vendored in `crates/designos-app/assets/icons/phosphor/`: 111 files including `LICENSE`. They are compiled into the binary with `include_str!` (`crates/designos-app/src/icons.rs:1-4`, `:402`).
- A few icons come from Lucide, ISC, in `crates/designos-app/assets/icons/lucide/`: 104 files including `LICENSE` and `README.md`.
- Both licence texts ship in every package: `scripts/package.sh:59-60` and `scripts/package-macos.sh:93-94`.
- Both licences permit redistribution in a binary, as long as the notice is kept.
- Embedding an SVG in the binary is distribution, and so is committing it to the public repository. Every option below has to meet that bar.

## 3. Sources

The terms are paraphrased. One short quote is given, from S3, the icons' own licence file.

| # | Source | What it says (paraphrased unless quoted) |
|---|---|---|
| S1 | Untitled UI licence, https://www.untitledui.com/license | One agreement covers Untitled UI Figma, React and Icons, free and PRO versions alike. Unlimited personal and commercial projects are allowed. Not allowed:<br>- selling, sublicensing, distributing or otherwise granting rights to the files or the assets inside them<br>- publishing them online or sharing access with other users<br>- building a UI kit, library, theme, template, design system, starter kit or app/website builder from them, even modified<br>Some Untitled UI React components are MIT; icons are not named among them. |
| S2 | Untitled UI legal licence page, https://www.untitledui.com/legal/license (PRO ENTERPRISE terms change for purchases on or after June 14, 2026) | Same terms as S1. Use inside commercial products is allowed only while source files (JSX, Tailwind, Figma files) are not exposed. |
| S3 | `untitleduico/icons` repository `LICENSE`, https://github.com/untitleduico/icons/blob/main/LICENSE (added in commit `951d22e8`, 2025-07-08). GitHub reports the repository licence as `NOASSERTION`. | The free icons' own licence file. Use in personal and commercial projects is allowed. Not allowed: "Sell, sublicense, or distribute the icons (in original or modified form)". Derivative icon libraries are banned, and so is use in a kit, library or template made for resale. It points to the full agreement (S1). |
| S4 | npm `@untitledui/icons` 0.0.23, https://registry.npmjs.org/@untitledui/icons/latest (registry entry modified 2026-09-22) | `package.json` declares `"license": "MIT"`. The README's License section says the icons may be used in unlimited personal and commercial projects and links to the agreement (S1). PRO is a private npm package with 4,600+ icons in 4 styles. |
| S5 | Untitled UI FAQ, https://www.untitledui.com/faqs | Free icons: commercial use without attribution. Commercial use excludes reselling, redistributing, repackaging or repurposing, and building a competing kit, library or template. |
| S6 | Studio's own licence register, `docs/eval-licences.md:264` | Untitled UI already appears in the eval `[ban]` patterns, which list proprietary files the eval may not use. |

## 4. Findings

1. **FACT: one agreement covers both sets.** The free set and PRO are under the same Untitled UI licence agreement (S1, S2, S5). The only MIT grant on the licence pages is for "certain Untitled UI React components", and icons are not among them (S1).
2. **FACT: the npm metadata conflicts with the repository's own licence file.** `package.json` says MIT (S4), while the `LICENSE` in the same repository forbids distributing the icons, original or modified, and forbids derivative icon libraries (S3). The README sends readers to the agreement (S4).
3. **INFERENCE: the restrictive text governs.** It is the human-readable licence and it is more specific. `package.json` is metadata that GitHub itself does not treat as a licence grant (`NOASSERTION`). Relying on a `license` field that contradicts the licence text is not a defensible position.
4. **FACT: both shipping routes count as distribution.**
   - The agreement forbids distributing or publishing the files or the assets inside them (S1).
   - Committing SVGs to a public repository publishes them.
   - Compiling them into a released binary distributes them.
   - The SVG text can be recovered from the binary, because `include_str!` stores it verbatim.
5. **INFERENCE: Studio may fall under the "builder" and "derivative library" bans.** Studio is a design tool, which S1 and S2 treat as a builder, and an icon set inside it can be read as a derivative icon library (S3). This holds even though Studio would not sell the icons.
6. **FACT: PRO is stricter, not looser.** PRO icons come from a private npm package for paying customers (S4 README). The paid licence forbids sharing access with other users (S1). A public repo shares access with everyone.

## 5. Verdict options

| Option | What Studio does | Risk | Notes |
|---|---|---|---|
| **A. Keep Phosphor and Lucide and restyle them (recommended)** | No Untitled icon files anywhere in the repo or the binaries. Stroke weight and colour follow the Untitled `fg-*` roles (R6). | None from Untitled. Phosphor (MIT) and Lucide (ISC) notices already ship. | This is exactly owner ruling R6. Visual distance from Untitled stays small: all three are 1.5–2 px line sets on a 24 grid (INFERENCE from the sets' published styles; not measured). |
| B. Vendor the free Untitled set | Commit SVGs and embed them. | **High.** Contradicts S1, S3 and S5 (distribution) and possibly S3's derivative-library clause. The MIT `package.json` field (S4) is the only counter-evidence, and S3 contradicts it. | Not acceptable without written permission from Untitled UI. |
| C. Vendor the PRO set | Same as B with paid files. | **Highest.** It also exposes paid files publicly (S1, S2). | Not acceptable. |
| D. Ask Untitled UI for a written grant | Email the address the licence page names (hello@untitledui.com) and ask for an explicit licence to embed the free icons in a public, source-available desktop app. | Low if granted in writing. Until then, B's risk. | Only worth it if the owner wants the Untitled icon look badly enough to wait. |
| E. Load icons at runtime from the user's own Untitled kit | Studio ships no icons. A user who owns the kit points Studio at it. | Low legal risk; high product cost: two icon paths and a fallback when no kit is present. | Out of proportion for chrome icons. Listed for completeness. |

## 6. Recommended answer

**Option A.** Studio keeps the Phosphor and Lucide sets, already licensed and shipped, and restyles them to the Untitled foreground roles, as R6 rules. No Untitled UI icon file enters the repository or a package. If the owner later wants Untitled's own icons, take Option D first and wait for written permission. Do not rely on the MIT field in the npm metadata.

## 7. What the owner must confirm

1. **(Recommended: yes)** Close the icon question with Option A for this revamp and after it. Untitled icons come in only with a written grant from Untitled UI (Option D).
2. **(Wider question, raised here because the same clauses apply; not blocking Q.)** The agreement also bars building a UI kit, library, theme, template, design system or starter kit with Untitled UI (S1, paraphrased). Specs A, B and C commit Untitled colour values, scale numbers and component anatomy into a public repo (`theme.rs`, `design/design.tokens.json`).
   - Hex values, spacing numbers and anatomy rules are facts and design choices rather than Untitled's files. No Figma file, SVG or source file is copied, and the plan already keeps the raw kit export out of the repo (plan.md, "Raw export is NOT committed").
   - Studio is also not a kit or theme offered for resale.
   - **Recommendation: proceed, but name nothing "Untitled UI" in shipped product text or token names beyond attribution, and commit no kit export.** If the owner wants certainty, the same email as Option D can ask about this too.
   - This is a legal-risk judgement for the owner, not a technical one.

## Side findings (not part of the question)

1. Studio's licence register already treats Untitled UI as proprietary: it is in the eval `[ban]` patterns (`docs/eval-licences.md:264`). That is consistent with this verdict.
2. `THIRD_PARTY_LICENSES.md` is generated from `docs/eval-licences.md` (header lines 3-4). It lists the vendored skills and Inter, but not Phosphor, Lucide or Roboto Mono. The packages still ship those notices (Section 2), so this is a gap in the register, not a compliance failure. Follow-up, for the PM: add register entries for Phosphor (MIT), Lucide (ISC) and Roboto Mono (OFL) so the generated file matches what the packages ship.

## Decisions on the open questions (writer, 2026-10-25)

1. **Is the npm `MIT` field enough to treat the free set as MIT?** No. The licence file in the same repository and the agreement it links both forbid distribution (findings 2 and 3). An agent must not cite the npm field as permission.
2. **Does embedding in a binary avoid "distribution"?** No. Release packages are distributed, and `include_str!` keeps the SVG text intact (finding 4).
