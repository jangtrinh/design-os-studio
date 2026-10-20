#!/usr/bin/env python3
"""Point the README, the GitHub Pages site and the docs at a published release.

    scripts/release-sync-pages.py 0.4.0                      # reads `gh release view v0.4.0`
    scripts/release-sync-pages.py 0.4.0 --assets-json r.json # offline, for tests
    scripts/release-sync-pages.py 0.4.0 --verify-urls        # also GET every new download URL

What it rewrites (only what the release actually lists, never a guess):
  * every `releases/download/v<old>/<asset>` link, plus the `<small>NN MB</small>` size
    inside the same anchor, from the asset list;
  * `Download Studio <old>` headings;
  * `build-release.sh <old>` examples;
  * the `<lastmod>` of docs/sitemap.xml (release publish date, UTC).

The previous version is read from the pages, never hardcoded. The script is idempotent:
on an up-to-date tree it changes nothing. It exits non-zero when a link would point at a
missing asset (the macOS .dmg is required; an optional Windows or Linux package that the
release does not list is dropped from index.html with a warning), when the requested
version is older than what the pages say, or when an old-version link is left over.

Marketing copy that names a version ("New in 0.3.0", film captions, anchors) needs real
new content, so it is never rewritten; the leftover mentions go to the `--report` file.
"""
from __future__ import annotations

import argparse
import json
import os
import re
import subprocess
import sys
import urllib.error
import urllib.request
from pathlib import Path

DEFAULT_REPO = "jangtrinh/design-os-studio"
TARGETS = ["README.md", "docs/index.html", "docs/install.md", "docs/llms.txt"]
SITEMAP = "docs/sitemap.xml"
VERSION = r"\d+\.\d+\.\d+"
REQUIRED_SUFFIX = "-macos.dmg"

# Leftover scan: files that legitimately keep old version numbers.
SCAN_SKIP_DIRS = {".git", "target", "plans", ".brv", "crates", "node_modules", "dist", ".claude", "videos"}
SCAN_SKIP_FILES = {"CHANGELOG.md", "Cargo.lock", "Cargo.toml"}
# History that names old versions on purpose, and this tool's own fixtures.
SCAN_SKIP_PREFIXES = ("docs/releasing.md", "docs/gpu-command-buffer-limit.md", "scripts/tests/", "scripts/release-sync-pages.py")
SCAN_MAX_BYTES = 1_000_000


class SyncError(Exception):
    pass


def semver(text: str) -> tuple[int, int, int]:
    return tuple(int(p) for p in text.split("."))  # type: ignore[return-value]


def size_label(num_bytes: int) -> str:
    return f"{int(num_bytes / 1_000_000 + 0.5)} MB"


def load_release(version: str, repo: str, assets_json: str | None) -> dict:
    if assets_json:
        data = json.loads(Path(assets_json).read_text())
    else:
        out = subprocess.run(
            ["gh", "release", "view", f"v{version}", "--repo", repo,
             "--json", "tagName,publishedAt,isPrerelease,assets"],
            capture_output=True, text=True,
        )
        if out.returncode != 0:
            raise SyncError(f"gh release view v{version} failed: {out.stderr.strip()}")
        data = json.loads(out.stdout)
    if data.get("tagName") not in (None, f"v{version}"):
        raise SyncError(f"release json is for {data.get('tagName')}, not v{version}")
    if data.get("isPrerelease"):
        raise SyncError(f"v{version} is a pre-release; pages only follow stable releases")
    return {
        "assets": {a["name"]: int(a["size"]) for a in data["assets"]},
        "published": (data.get("publishedAt") or "")[:10],
    }


def download_re(repo: str) -> re.Pattern:
    base = re.escape(f"https://github.com/{repo}/releases/download/")
    return re.compile(rf'{base}v(?P<ver>{VERSION})/(?P<name>[^"\s)<>]+)')


def anchor_re(repo: str) -> re.Pattern:
    base = re.escape(f"https://github.com/{repo}/releases/download/")
    return re.compile(
        rf'<a\b[^>]*href="{base}v(?P<ver>{VERSION})/(?P<name>[^"]+)"[^>]*>.*?</a>', re.S
    )


def previous_versions(texts: dict[str, str], repo: str) -> set[str]:
    """Every version the pages currently name in a download link, heading or build example."""
    found: set[str] = set()
    rx = download_re(repo)
    for text in texts.values():
        found.update(m["ver"] for m in rx.finditer(text))
        found.update(re.findall(rf"Download Studio ({VERSION})", text))
        found.update(re.findall(rf"build-release\.sh ({VERSION})\b", text))
    return found


def rewrite_text(path: str, text: str, version: str, release: dict, repo: str, warnings: list[str]) -> str:
    assets: dict[str, int] = release["assets"]
    html = path.endswith(".html")
    anchors = anchor_re(repo)

    def new_name(ver: str, name: str) -> str:
        return name.replace(ver, version)

    def fix_anchor(m: re.Match) -> str:
        ver, name = m["ver"], m["name"]
        target = new_name(ver, name)
        if target not in assets:
            if target.endswith(REQUIRED_SUFFIX):
                raise SyncError(f"{path}: required asset {target} is not in release v{version}")
            warnings.append(f"{path}: {target} is not in release v{version}; link dropped")
            return ""
        out = m[0].replace(f"/v{ver}/{name}", f"/v{version}/{target}")
        return re.sub(r"<small>[^<]*</small>", f"<small>{size_label(assets[target])}</small>", out)

    if html:
        text = anchors.sub(fix_anchor, text)
        # A platform row left with no link at all goes away with its links.
        text = re.sub(r'\n[ \t]*<li>\s*<span class="os">[^<]*</span>\s*<span class="files">\s*</span>\s*</li>', "", text)
    else:
        def fix_url(m: re.Match) -> str:
            ver, name = m["ver"], m["name"]
            target = new_name(ver, name)
            if target not in assets:
                raise SyncError(f"{path}: {target} is not in release v{version}")
            return m[0].replace(f"/v{ver}/{name}", f"/v{version}/{target}")
        text = download_re(repo).sub(fix_url, text)

    text = re.sub(rf"Download Studio {VERSION}", f"Download Studio {version}", text)
    text = re.sub(rf"(build-release\.sh )({VERSION})\b", rf"\g<1>{version}", text)
    return text


def rewrite_sitemap(text: str, published: str) -> str:
    if not published:
        return text
    return re.sub(r"<lastmod>[^<]*</lastmod>", f"<lastmod>{published}</lastmod>", text)


def final_assertions(root: Path, files: list[str], version: str, release: dict, repo: str) -> None:
    rx = download_re(repo)
    problems: list[str] = []
    for rel in files:
        text = (root / rel).read_text()
        for m in rx.finditer(text):
            line = text.count("\n", 0, m.start()) + 1
            if m["ver"] != version:
                problems.append(f"{rel}:{line}: old-version link left: {m[0]}")
            elif m["name"] not in release["assets"]:
                problems.append(f"{rel}:{line}: link to missing asset {m['name']}")
        for m in re.finditer(rf"build-release\.sh ({VERSION})\b", text):
            if m[1] != version:
                line = text.count("\n", 0, m.start()) + 1
                problems.append(f"{rel}:{line}: old build-release.sh version {m[1]}")
        for m in re.finditer(rf"Download Studio ({VERSION})", text):
            if m[1] != version:
                problems.append(f"{rel}: old 'Download Studio {m[1]}' heading")
    if problems:
        raise SyncError("not clean after rewrite:\n  " + "\n  ".join(problems))


def leftover_mentions(root: Path, old_versions: set[str]) -> list[str]:
    """Lines outside the synced targets that still name a previous version."""
    hits: list[str] = []
    if not old_versions:
        return hits
    forms = []
    for v in sorted(old_versions):
        forms.append(r"(?<![\d.])" + re.escape(v) + r"(?![\d])")
        forms.append(r"\bv" + v.replace(".", "") + r"(?!\d)")
    rx = re.compile("|".join(forms))
    for dirpath, dirnames, filenames in os.walk(root):
        dirnames[:] = [d for d in dirnames if d not in SCAN_SKIP_DIRS]
        for fname in sorted(filenames):
            if fname in SCAN_SKIP_FILES:
                continue
            path = Path(dirpath, fname)
            rel = path.relative_to(root).as_posix()
            if rel.startswith(SCAN_SKIP_PREFIXES):
                continue
            try:
                if path.stat().st_size > SCAN_MAX_BYTES:
                    continue
                lines = path.read_text().splitlines()
            except (UnicodeDecodeError, OSError):
                continue
            for i, line in enumerate(lines, 1):
                if rx.search(line):
                    hits.append(f"{rel}:{i}: {line.strip()[:110]}")
    return hits


def verify_urls(urls: list[str]) -> list[str]:
    """GET the first byte of each URL; GitHub's CDN rejects HEAD on signed redirects."""
    bad = []
    for url in urls:
        req = urllib.request.Request(url, headers={"Range": "bytes=0-0", "User-Agent": "release-sync-pages"})
        try:
            with urllib.request.urlopen(req, timeout=60) as resp:
                status = resp.status
        except urllib.error.HTTPError as err:
            status = err.code
        except OSError as err:
            bad.append(f"{url}: {err}")
            continue
        if status not in (200, 206):
            bad.append(f"{url}: HTTP {status}")
    return bad


PUBLISH_JOB = "Publish release"


def publish_job_succeeded(run: dict, name: str = PUBLISH_JOB) -> bool:
    """True when the Release workflow's publish job succeeded, whatever the run's own conclusion.

    The run ends `failure` when a Windows or Linux leg fails, yet release.yml still publishes
    (macOS is the only required leg), so the run conclusion alone would skip a real release.
    `run` is `gh run view <id> --json jobs` output.
    """
    return any(j.get("name") == name and j.get("conclusion") == "success" for j in run.get("jobs", []))


def run(args: argparse.Namespace) -> int:
    if args.publish_job_ok:
        # Exit 0 = succeeded, 1 = did not, 2 = no usable answer (empty stdout, a 403 body).
        # The caller must fail on 2, never treat it as "skip".
        try:
            data = json.load(sys.stdin)
        except ValueError as err:
            print(f"error: stdin is not JSON ({err}); did `gh run view` fail?", file=sys.stderr)
            return 2
        if not isinstance(data, dict) or not isinstance(data.get("jobs"), list):
            print("error: stdin has no 'jobs' list; did `gh run view` fail?", file=sys.stderr)
            return 2
        return 0 if publish_job_succeeded(data) else 1
    if not args.version:
        raise SyncError("version is required")
    version = args.version.removeprefix("v")
    if not re.fullmatch(VERSION, version):
        raise SyncError(f"version must look like 1.2.3, got {args.version!r}")
    root = Path(args.root).resolve()
    release = load_release(version, args.repo, args.assets_json)
    if not any(n.endswith(REQUIRED_SUFFIX) for n in release["assets"]):
        raise SyncError(f"release v{version} has no *{REQUIRED_SUFFIX} asset")

    files = [f for f in TARGETS if (root / f).is_file()]
    texts = {f: (root / f).read_text() for f in files}
    old = previous_versions(texts, args.repo)
    newest_seen = max(old | {version}, key=semver)
    if newest_seen != version:
        raise SyncError(f"pages already point at v{newest_seen}, newer than v{version}; refusing to downgrade")

    warnings: list[str] = []
    changed: list[str] = []
    for f in files:
        out = rewrite_text(f, texts[f], version, release, args.repo, warnings)
        if out != texts[f]:
            (root / f).write_text(out)
            changed.append(f)
    sm = root / SITEMAP
    if sm.is_file():
        before = sm.read_text()
        after = rewrite_sitemap(before, release["published"])
        if after != before:
            sm.write_text(after)
            changed.append(SITEMAP)

    final_assertions(root, files, version, release, args.repo)

    rx = download_re(args.repo)
    urls = sorted({m[0] for f in files for m in rx.finditer((root / f).read_text()) if m["ver"] == version})
    if args.verify_urls:
        bad = verify_urls(urls)
        if bad:
            raise SyncError("download URLs not reachable:\n  " + "\n  ".join(bad))
        print(f"verified {len(urls)} download URLs return 200/206")

    leftovers = leftover_mentions(root, old - {version})
    if args.report:
        lines = [f"## Release sync for v{version}", ""]
        lines.append(f"Previous version on the pages: {', '.join(sorted(old - {version})) or 'none (already current)'}")
        lines.append(f"Files changed: {', '.join(changed) or 'none'}")
        lines.append("")
        if warnings:
            lines += ["### Warnings", *[f"- {w}" for w in warnings], ""]
        lines.append("### Manual follow-up")
        lines.append("Mentions of the previous version that need real new content, not a find-and-replace:")
        lines += [f"- `{h}`" for h in leftovers] or ["- none"]
        Path(args.report).write_text("\n".join(lines) + "\n")

    for w in warnings:
        print(f"warning: {w}", file=sys.stderr)
    print(f"v{version}: " + (f"updated {', '.join(changed)}" if changed else "no changes (already up to date)"))
    if leftovers:
        print(f"{len(leftovers)} leftover mention(s) of the previous version need manual copy work")
    return 0


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("version", nargs="?", help="release version, 1.2.3 or v1.2.3")
    ap.add_argument("--publish-job-ok", action="store_true",
                    help=f"read `gh run view --json jobs` from stdin; exit 0 if the '{PUBLISH_JOB}' job succeeded")
    ap.add_argument("--root", default=".", help="repository root (default: cwd)")
    ap.add_argument("--repo", default=DEFAULT_REPO)
    ap.add_argument("--assets-json", help="gh release view --json tagName,publishedAt,isPrerelease,assets output")
    ap.add_argument("--verify-urls", action="store_true", help="GET every new download URL")
    ap.add_argument("--report", help="write a markdown report (warnings + manual follow-ups) to this file")
    try:
        return run(ap.parse_args())
    except SyncError as err:
        print(f"error: {err}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    sys.exit(main())
