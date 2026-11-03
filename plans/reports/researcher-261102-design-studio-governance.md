# design-studio (itspatmorgan, MIT, v0.1.3): knowledge / governance / extensibility analysis

Clone: `/private/tmp/claude-501/-Users-jang-Products-design-os-studio/f1428fa3-d2ec-4abe-b46d-db13f0015fd5/scratchpad/ext-design-studio` (HEAD 531e181, 1 commit visible, no git tags). Read-only; nothing executed.
All paths below are relative to `src/platform/` unless prefixed (`AGENTS.md`, `.agents/`, `.github/`, `src/systems/`, `studio.config.json` are repo-root).

## NOT verified (read first)
- Nothing was run. No tests, no `pnpm studio sync`, no host (Claude Code/Codex/Cursor) was launched. "Works in each host" is NOT verified; the repo itself says so (`context/agent-context.md:63`, `:45` "Cursor ... actual selector behavior must be checked").
- `.claude/skills/*` symlink behavior on Windows: INFERENCE only (agent-skills.js:176 `fs.symlinkSync(..., 'dir')`; Windows needs privilege; repo says nothing).
- Plugin repo `design-studio-plugin` (setup/publish entry points) NOT read (`context/agent-context.md:53`).
- Not read in depth: file-types.md, publishing.md, releasing.md, onboarding module, `app/`, Vite plugins, browser side. Topic 9 is therefore partial.
- Whether prose-level guidance actually changes agent behavior: no eval exists in the repo (the repo admits automated checks do not prove live behavior, `context/agent-context.md:63`, `context/documentation-standards.md:31`).

## Epistemic ledger
- FACT: everything cited with file:line was seen this session (line numbers from `cat -n` / `grep -n`).
- INFERENCE: labeled inline.
- Weakest link of the whole report: the repo is a pre-release (0.1.x, "pre-release: no compatibility gate", `context/modules.md:67`), so "good" here = good design, not proven at scale.

## Ranking: what is worth stealing (highest value first)
1. Literal-only, duplicate-key-rejecting declaration reader + before-side authority evaluation (T9). Cheap, portable, rare.
2. Hash-receipt sync of generated host adapters (T2). Directly applicable to a multi-host plugin bundle.
3. Generated routing block from per-module `instructions:[{path,when}]` (T1). Applicable, with the drift caveat.
4. Explicit declarations + "validation names the missing field" (T3). Applicable as a lint rule, not as a doctrine.
5. Permanent opaque IDs (T5). Right idea, heavy implementation for a text-file world; in a native app with a document model it is cheaper.
6. Agent preflight command `studio context ... --json` (T7/T9). Small, high leverage.
7. Ownership map (T6): weakest. Documentation only, no enforcement (see T6).

---

## T1. Generated AGENTS.md from canonical sources + module routing block

**Mechanism (FACT)**
- Output = header + platform text + routing block + team text. `core/declarations.ts:53-57` `composeAgents`; header at `:48-49`; sources `node/lib/agents-file.js:7-8`, assembly `:11-21`.
- Routing block: `core/declarations.ts:11-22` `agentsBlock`: for each *enabled* module's `instructions` entry that has `when`, emit `When the person ${when}, read [link](link).`; a path ending `/` means skill folder, link becomes `.../SKILL.md` (`:17`). Markers `<!-- studio:modules -->` (`:5-6`); live example `AGENTS.md:23-34`.
- Platform text is written in `src/platform/` so links are rebased to repo root (`core/declarations.ts:33-46` `rebaseLinks`, applied `:54`). This is what lets upstream own a file that is relocated in the output.
- Team part is a separate file never overwritten: `AGENTS.team.md:3`, `context/ownership.md:48`.
- `CLAUDE.md` is just `@AGENTS.md` (`CLAUDE.md:1`), created only if missing (`node/lib/agent-skills.js:182`).
- Regenerated on: `pnpm dev` (`package.json:10`), enable/disable/create/remove (`node/cli/studio-runtime.js:199,280,416,500,559`), skill file changes in the Vite watcher (`node/build/vite-manifest-watch-plugin.js:97`; note it syncs skills only, not AGENTS.md; INFERENCE from that one call site). CI fails if stale: `.github/workflows/checks.yml:77` `pnpm studio sync --check`; implementation `node/cli/studio-runtime.js:131-139` (throws "AGENTS.md is stale").
- Schema guard on routing entries: `core/modules/index.ts:117` path must be under `context/` or `skills/`; `:185-197` validation; type at `:69-71`.

**Why good**
- One routing sentence form ("When the person asks X, read Y") is a trigger the agent can match without loading the skill; module off => line disappears (`core/declarations.ts:8-10`), so the agent is never routed to an absent capability. That kills a whole class of "agent tries a disabled feature" bugs.
- Generated file is stamped, committed, and CI-checked: the single most robust way to keep an always-loaded instruction file honest.
- Three-way split (platform / generated block / team) gives clean ownership without merge-engine (see T6).

**Weak spots**
- Two routing channels that can drift: `when` text in `module.ts` and `description:` in SKILL.md frontmatter (which hosts use for their own discovery). Real example: `modules/canvas/module.ts:15` ("a page of views, documents, and notes arranged together") vs `modules/canvas/skills/use-canvas/SKILL.md:3` ("views, diagrams, documents, notes, and arrows. Use only when Canvas is enabled"). No check compares them. INFERENCE: they will diverge.
- Entries without `when` are validated but never routed (`core/declarations.ts:14`); silent no-op. Easy to author a skill nobody is told about except via the host's description match.
- Cost is linear in module count in an always-loaded file (10 lines now, fine; INFERENCE: breaks at ~50 modules).
- The hand-written platform paragraph is dense (`AGENTS.md:7`, one ~120-word paragraph of conditionals), which cuts against the repo's own principle (T8).
- Generated, committed file => upgrade merges conflict on it (INFERENCE; ownership.md says to run sync after merging, `:29`).

**Portability**: High. In DESIGN:OS: generate per-host entry files (AGENTS.md / CLAUDE.md / plugin manifest) from one source and the enabled-feature set; make `when` and skill description a single field to remove the drift.

---

## T2. Skill sync to multiple hosts with a hashed receipt

**Mechanism (FACT)** `node/lib/agent-skills.js`
- Canonical skills stay in owner folders (platform, module, system). `knowledgeOwners` `:14-25`, `skillCatalog` `:27-58`. Exposed name is `studio-<kind>-<owner>-<skill>`, lowercase/hyphen, <=64 chars (`:45-48`) so names are collision-free across owners.
- Generated entry is a *stub with no independent procedure*: frontmatter name+description, then "Read this repository's AGENTS.md ... Follow the canonical skill (relative link)" plus owner-specific guard lines (system skill = only for assigned prototypes; module skill = confirm module enabled) `:94-105`.
- Two targets from one source: `.agents/skills/<name>/SKILL.md` (Codex/Cursor) as file, `.claude/skills/<name>` as relative symlink to it (`:108-109`; claim in `context/agent-context.md:41-43`).
- Receipt `.agents/studio-skills.json`: `{schema:1, files:{path:{kind,hash}}}` (live sample `.agents/studio-skills.json:1-8`). Hash of file content or of link target (`:59,111-117`).
- Reconcile rules: existing path whose content != desired and whose hash != receipt => "Preserved user-owned or modified adapter" (`:131-139`); skill folder exists but not in receipt => preserved (`:140-144`); Claude link skipped when its file adapter collided (`:121-125`); receipt entries no longer desired are removed only if hash still matches, else "Preserved modified obsolete adapter" (`:148-161`). Only paths matching `studio-*` are ever managed (`:118-119`), invalid receipt entries throw (`:149-151`).
- All checks complete before any write (`:166`); `--check` mode returns a change count without writing (`:165`; test `node/lib/agent-skills.test.js:94-100`). Path-safety: symlinked parents / escaping dirs refused (`:60-68,81-86`; `node/lib/safe-paths.js`).
- Tests cover collision, user modification, removal on disable (`agent-skills.test.js:51,70,88,94`).

**Why good**
- Ownership by receipt+hash, not by naming alone: this is `dpkg conffiles` logic applied to agent skills. The user can hand-edit or shadow an adapter and sync never clobbers it, yet stale generated files do get garbage-collected.
- Stub design = zero duplication of procedures; one edit propagates to all hosts; hosts that only read frontmatter still discover it.
- Prefix namespace + length cap prevents the usual cross-pack name collision.

**Weak spots**
- Stub body is only a pointer: every skill use costs one extra read hop; host-specific frontmatter (allowed-tools, model, hooks) is not propagated (`:105` emits only name/description). For richer hosts this is lossy. INFERENCE.
- Receipt is hash-only: it detects modification but cannot say *what* changed; fine for stubs, weak for anything larger.
- Receipt `adopts` a pre-existing file that happens to equal the desired content with no receipt entry (`:131-139` falls through when `before === spec.content`). Harmless, but ownership claim is implicit.
- Symlink requirement (Windows) unverified. No Cursor-specific path (`context/agent-context.md:45`, relies on Cursor reading `.claude`/`.agents`).
- Two-writer problem: Vite watcher and CLI both call it; mitigated by atomic-ish ordering only, no lock (`.studio-system-operation` sentinel exists for system ops, `vite-manifest-watch-plugin.js:90`). INFERENCE.

**Portability**: Very high; this is the most directly reusable piece for a multi-host plugin bundle (install into Claude plugin, Codex, Cursor, Grok from one canonical skill dir). Keep: stub+canonical, prefix, receipt+hash, `--check`. Add: propagate host-specific frontmatter; store the generator version in the receipt.

---

## T3. "Declare what the system provides"

**Mechanism (FACT)**
- Doctrine: `context/principles.md:33-37` ("Do not hide a setting or activate behavior through omission ... Validation must identify missing required declarations rather than silently infer a default"). Restated for agents at `context/working-in-studio.md:12`.
- Enforcement in code, not just prose:
  - Module: `requires`, `optional`, `lib` must be present booleans/strings, error says what to add (`core/modules/index.ts:135-142,145,151`). Type comment `:36-38`.
  - Config: every installed module must be listed true/false (`core/config.ts:102,119`); unknown keys rejected; activation only on explicit `true` (`core/config.ts:149-150` `isEnabled`); `defaultSystem` required (`:96`); `systemMaintainers.<id>` must exist even if `[]` (`:187-192`). Live sample `studio.config.json:9-33` (explicit `"systemMaintainers": {"...": []}`, `"admins": []`).
  - Compile-time guard that `CONFIG_KEYS` lists every `StudioConfig` key (`core/config.ts:51`); JSON Schema kept in agreement by test (header `core/config.ts:3-5`).
  - Prototype: `systemId` must be present, `null` is a legal explicit value; default only at creation (`node/lib/prototype-assignment.js:12-15`); obsolete field names rejected (`:16-17`); explicit `origin: 'shadcn' | null` and `libraries: []` "use [] when empty" (`modules/systems/spec.ts:96-98`).
  - Preferences: every field required, defaults explicit, error "declare X as type" (`core/modules/index.ts:19-27`).
  - Messages are fix-oriented sentences ("add requires ... like requires: '0.1.3'", `:136-138`).
- Scaffolds write the defaults visibly (`context/principles.md:37`).

**Why good**
- "Absent" and "false/null" become distinct states, so audits can distinguish "decided" from "forgot". Optional/removable is a declared property, not inferred, which is what lets the removal check (T4) be mechanical.
- Messages that name the next action are good agent UX: a model can fix them without reading docs.

**Weak spots**
- Verbosity tax and noise: required empty arrays/objects everywhere (`studio.config.json:25-31`, `"admins": []`); each new required field is a breaking change for all teams (admitted: "introduce new required fields through explicit, reviewable configuration changes", `principles.md:37`) and there is no migration engine (`context/formats.md`, "explicit migration").
- Inconsistently applied: `compatible()` fails OPEN when `requires` is missing or malformed (`core/modules/index.ts:306-307`), contradicting the doctrine; it relies on `moduleProblems` having already flagged it. INFERENCE: ordering dependency between two functions.
- Two sources of truth for config shape (TS type + JSON schema + `configProblems`) held in sync by a test (`core/config.ts:3-5`), i.e. 3 artifacts per field.
- Doctrine bleeds into non-behavioral places (themes: omitted tokens are "intentionally outside the system", `principles.md:35`), raising authoring cost for designers, the stated user.

**Portability**: High as a lint rule ("no field has a silent default; validators return the fix"). For a native app: serialize enabled-features and per-document design-system assignment explicitly, keep nullable-vs-missing distinct in the schema.

---

## T4. Module system

**Mechanism (FACT)**
- A module = folder with `module.ts`; no separate registry (`context/modules.md:5`). Two roots: supplied `src/platform/modules/`, team `src/modules/` (`core/roots.ts`, `SUPPLIED_MODULES_ROOT`/`TEAM_MODULES_ROOT`; `context/modules.md:7`); ID = folder name, must be unique across both (`core/modules/index.ts:251-260` `moduleRootProblems`).
- Declaration fields: `requires` (min platform version), `optional`, `lib`, `section`, `hosts`, `preferences`, `instructions`, `dependencies`, `optimizeDependencies<=32` (`core/modules/index.ts:29-74`). Real examples: canvas `modules/canvas/module.ts:4-19`, required `systems/module.ts:4-17` (`optional:false`), contributors `modules/contributors/module.ts`.
- Compatibility: `PLATFORM_VERSION = '0.1.3'` constant (`core/modules/index.ts:11`); incompatible module is *disabled and reported*, not crashed (`:306-311`, `context/modules.md:25`). package.json has no `version` field (grep), so the constant is the only version of record; git tags are said to identify the release (`context/ownership.md:36`) but the clone has no tags.
- Optional vs required: `studio.config.json` may only turn off `optional:true` modules (`core/config.ts:115`); team mode requires `contributors` (`:90`).
- Removal safety is a checked invariant: "nothing outside an optional module imports its files other than module.ts, and no module imports an optional one, so deleting its folder leaves nothing broken" (`node/check/check-modules.js:16-17`); release suite builds with all optional modules disabled, then physically removed (`context/modules.md:155`; `node/lib/module-removal.integration.test.js:14,45,66,100`).
- Public API: team modules/systems import platform only via `#studio/<entry>` mapped by package.json `imports` to `src/platform/api/*.ts` (`context/modules.md:29-43`). Inventory + per-entry executable environment (application/node/runtime) in one table `core/modules/boundaries.ts:12-24`; violation text `:40-48`; node/browser graph checking `node/lib/api-environments.js`, scope rules `node/lib/scope.js:25-156` (prototype can import only declared system libraries, `:131-139`).
- Per-module extension points by file convention: `app.tsx`, `type.ts`/`open.tsx`/`loader.ts`, `server.ts` (dev routes `POST /__studio/<module>/<route>`), `check.ts`, `build.ts` (`core/modules/index.ts:76-112`; `context/modules.md:91-104`).
- Install model: modules are *copied into the repo*, "not loaded as plugins at runtime"; no provenance lock (`context/modules.md:9,145`); installed declarations are trusted executable code (`node/lib/installed-module-declarations.js:1`; `skills/manage-modules/SKILL.md` "Treat installed modules as trusted code").

**Why good**
- Capability removal tested as a release gate (disabled and physically removed) is the strongest evidence of real modularity; most plugin systems only test "enabled".
- One declaration drives build, dev server, app, agent routing, settings listing, check — "all read one list" (`core/modules/index.ts:1-4`).
- Public API table with environment (browser/node/runtime) policy prevents the classic "node import in browser bundle" class of errors and is the single source for checks and Vite guard (`context/modules.md:51,59`).
- Fail-soft compat (disable + report) is the right default for a user-owned repo.

**Weak spots**
- No SemVer range: `requires` is a single minimum; no upper bound or API-level feature negotiation. 0.x, no compat snapshot (`context/modules.md:67,89`); breaking `#studio/*` changes will silently strand team modules until `pnpm check`. 
- `PLATFORM_VERSION` hand-bumped constant, not derived from package/tag; drift risk (INFERENCE; no test seen asserting equality with a release).
- Hidden global state: module folders live in a mutable registry set at startup (`core/roots.ts:58-59` `setModuleFolders`) and an unregistered module silently defaults to the team root.
- Trust: installed modules run with repo privileges; checks are "source contracts, not a security sandbox" (`context/modules.md:63`). Fine for local-first, hostile to a marketplace.
- Hand-rolled import-graph checker (`node/lib/imports.js`, `api-environments.js`, ~9.4k LOC across `node/lib`) is a large maintenance surface; "literal import paths required so checks can resolve" (`context/modules.md:71`) restricts authors.

**Portability**: Medium-high conceptually (declaration, requires, optional, removal-test, public-API table). In Rust: crate features/trait registries + a "disable and remove" CI matrix are the equivalent; the import-graph machinery maps to cargo-deny/`pub(crate)` + workspace layering, nearly free.

---

## T5. Permanent resource identity

**Mechanism (FACT)**
- Format: 16 chars of Crockford-style base32 (alphabet without `i l o u`) from 10 random bytes = 80 bits, no time/name/content (`core/resourceIdentity.ts:3,14-31`; spec `context/resource-identity.md:27`). Branded type `ResourceId` + strict regex (`:5-12`).
- Applies to prototype, artifact, system, contributor; *not* to components/docs/skills which keep path identity (`context/resource-identity.md:23`) — deliberate scoping.
- Stored where the format allows: `meta.json` `studioId`, `system.ts`, Markdown frontmatter, `/** @studio-id */` leading comment, `%% @studio-id` in Mermaid, top-level in canvas JSON (`context/resource-identity.md:33-41`) via per-file-type adapter `IdentityMetadata {read, write}` (`core/resourceIdentity.ts:56-102`).
- Duplicate-key trap handled: `JSON.parse` last-wins is rejected by a token scan (`:104-123`).
- URLs use IDs only, no name aliases (`context/resource-identity.md:55-61,87`); inventory resolves id -> current path, "unknown identities must not become filesystem paths" (`:67`; `core/resourceIdentity.ts:33-34`).
- Reads/builds/saves never mint IDs; creation/duplication do; copy-with-retained-ID fails validation (`context/resource-identity.md:13-15,113`). Cross-prototype artifact move is rejected until a policy exists (`:51`).
- Relationships by ID: `ownerContributorId`, `systemId`, `rebuild.sourcePrototypeId/targetSystemId`, `archivedBySystemId`, grants in config (`:91-93`; `node/lib/prototype-assignment.js:16-48`). Deleted system leaves a `systemMissing{systemId,label}` tombstone so references survive (`:20-33`).
- CI compares identity inventories before vs after (`node/check/check-scope.js:30-41`, `node/lib/resource-identity-changes.js`); `pnpm studio identity-audit` / `identify` preview-then-`--yes` (`context/resource-identity.md:101,111`).

**Why good**
- Rename/move cannot break links, grants, or assignments; authority references survive folder renames ("Filesystem movement does not silently grant ownership", `:91`).
- Choosing opaque random over derived ID avoids content/time coupling; 80 bits makes collisions ignorable without a central counter.
- Adapters per file type keep the identity living *inside* the artifact (survives git mv, copy-out) instead of a side database.
- "Dependency integrity is separate from identity lookup" (`:49`) avoids the common conflation.

**Weak spots**
- Heavy for what it buys: the identity layer is ~1.5k words of spec + ~2.2k LOC of identity/reference code (wc) + 68 test files platform-wide, for a pre-release prototype sandbox. INFERENCE: over-engineered for solo use; justified only by team grants.
- URLs are unreadable and have "no compatibility aliases" (`:87`), so share-links lose meaning and any pre-release link dies.
- Identity in comments/frontmatter is fragile to naive copy/paste and to formatters; protection relies on duplicate detection (`:29,113`), not prevention.
- Forks retain IDs (`:119`), so two diverged repos collide on merge; the answer is "preserve the original and convert separately" (`:105`).
- Hand-parsed frontmatter regex (`core/resourceIdentity.ts:68-100`) instead of a YAML parser: edge-case risk (INFERENCE).
- Explicit statement: "Identity is not authentication or access control" (`:29,95`).

**Portability**: High and cheaper in Rust. A native app already has a Document model: use the same 80-bit random ID per node/resource, keep tombstones for deleted referents, reject duplicate IDs on import, never mint on read. Skip the per-format comment adapters unless you round-trip foreign text formats.

---

## T6. Ownership tiers + path map + manual Git upgrade

**Mechanism (FACT)**
- Tiers (`context/ownership.md:9-11`): upstream platform / team content+settings / generated / starter examples; labelled "guidance, not an automated merge policy".
- Path map table (`:13-20`): `src/platform/` + root build config + `.github/` + `.husky/` + `patches/` = supplied platform; `src/systems/ src/modules/ src/prototypes/ contributors/ studio.config.json README.md AGENTS.team.md` = team; `AGENTS.md` + skill adapters = generated; `pnpm-lock.yaml` generated.
- Path layout *is* the ownership boundary: dual roots for modules and systems (`core/roots.ts:13-14,SUPPLIED_*`; `modules/systems/spec.ts:157-177` forces the supplied systems root to hold only the platform system; supplied module IDs cannot be reused by team modules).
- Upgrade = 5 manual Git steps: branch, merge exact tag, review ALL changes incl. clean merges, resolve, `pnpm studio sync` + test + build, review before merging (`:22-30`). Explicit: "There is no `studio update` command or special seed-preservation merge engine" (`:24`).
- Admission: "there is no machine-readable ownership map or path-coverage check" (`context/checks.md:31`).
- CI scope check enforces *who may touch what* on a different axis (`.github/workflows/checks.yml:38-51`; see T9).

**Why good**
- Honest, minimal: use Git as the merge engine; put the boundary in the directory tree so a diff of `src/platform/` is the entire upstream surface. Cheap to maintain, hard to get wrong.
- Review-all-merge-results instruction (`:27`) is a genuine, non-obvious catch: clean merge != correct merge when users deleted sample files.

**Weak spots**
- Not enforced: no check that a local edit under `src/platform/` is flagged on upgrade; the table is prose, and `checks.md:31` says so. The `.husky`/`.github`/root-config files are listed as "upstream" but live outside `src/platform/`, so path-prefix tooling can't capture them (INFERENCE).
- Mixed paths admitted (`:50-52` root `package.json`, lockfile); the most common conflict sites have no policy beyond "review together".
- Manual merge of a 21k-word context tree plus a 9k-LOC node layer by a designer/PM (the target user) is the opposite of the persona's stated need (`context/personas.md`); in practice the agent does it, with no safety net beyond tests. INFERENCE.
- No tags in clone; "Git history and release tags identify the source revision" (`:36`) cannot be checked here.

**Portability**: Low-medium. For a native app + plugin bundle the equivalent is: plugin bundle is replaced wholesale on update (no user edits inside), user content lives in a separate store. The tier vocabulary (platform / team / generated) is still a good doc vocabulary. Do not copy "merge upstream into your fork" for a desktop app.

---

## T7. Design system as an agent-readable package

**Mechanism (FACT)**
- A system = folder `src/systems/<id>/` with `system.ts` + `components/` + `styles/theme.css` + `context/` + `skills/` + `AGENTS.md` + `components.json` + `intro.tsx` + `package.json` + optional `libraries/` (listing `src/systems/product/`; `node/check/check-modules.js:14` validates system.ts/components/theme.css).
- `SystemSpec`: `studioId`, `role (platform|prototype)`, `status (active|archived)`, `themeClass`, `styling`, `colorModes`, `docs: warn|strict|off`, `origin: shadcn|null`, `libraries[]` with `kind: package|source`, `entry`, `assets`, `components` (`modules/systems/spec.ts:37-55`; example `src/systems/product/system.ts:6-76`). Validated sentence-by-sentence (`spec.ts:63-150`); theme scope isolation: each system's CSS may set values only under its `themeClass`, reserved names guarded (`spec.ts:43-45,181-195`).
- Library inventory is an agent affordance: prototypes may import only a *declared public library entry* of their assigned system (`node/lib/scope.js:131-139`, advice text `:16-23`).
- Per-system `AGENTS.md` is the entry; only applies to prototypes assigned to it (`AGENTS.md:7`; `src/systems/marketing/AGENTS.md:3-7` links `context/brand.md`, `design.md`, `library.md` and states an import restriction).
- `components.json` here is the stock shadcn registry config (`src/systems/product/components.json:1-20`, `$schema` ui.shadcn.com), not a Studio-specific format; the Studio-specific machine contract is `system.ts`.
- Assignment: `meta.json systemId` = permanent ID or `null`; missing = invalid (`context/agent-context.md:23-27`; `node/lib/prototype-assignment.js:12-15,19`); default written at creation only.
- Preflight CLI: `pnpm studio context src/prototypes/<contributor>/<proto> --json` returns identity, scope, `canEdit`, explicit assignment, system entry path, rebuild target, enabled modules, guidance paths (`node/lib/prototype-context.js:13-83`); it deliberately "report[s] facts and entry paths without reading a transitive instruction bundle ... or deciding which task" (`:11-12`).
- Skills in a system are scoped to the system: stub text tells the agent to use them "only for ... prototypes assigned to system X" (`node/lib/agent-skills.js:97-100`).
- Rebuild flow: a duplicate carries `rebuild{sourcePrototypeId,targetSystemId}`; the old assignment stays the runtime boundary until migration completes (`context/agent-context.md:29`; `prototype-assignment.js:37-49`).

**Why good**
- The agent never has to guess "which design system": it is an explicit ID or null, resolved by a script that also returns the entry file path. Removes the single biggest source of design-system confusion in multi-system repos.
- Machine boundary + prose boundary agree: the same declared library list drives both the import check and the docs.
- Tombstone (`systemMissing`) when a system is deleted keeps prototypes loadable and states what to rebuild.

**Weak spots**
- The starter ships the *shape*, not content: `src/systems/product/context/` and `skills/` are empty and its `AGENTS.md` is a placeholder ("Empty sections do not require invented personas", `src/systems/product/AGENTS.md:5`); marketing has 3 context docs and zero skills (listing). So the agent-readable package is mostly unproven in the shipped examples.
- shadcn/Tailwind/Base-UI/Lucide coupling baked into `SystemSpec` (`origin: 'shadcn' | null`, `styling: tailwind|custom`, `spec.ts:47,53`). A system that is not React/shadcn is a second-class case. Token inventory is not a machine-readable file here (theme.css under a scope class), so "tokens for the agent" is CSS parsing.
- System skills are "applicable only to assigned prototypes" by prose guard in the stub, not by host mechanism; a host that auto-loads all skills can still pick the wrong one (INFERENCE; `agent-context.md:35` concedes "Exposing a system skill does not make it applicable to every prototype").
- `docs: warn|strict|off` (`spec.ts:22`) defaults to `warn` in the shipped system: undocumented components pass the build.

**Portability**: High. A design-system package that declares: ID, supported modes, public library entries, tokens, context, skills, entry AGENTS.md; and a `context --json` preflight tool exposed over MCP. This maps almost 1:1 to a DESIGN:OS "system pack" + `get_document_context` MCP tool. Prefer a machine-readable token file over CSS-class scoping.

---

## T8. Minimum-sufficient guidance / verification

**Mechanism (FACT)**
- Principle: "Give people and agents the guidance they need ... Each instruction should support a meaningful decision or prevent a concrete failure ... Put repeatable mechanics in code. Use guidance to explain intent, choices, and recovery" (`context/principles.md:21-25`).
- Authoring rules: "Skills hold judgment ... Reuse existing commands for mechanics ... Do not add a rule merely to memorialize one past failure" (`context/documentation-standards.md:23-25`); one canonical source per subject, link not repeat (`:13`); verify a skill by walking a matching and a non-matching request (`:31`).
- Verification: choose checks by changed behavior; do not rerun unrelated tests; small suite for "critical infrastructure, data integrity, stable behavioral contracts, consequential regressions"; use synthetic fixtures not sample content; tiered `test` vs `test:release` with `.integration.test.js` only in release; "An empty test inventory fails verification"; tests discovered by directory, "No catalog or change-selection rules need maintenance" (`context/checks.md:25,37-41`; table `:9-19`).
- Agent-facing form: "Reuse unchanged instructions already read; read source before editing. Save verbose check output to a temporary log and report results and warnings concisely" (`AGENTS.md:17`); "Repeat passed checks only when new changes or unresolved failures justify it" (`context/working-in-studio.md:14`).
- Honest limits stated: "Automated checks validate structure; they do not establish clear prose or correct live agent behavior" (`documentation-standards.md:31`).

**Why good**
- Mechanics-in-code / judgment-in-prose is the right split, and the discovery-by-directory test rule removes a classic registry drift.
- Verification policy is written for an agent that is tempted to rerun everything; it also tells it to compress output.
- Release-tier lifecycle tests (setup, module removal) keep expensive checks where they matter.

**Weak spots**
- Self-contradiction: 21,392 words in `context/*.md` (wc) and individual docs of 1.5-2.3k words (`resource-identity.md` 1,527; `modules.md` 2,350; `file-types.md` 2,178) is not minimal; much of the prose records implementation decisions (e.g., `context/resource-identity.md:115` regression-coverage inventory), which `documentation-standards.md:19` says to keep in "maintainer records".
- Some SKILL.md files are long conditional paragraphs (e.g., `modules/systems/skills/setup-design-system/SKILL.md:16`, ~120 words in one line), hard for a weaker model to follow.
- No measurement of "sufficiency": no agent eval, no token budget per entry. The stated test ("walk through a matching request and a similar one outside scope") is manual.
- `pre-commit` is the only hook and "never blocks" for scope (`node/check/check-scope.js` header `:1-4`); verification relies on CI.

**Portability**: High for the doctrine (mechanics in code, judgment in guidance; one owner per fact; directory-discovered tests; separate release tier). Keep the doctrine; do not copy the volume. Add the missing piece they lack: a per-skill eval/trace.

---

## T9. Other clever bits

1. **Literal-only declaration reader** (`core/declarations.ts:59-63,63-193`): parses `export default {...}` from `module.ts` / `system.ts` as pure data (strings/numbers/bools/null/arrays/objects; `satisfies`/`as const` tolerated; calls, identifiers, `${}` templates refused, `:178-180,117`; `__proto__`/`constructor`/`prototype` keys refused `:147`; duplicate keys refused `:150`). Reason: reviewing an untrusted pack must not execute it; CI reads the *before-side* declarations this way (`check-scope.js` uses `readDeclaration` on `git show base:...`). Weak: ~130 lines of hand-written TS-subset parser; comment stripping is character-based (INFERENCE: edge cases on exotic strings); installed declarations are separately `require`d as trusted code (`installed-module-declarations.js:1,24`), so there are two readers with different trust.
2. **Before-side authority**: scope check evaluates the *base* revision's config, profiles, maintainers and module declarations so a PR cannot grant itself rights (`check-scope.js:53-54,59-60,90-91`; `context/contributor-scope.md:20`; `context/checks.md:69`). `canPerform(config, actor, contributors, resource, action)` is a single pure matrix (`core/permissions.ts:61-79`). Roles: Contributor (own folder), system maintainer (per-system grant, not a global role), Admin (`contributor-scope.md:8-14`). Platform changes allowed only if GitHub collaborator role is admin/maintain (`.github/workflows/checks.yml:49`; `check-scope.js:192`). Honest scope: "These rules guide people and agents. They are not filesystem authentication or GitHub permissions" (`contributor-scope.md:22`). Weak: on PRs scope is `--review` (informational, `checks.yml:47`; exit 1 only on push without maintainer, `check-scope.js:192`); enforcement therefore depends on branch protection set by the admin (`checks.md:73`); `usage: personal` skips everything (`check-scope.js:55-58`), and the "personal" bit is itself a config key read from base, fine, but a repo that starts personal has zero team protection until flipped.
3. **Plan/apply with before/after for retained data** (contributor preferences): `pnpm studio preferences --plan file` then `--apply file`, rechecks authority, declarations and every source before write; updates take an opaque source `version` and are rejected when stale (`context/contributor-preferences.md` §Node transactions/§Explicit initialization). Same preview-then-`--yes` convention for create/remove/identify (`context/modules.md:131`). Weak: many ceremony commands for what are tiny JSON edits.
4. **Format versions only on user-owned files**, with stated reasons per format for not versioning others (`context/formats.md:7-24`). Good discipline.
5. **Skill format conformity**: validates frontmatter against agentskills.io limits (name<=64, description<=1024, folder==name; `modules/systems/content/skills.ts:1-62`), invoked inside sync so malformed skills block exposure (`agent-skills.js:42-44`).
6. **Plugin kept out of the product repo**: setup plugin lives in a separate repo, starter has no plugin files, and the project works without it (`context/agent-context.md:49-55`). Good separation of install-time from run-time.
7. **Instruction-link validation tolerates removed modules**: links into a removed owner are excluded, missing files inside an installed (even disabled) owner are errors (`context/modules.md:15`).

**Portability**: 1, 2, 3 survive well. 1 is the most transferable trick (data-only manifests for third-party packs; in Rust parse TOML/JSON, no code).

---

## Trade-off matrix (adoption value for DESIGN:OS Studio: Rust app + MCP + multi-host plugin bundle)

| Mechanism | Value | Cost to adopt | Risk | Verdict |
|---|---|---|---|---|
| Receipt+hash adapter sync (T2) | High | Low (~200 LOC equivalent) | Windows symlink, lossy stubs | Adopt |
| Generated routing block from module declarations (T1) | High | Low | `when` vs description drift | Adopt, single-field `when` |
| Data-only pack manifests + duplicate-key reject (T9.1) | High | Low | none | Adopt |
| Before-side authority in CI (T9.2) | Medium (team/repo use only) | Medium | needs branch protection | Adopt only if shared-repo workflows exist |
| Explicit declarations + fix-oriented messages (T3) | High | Low-medium | schema churn, noise | Adopt as lint rule |
| Opaque permanent IDs + tombstones (T5) | High | Medium | URL readability, fork collisions | Adopt in document model, skip comment adapters |
| Preflight JSON tool (`context`) (T7) | High | Low | none | Adopt as MCP tool |
| Removal-matrix CI for optional features (T4) | Medium-high | Medium | CI time | Adopt for plugin features |
| Dual-root ownership by path (T6) | Low for native app | n/a | not enforced | Skip; use bundle replace |
| System-as-package with context+skills (T7) | High | Medium | shipped examples empty; shadcn coupling | Adopt shape, machine-readable tokens |

## Recommendation (ranked)
1. Build the sync layer first: one canonical skill/context tree, generated per-host entries with a hashed receipt (T2) and a generated routing block (T1), with CI `--check`. Make `when` = skill `description` (one field) to remove the one drift hole found.
2. Make every pack manifest data-only with duplicate-key and `__proto__` rejection (T9.1) and "declare, don't default" validators with fix sentences (T3).
3. Put the permanent-ID + tombstone + never-mint-on-read rules in the DESIGN:OS document model (T5) and expose a `context --json` preflight over MCP (T7).
4. Do not import: manual-Git upgrade story (T6), comment/frontmatter ID adapters, or the doc volume (T8). Keep their doctrine, cut their length.

## Unresolved questions
- Does `compatible()` failing open on a malformed `requires` (`core/modules/index.ts:306-307`) get caught in every code path by `moduleProblems` first? Not traced.
- Is `PLATFORM_VERSION` asserted against release tags anywhere (`context/releasing.md` not read)?
- Do hosts (Codex, Cursor, Claude Code) honor the `.claude/skills/<name>` symlink -> `.agents/skills/<name>` chain identically? Repo says untested for Cursor.
- Real-world token cost of the generated `AGENTS.md` + stubs per session: not measured.
- Whether the Vite watcher re-syncs `AGENTS.md` on module `instructions` edits (only skills sync seen at `vite-manifest-watch-plugin.js:97`).

Status: DONE_WITH_CONCERNS
Summary: Report written; topics 1-8 each have verified file:line citations, weak spots, and portability notes, plus topic 9 extras and a ranked adoption matrix.
Concerns/Blockers: Read-only static analysis; nothing executed; plugin repo, file-types/publishing/releasing docs, onboarding module and browser app not read; host behavior unverified.
