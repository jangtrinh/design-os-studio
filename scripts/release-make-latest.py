#!/usr/bin/env python3
"""Decide the `make_latest` input of the release publish step.

    gh release list --limit 1000 --json tagName,isDraft,isPrerelease | scripts/release-make-latest.py vX.Y.Z

Prints `true` when the tag is the highest stable `vX.Y.Z` among the published (non-draft,
non-prerelease) releases plus the tag itself, `false` when a higher one is already published.
GitHub marks a new full release Latest by default, so an older number published after a newer
one would take the badge; `make_latest: false` stops that (docs/releasing.md).

A pre-release tag, or any tag that is not `vX.Y.Z`, prints nothing: the step then leaves
`make_latest` unset and GitHub behaves as it always did (it never makes a pre-release Latest).

The output must be exactly `true` or `false`. softprops/action-gh-release@v2 reads any other
value as "unset", and unset means GitHub's default, which is Latest.

Exit codes: 0 answered, 2 no usable answer (usage error, empty stdout from a failed `gh`,
an API error body, rows without the three fields). The caller must fail on 2, never publish.
"""
from __future__ import annotations

import json
import re
import sys

STABLE_TAG = re.compile(r"v(\d+)\.(\d+)\.(\d+)")


class LatestError(Exception):
    pass


def stable_version(tag: str) -> tuple[int, int, int] | None:
    m = STABLE_TAG.fullmatch(tag)
    return (int(m[1]), int(m[2]), int(m[3])) if m else None


def make_latest(tag: str, releases: object) -> str:
    """`true`, `false`, or `` (leave the input unset) for `tag` given the `gh release list` rows."""
    if not isinstance(releases, list):
        raise LatestError("release list is not a JSON array")
    mine = stable_version(tag)
    published: list[tuple[int, int, int]] = []
    for row in releases:
        if not (isinstance(row, dict) and isinstance(row.get("tagName"), str)
                and isinstance(row.get("isDraft"), bool) and isinstance(row.get("isPrerelease"), bool)):
            raise LatestError(f"release row needs tagName, isDraft and isPrerelease: {row!r}")
        version = stable_version(row["tagName"])
        if version and not row["isDraft"] and not row["isPrerelease"]:
            published.append(version)
    if mine is None:
        return ""
    return "true" if mine >= max(published, default=mine) else "false"


def main() -> int:
    if len(sys.argv) != 2:
        print("usage: gh release list --json tagName,isDraft,isPrerelease | release-make-latest.py <tag>",
              file=sys.stderr)
        return 2
    try:
        releases = json.load(sys.stdin)
        answer = make_latest(sys.argv[1], releases)
    except (ValueError, LatestError) as err:
        print(f"error: stdin is not the JSON of `gh release list` ({err}); did `gh release list` fail?",
              file=sys.stderr)
        return 2
    if answer:
        print(answer)
    return 0


if __name__ == "__main__":
    sys.exit(main())
