"""Every filesystem open, read, write, rename, create_dir or stat in non-test designos-mcp code, with its class.

Usage: python3 -I fs_sites_classified.py <designos-mcp src dir> [--hits-only]

Skips files named tests.rs, *_tests.rs, test_*.rs, src/bin/gen-tool-docs.rs, and any item gated by #[cfg(test)] or
#[cfg(all(test ...))] (same stripping rule as the PR #1067 review script non_test_fs_writes.py, with a wider pattern: reads and
metadata calls too). Each hit is looked up in CLASSES by (file, enclosing fn); a hit with no class is printed as UNCLASSIFIED
and the exit status is 1, so the list cannot silently fall behind the code. A hit classed `session` outside the helper
files must go through the helper (session_file_io:: / file_problems. / problems.); one that does not is printed as RAW-SESSION and
also fails the run.

Classes:
  session  a file below DESIGNOS_SESSION_DIR (or the path of an env override of one): must go through session_file_io
  workspace  the --workspace tools: open_file, save_file, render_png, export_fig_file, import_web_capture (out of scope per #1068)
  library  Studio's library root or the bundled guideline files: not agent-writable session files
  other  none of the above
"""
import os
import re
import sys

PAT = re.compile(
    r"fs::|File::|OpenOptions|read_to_string|read_dir|\.metadata\(|canonicalize|\.exists\(\)|\.is_file\(\)|\.is_dir\(\)|write_all|"
    r"create_dir|remove_file|remove_dir|rename\(|session_file_io::|file_problems\.|problems\.(read|wrote|modified|keep)|"
    r"load_external|designos_library::"
)
GATE = re.compile(r"#\[cfg\((test|all\(test[^\]]*|any\(test[^\]]*)\)\]")
FN = re.compile(r"^\s*(?:pub(?:\([^)]*\))?\s+)?(?:async\s+)?fn\s+([A-Za-z0-9_]+)")

# (file suffix, enclosing fn or None for any, class, why)
CLASSES = [
    ("src/session_file_io.rs", None, "session", "the helper itself: lstat, refuses a link or a non-file"),
    ("src/session_file_io/no_follow.rs", None, "session", "the helper itself: O_NOFOLLOW|O_NONBLOCK opens, fstat check"),
    ("src/session_file_io/path_based.rs", None, "session", "the helper itself: non-unix fallback"),
    ("src/session_file_problems.rs", None, "session", "calls the helper and keeps what it refused"),
    ("src/server.rs", "atomic_write_file", "session", "canvas.designos, doc-sync.designos: write_atomic"),
    ("src/server.rs", "append_log_line", "session", "doc-patch.jsonl and the eval transcript: append"),
    ("src/server.rs", "load_enabled_tools", "session", "mcp-config.json (or DESIGNOS_MCP_CONFIG): read"),
    ("src/server.rs", "load_session_options", "session", "session-options.json: read"),
    ("src/server.rs", "save_session_options", "session", "session-options.json: write_atomic via save_to_dir"),
    ("src/server.rs", "replay_unabsorbed_live_patches", "session", "doc-patch.jsonl: read"),
    ("src/server.rs", "edit_log_len", "session", "doc-patch.jsonl: lstat"),
    ("src/server.rs", "sync_from_live_canvas_if_newer", "session", "context.json, mcp-config.json, canvas.designos, bootstrap.json: read"),
    ("src/server.rs", "publish_live_doc_sync", "session", "canvas.designos mtime after the write: lstat"),
    ("src/server.rs", "publish_live_patch", "session", "doc-patch.jsonl: append_log_line"),
    ("src/server.rs", "call", "session", "transcript: append_log_line (eval harness path, not a session-folder file)"),
    ("src/server.rs", "attach_session_file_warnings", "session", "drains the problems to the reply"),
    ("src/server.rs", "new", "workspace", "the --workspace root check"),
    ("src/server.rs", "with_session_dir", "workspace", "the --workspace root check"),
    ("src/rules.rs", "load", "session", "mcp-config.json read (base only; now Session reads it through the helper)"),
    ("src/tools/session_options.rs", "load_from_dir", "session", "session-options.json read (base only; now Session reads it through the helper)"),
    ("src/server.rs", "input_path", "workspace", "--workspace path check (canonicalize)"),
    ("src/server.rs", "output_path_allow_overwrite", "workspace", "designos_save_file target check"),
    ("src/server.rs", "png_output_path", "workspace", "designos_render_png target check"),
    ("src/server.rs", "fig_output_path", "workspace", "designos_export_fig_file target check"),
    ("src/server.rs", "call_inner", "workspace", "open_file, save_file, render_png, export_fig_file, import_web_capture"),
    ("src/engine.rs", "read_session_json_field", "session", "context.json / mcp-config.json fallback read via session_file_io"),
    ("src/engine.rs", "call", "workspace", "designos_export_fig_file and import_web_capture file reads (--workspace)"),
    ("src/engine.rs", None, "workspace", "--workspace tool file access"),
    ("src/tools/ledger.rs", "load_from_path", "session", "idempotency-ledger.jsonl: read"),
    ("src/tools/ledger.rs", "rewrite_file", "session", "idempotency-ledger.jsonl: write_atomic"),
    ("src/tools/ledger.rs", "clear", "session", "idempotency-ledger.jsonl: truncate"),
    ("src/tools/ledger.rs", "append_json_line", "session", "idempotency-ledger.jsonl: append"),
    ("src/tools/session_options.rs", "save_to_dir", "session", "session-options.json: write_atomic"),
    ("src/tools/libraries.rs", None, "library", "Studio library root (DESIGNOS_LIBRARY): read-only listing"),
    ("src/tools/library_source.rs", None, "library", "Studio library root: read-only snapshots"),
    ("src/tools/guidelines.rs", None, "library", "bundled guideline files: read-only"),
]


def strip_test_items(lines):
    keep = [True] * len(lines)
    i, n = 0, len(lines)
    while i < n:
        if GATE.search(lines[i]) and "not(test" not in lines[i]:
            j, depth, started = i, 0, False
            while j < n:
                code = lines[j].split("//")[0]
                for ch in code:
                    if ch == "{":
                        depth += 1
                        started = True
                    elif ch == "}":
                        depth -= 1
                keep[j] = False
                if started and depth <= 0:
                    break
                if not started and code.rstrip().endswith(";") and j > i:
                    break
                j += 1
            i = j + 1
            continue
        i += 1
    return keep


def skipped(rel, name):
    return name in ("tests.rs",) or name.endswith("_tests.rs") or name.startswith("test_") or rel.endswith("src/bin/gen-tool-docs.rs") or "/tests/" in rel


def enclosing_fn(lines, index):
    indent = len(lines[index]) - len(lines[index].lstrip())
    for k in range(index, -1, -1):
        m = FN.match(lines[k])
        if m and (len(lines[k]) - len(lines[k].lstrip())) <= indent:
            return m.group(1)
    return None


def classify(rel, fn):
    for suffix, want_fn, klass, why in CLASSES:
        if rel.endswith(suffix) and (want_fn is None or want_fn == fn):
            return klass, why
    return None, None


HELPER_FILES = ("src/session_file_io.rs", "src/session_file_io/no_follow.rs", "src/session_file_io/path_based.rs", "src/session_file_problems.rs")
VIA_HELPER = re.compile(r"session_file_io::|file_problems\.|problems\.|\.problems")


def raw_session_site(rel, klass, code):
    """A session-class site outside the helper that does not go through it."""
    return klass == "session" and not rel.endswith(HELPER_FILES) and not VIA_HELPER.search(code)


def main(root, hits_only):
    hits, unclassified = [], 0
    for dirpath, _, files in sorted(os.walk(root)):
        for name in sorted(files):
            path = os.path.join(dirpath, name)
            rel = os.path.relpath(path, os.path.dirname(root.rstrip("/"))).replace(os.sep, "/")
            rel = "src/" + rel.split("src/", 1)[1] if "src/" in rel else rel
            if not name.endswith(".rs") or skipped(rel, name):
                continue
            lines = open(path).read().split("\n")
            keep = strip_test_items(lines)
            for idx, line in enumerate(lines):
                code = line.split("//")[0]
                if not keep[idx] or not PAT.search(code) or line.lstrip().startswith(("//", "///", "//!")):
                    continue
                fn = enclosing_fn(lines, idx)
                klass, why = classify(rel, fn)
                if raw_session_site(rel, klass, code):
                    klass = "RAW-SESSION"
                hits.append((rel, idx + 1, fn, klass, why, line.strip()))
                unclassified += klass is None or klass == "RAW-SESSION"
    for rel, lineno, fn, klass, why, code in hits:
        tag = klass or "UNCLASSIFIED"
        print(f"{tag:<12} {rel}:{lineno:<5} fn {fn or '-':<34} {code[:110]}" + ("" if hits_only or not why else f"   // {why}"))
    by_class = {}
    for h in hits:
        by_class[h[3] or "UNCLASSIFIED"] = by_class.get(h[3] or "UNCLASSIFIED", 0) + 1
    print("\nSUMMARY:", ", ".join(f"{k}={v}" for k, v in sorted(by_class.items())), f"(total {len(hits)})")
    return 1 if unclassified else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1], "--hits-only" in sys.argv))
