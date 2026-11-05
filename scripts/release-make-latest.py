#!/usr/bin/env python3
"""Keep GitHub's Latest badge on the highest published stable release.

    gh release list --limit 1000 --json tagName,isDraft,isPrerelease | scripts/release-make-latest.py vX.Y.Z
    gh release list --limit 1000 --json tagName,isDraft,isPrerelease,isLatest | scripts/release-make-latest.py --reassert vX.Y.Z

Default mode, for the publish step of release.yml. Prints `true` when the tag is the highest
stable `vX.Y.Z` among the published (non-draft, non-prerelease) releases plus the tag itself,
`false` when a higher one is already published. GitHub marks a new full release Latest by
default, so an older number published after a newer one would take the badge;
`make_latest: false` stops that (docs/releasing.md). A pre-release tag (it has a hyphen)
prints nothing: the step then leaves `make_latest` unset and GitHub behaves as it always did
(it never makes a pre-release Latest). The output must be exactly `true` or `false`:
softprops/action-gh-release@v2 reads any other value as "unset", and unset means GitHub's
default, which is Latest.

`--reassert`, for the sync workflow that runs from main. release.yml runs from the tagged
commit, so a tag cut from a branch without the guard still takes Latest. Prints the tag that
should hold Latest (the highest published stable release) when it does not hold it yet, so a
repair run for the highest version works too; prints nothing when there is nothing to fix, or
when `vX.Y.Z` is newer than every listed release (a stale list). Needs `isLatest`.

A tag that is neither `vX.Y.Z` nor hyphenated (`v1.0.0+b1`) cannot be ranked: exit 2.

Exit codes: 0 answered, 2 no usable answer (usage error, empty stdout from a failed `gh`,
an API error body, rows without the required fields, a tag that cannot be ranked). The caller
must fail on 2, never publish or edit on a guess.
"""
from __future__ import annotations

import json
import re
import sys

STABLE_TAG = re.compile(r"v(\d+)\.(\d+)\.(\d+)")
Version = tuple[int, int, int]


class LatestError(Exception):
    pass


def stable_version(tag: str) -> Version | None:
    m = STABLE_TAG.fullmatch(tag)
    return (int(m[1]), int(m[2]), int(m[3])) if m else None


def tag_version(tag: str) -> Version | None:
    """The version of the tag being published: `vX.Y.Z`; None for a hyphenated (pre-release) tag; else an error."""
    version = stable_version(tag)
    if version or "-" in tag:
        return version
    raise LatestError(f"tag {tag!r} is neither vX.Y.Z nor a hyphenated pre-release, so it cannot be ranked")


def published_stable(releases: object, need_is_latest: bool = False) -> list[tuple[Version, str, bool]]:
    """(version, tag, holds Latest) of every published, stable `vX.Y.Z` release, highest first."""
    if not isinstance(releases, list):
        raise LatestError("stdin is not the JSON array of `gh release list`; did it fail?")
    found: list[tuple[Version, str, bool]] = []
    for row in releases:
        if not (isinstance(row, dict) and isinstance(row.get("tagName"), str)
                and isinstance(row.get("isDraft"), bool) and isinstance(row.get("isPrerelease"), bool)
                and (isinstance(row.get("isLatest"), bool) or not need_is_latest)):
            need = "tagName, isDraft, isPrerelease" + (" and isLatest" if need_is_latest else "")
            raise LatestError(f"a `gh release list` row needs {need}: {row!r}")
        version = stable_version(row["tagName"])
        if version and not row["isDraft"] and not row["isPrerelease"]:
            found.append((version, row["tagName"], row.get("isLatest") is True))
    return sorted(found, reverse=True)


def make_latest(tag: str, releases: object) -> str:
    """`true`, `false`, or `` (leave the input unset) for `tag` given the `gh release list` rows."""
    mine = tag_version(tag)
    published = published_stable(releases)
    if mine is None:
        return ""
    return "true" if not published or mine >= published[0][0] else "false"


def reassert_latest(tag: str, releases: object) -> str:
    """The highest published stable tag when it must be given Latest, or `` when nothing needs fixing."""
    mine = tag_version(tag)
    published = published_stable(releases, need_is_latest=True)
    if mine is None or not published:
        return ""
    version, highest, holds_latest = published[0]
    if mine > version:
        return ""  # the tag's own release is not in the list yet; the list is stale, so do not touch Latest
    return "" if holds_latest else highest


def main() -> int:
    argv = sys.argv[1:]
    reassert = argv[:1] == ["--reassert"]
    if reassert:
        argv = argv[1:]
    if len(argv) != 1:
        print("usage: gh release list --json tagName,isDraft,isPrerelease[,isLatest] "
              "| release-make-latest.py [--reassert] <tag>", file=sys.stderr)
        return 2
    try:
        releases = json.load(sys.stdin)
    except ValueError as err:
        print(f"error: stdin is not JSON ({err}); did `gh release list` fail?", file=sys.stderr)
        return 2
    try:
        answer = (reassert_latest if reassert else make_latest)(argv[0], releases)
    except LatestError as err:
        print(f"error: {err}", file=sys.stderr)
        return 2
    if answer:
        print(answer)
    return 0


if __name__ == "__main__":
    sys.exit(main())
