# Release notes

Every tag `vX.Y.Z` needs `docs/releases/vX.Y.Z.md` before it is pushed. The Release workflow fails without it.

**Title** (set by the workflow): just the tag, `vX.Y.Z`. No product name or pack name.

**Body:** write only the sections below. The workflow appends Downloads (one row per OS), Checksums (from `SHA256SUMS`) and the full-changelog link.

Rules: English only. DESIGN:OS Studio is free; never call it "open-source". State only facts from merged PRs.

## Template

```markdown
One line: what this release is, with its milestone link.

## Highlights

- New or changed behavior (#PR).

## Fixes

- Bug fixed (#PR).

## Known issues

- Known problem and its tracking issue (#issue), or "- None reported."
```
