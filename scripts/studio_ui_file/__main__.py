"""Tools for `design/studio-ui.designos`, the Studio chrome drawn in Studio itself.

    python3 scripts/studio_ui_file <command> [options]

    plan        theme.rs -> variable plan; prints the constant and variable counts (no MCP)
    foundation  foundation-only file with the installed designos-mcp (pages, variables, text styles, icons, Foundations
                specimens), written to a mandatory --out; the gate refuses it (no component set): use `library` for the committed file
    library     foundation plus every component module of components/ on page 02 Core, layout pass, bind intents verified; the
                committed file and its allow-list are replaced only after every check (reopen, has-hardcoded, file gate, swap) passed
    check       the gate (G1 parity, G2 bound, G3 registry, G4 content) on a file; same as scripts/check_studio_ui_file.py
    evidence    renders, an MCP reopen check, the icon comparison (needs Pillow) and structure checks:
                evidence render | verify | icons | structure | compare | screens | regress

Standard library only. Pillow is imported by `evidence icons` alone, which no CI path runs. Component work (W1 on) adds
one module per tier under components/. Every file is at most 200 lines.
"""
import sys
from pathlib import Path

PKG = Path(__file__).resolve().parent
sys.path.insert(0, str(PKG.parent))  # scripts/: the committed gate, theme reader and policy
sys.path.insert(0, str(PKG))

EVIDENCE = {"render": "render_sections", "verify": "verify_foundation", "icons": "icon_compare", "structure": "file_checks", "compare": "component_compare",
            "screens": "screen_compare", "regress": "scores_regression"}


def _evidence(argv):
    if not argv or argv[0] not in EVIDENCE:
        print(f"usage: studio_ui_file evidence {{{' | '.join(EVIDENCE)}}} [options]", file=sys.stderr)
        return 2
    import importlib
    return importlib.import_module(f"evidence.{EVIDENCE[argv[0]]}").main(argv[1:])


def _plan(argv):
    import variables_plan
    return variables_plan.main(argv)


def _foundation(argv):
    import assemble_foundation
    return assemble_foundation.main(argv)


def _library(argv):
    import assemble_library
    return assemble_library.main(argv)


def _check(argv):
    import check_studio_ui_file
    return check_studio_ui_file.main(argv)


COMMANDS = {"plan": _plan, "foundation": _foundation, "library": _library, "check": _check, "evidence": _evidence}


def main(argv=None):
    argv = sys.argv[1:] if argv is None else argv
    if not argv or argv[0] in ("-h", "--help") or argv[0] not in COMMANDS:
        print(__doc__)
        return 0 if argv and argv[0] in ("-h", "--help") else 2
    return COMMANDS[argv[0]](argv[1:]) or 0


if __name__ == "__main__":
    sys.exit(main())
