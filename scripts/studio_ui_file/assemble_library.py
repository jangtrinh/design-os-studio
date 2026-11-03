"""Assemble design/studio-ui.designos with the component library on page `02 Core`.

    python3 scripts/studio_ui_file library [--repo R] [--workspace DIR] [--keep-workspace] [--out FILE]

The foundation (pages, variables, text styles, icons, specimens) plus every module in components/. Order: REST import with
page 02 filled -> variables, text styles -> id map (`source_id`) -> layout pass (measure texts, grid, focus rings, sections) ->
foundation specimens -> the paint a swap drops written back through MCP, on the swapped glyphs and on every chain copy of one (swap_paints) -> save -> native post-pass (code
syntax, style bindings, every bind intent, their binding and `Fills` flag) -> reopen -> save -> reopen
and verify that EVERY intent shows in `effective_bindings` (a miss is fatal) -> a STAGED copy in the workspace. Then the MCP `has-hardcoded`
rows on 02 Core must equal the allow-list (hardcoded_check), the file gate must pass on the staged file, and the icon-swap round-trip runs (swap_check:
reporting as SWAP-PENDING #1427 until SWAP_CHECK_FATAL flips). Only when every check has passed are `design/studio-ui.designos` and its allow-list
replaced (library_publish): a failing check leaves the committed files untouched. The scratch workspace is a temporary directory removed
when the run ends (kept with --keep-workspace, or when --workspace names one).
"""
import json
import pathlib

import assemble_foundation as base
import check_studio_ui_file as gate
import library_publish
import scratch_workspace as scratch
import icon_lib
import icons_rest
import library_core
import library_layout
import hardcoded_check
import library_verify
import swap_check
import swap_paints
import mcp_client as mcpc
import screens as screens_pkg
import studio_ui_theme_reader as theme
import text_styles_plan as styles_mod
import variables_plan as plan_mod


def main(argv=None):
    a = base.parse_args("studio_ui_file library", argv)
    with scratch.workspace(a.workspace, keep=a.keep_workspace) as ws:
        a.workspace = ws
        return build(a)


def build(a):
    plan = plan_mod.build_plan(a.repo)
    styles, style_bindings = styles_mod.build(plan)
    nodes, comps, skipped = icons_rest.build_components(a.repo)
    core = library_core.build_core(a.repo, plan, styles, icon_lib.IconLib(a.repo))
    (a.workspace / "library.json").write_text(json.dumps(base.rest_document(nodes, comps, core)))
    mcp = mcpc.Mcp(str(a.workspace))
    stats = {"server": mcp.server_info}
    pages = base.open_and_define(mcp, plan, styles, "library.json")
    mcp.tool_or_die("designos_save_file", {"path": "stage0.designos", "overwrite": True})
    ids = library_layout.source_ids(a.workspace / "stage0.designos")
    stats["texts_measured"] = library_layout.relayout(mcp, ids, core["info"])
    stats["nested_texts_checked"] = library_layout.check_nested(mcp, ids, core["info"])
    stats["resized_copies_checked"] = library_layout.check_resized(mcp, ids, core["info"])
    sections = {}
    for s in core["info"]:
        sections.setdefault(s["section"], []).append(s["id"])
    sizes = library_layout.place(mcp, ids, core["info"], sections)
    stats.update(base.draw_foundation(mcp, plan, styles, nodes, pages))
    registry = json.loads((pathlib.Path(a.repo) / "design/component-registry.json").read_text(encoding="utf-8"))
    stats["screens"] = screens_pkg.build(mcp, pages["04 Screens"], screens_pkg.check_specs(screens_pkg.discover(), registry))
    stats["swap_paints"] = swap_paints.apply_in_session(mcp, a.workspace)  # paint 0 of every swapped glyph and chain copy; the post-pass adds the binding and the flag below
    staged = library_publish.stage_path(a.workspace, a.out)
    stats.update(base.finish(mcp, a.workspace, staged, plan, style_bindings, core["intents"], core["allow"]))
    stats["swap_paints"].update(stats["post_pass"].pop("swap_paints"))
    allow = stats.pop("allow")  # the given entries plus the ones mirrored onto nested instances
    mcp = mcpc.Mcp(str(a.workspace))
    mcp.tool_or_die("designos_open_file", {"path": "studio-ui.designos", "discard_changes": True})
    stats["verify"] = library_verify.verify(mcp, a.workspace / "studio-ui.designos", plan, core["intents"], stats["swap_paints"])
    problems = hardcoded_check.check(mcp, a.workspace / "studio-ui.designos", allow)
    if problems:
        raise SystemExit("MCP has-hardcoded rows on 02 Core differ from the allow-list:\n" + "\n".join(problems[:25]))
    stats["has_hardcoded_rows_match_allow_list"] = True
    repo = pathlib.Path(a.repo)
    if gate.main([str(staged), "--allowlist", str(library_publish.staged_allowlist(staged)), "--theme", str(repo / theme.THEME_REL),
                  "--tokens", str(repo / "design/design.tokens.json"), "--registry", str(repo / "design/component-registry.json")]):
        raise SystemExit(f"the file gate failed on the staged file: {a.out} was not written (rerun with --keep-workspace to keep {staged})")
    stats["swap"] = swap_check.run(mcp, a.workspace)  # on a scratch copy: the staged file is never saved from here
    mcp.close()
    print("\n".join(swap_check.report(stats["swap"])))  # SWAP-PENDING while SWAP_CHECK_FATAL is False; stops the build once it is True
    library_publish.publish(staged, a.out)  # every check passed
    stats.update(bytes=pathlib.Path(a.out).stat().st_size, sets=len(core["info"]), variants=len(sizes), intents=len(core["intents"]),
                 allow_entries=len(allow), skipped_icons=skipped)
    (a.workspace / "variant-sizes.json").write_text(json.dumps(sizes, indent=1))
    (a.workspace / "assemble-stats.json").write_text(json.dumps(stats, indent=1))
    print(json.dumps(stats, indent=1))
    return 0
