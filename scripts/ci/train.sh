#!/usr/bin/env bash
# Batch merge train for jangtrinh/design-os-studio.
#
# Selects open, non-draft PRs targeting main that carry `train:ready`, have all 4 required
# checks green on their own head (`Check`, `Test (macos-15)`, `Clippy`, `Rustfmt`), and are
# not stacked on another branch or open PR.
#
# Orders eligible PRs: `hotfix` -> `release:` / `train:solo` (alone) -> oldest (`createdAt`),
# capped at MAX_BATCH (default 5).
#
# In live mode (DRY_RUN=0):
#   1. Builds `train/<UTC ts>` from `origin/main` via `git merge --no-ff` of each PR head SHA
#      (on conflict: aborts that merge, drops the PR, labels `train:conflict`, removes
#      `train:ready`, and comments).
#   2. Pushes `train/<UTC ts>` and polls the 4 required checks on the train SHA every
#      POLL_INTERVAL (default 120s) up to POLL_TIMEOUT (default 5400s = 90m).
#   3. Green: squash-merges each PR in order with `gh pr merge N --squash --match-head-commit <sha>`,
#      removes `train:ready`, comments `"landed in <sha> with #a #b"`, and deletes the train branch.
#   4. Red: lists failing check names; if >1 PR in batch, not yet retried, and exactly one PR's
#      changed files intersect the failing crate(s), labels that PR `train:failed` (with the
#      failing check names) and reruns the remaining PRs once; otherwise labels all batch PRs
#      `train:failed` and stops. Never uses `--admin`; never merges red.
#
# In dry-run mode (DRY_RUN=1):
#   Performs read-only queries only; prints the selection, order, and planned merges without
#   pushing branches, merging PRs, or changing labels/comments.

set -euo pipefail

REPO="${GH_REPO:-jangtrinh/design-os-studio}"
MAX_BATCH="${MAX_BATCH:-5}"
POLL_INTERVAL="${POLL_INTERVAL:-120}"
POLL_TIMEOUT="${POLL_TIMEOUT:-5400}"
REQUIRED=("Check" "Test (macos-15)" "Clippy" "Rustfmt")

case "${DRY_RUN:-1}" in
  0|false|FALSE|no|NO) DRY_RUN=0 ;;
  *) DRY_RUN=1 ;;
esac

log() {
  printf '%s %s\n' "$(date -u +%H:%M:%SZ)" "$*"
}

die() {
  printf 'ERROR: %s\n' "$*" >&2
  exit 1
}

# Run the jq selection & ordering pipeline on a `gh pr list --json ...` payload.
# Input: JSON array on stdin.
# Output: JSON object `{total_open, draft_count, non_draft_count, ready_count, selected, skipped, non_ready_non_draft}`.
select_and_order_json() {
  local max_batch="$1"
  jq --argjson max_batch "$max_batch" '
    def label_names:
      [(.labels // [])[] | if type == "object" then (.name // "") else tostring end];

    def has_label($name):
      (label_names | index($name)) != null;

    def check_one($pr; $req):
      [($pr.statusCheckRollup // $pr.check_runs // [])[]
        | select(((.name // .context // "") == $req))] as $runs
      | if ($runs | length) == 0 then
          "PENDING:" + $req
        else
          ($runs | last) as $last
          | (($last.status // "COMPLETED") | ascii_downcase) as $st
          | (($last.conclusion // $last.state // "") | ascii_downcase) as $conc
          | if $st != "completed" then
              "PENDING:" + $req
            elif ($conc == "success" or $conc == "skipped") then
              "SUCCESS"
            else
              "FAIL:" + $req
            end
        end;

    def required_checks_summary:
      . as $pr
      | (["Check", "Test (macos-15)", "Clippy", "Rustfmt"] | map(check_one($pr; .))) as $states
      | ($states | map(select(startswith("FAIL:"))) | map(sub("^FAIL:"; ""))) as $failed
      | ($states | map(select(startswith("PENDING:"))) | map(sub("^PENDING:"; ""))) as $pending
      | if ($failed | length) > 0 then
          {ok: false, state: ("FAIL:" + ($failed | join(", ")))}
        elif ($pending | length) > 0 then
          {ok: false, state: ("PENDING:" + ($pending | join(", ")))}
        else
          {ok: true, state: "SUCCESS"}
        end;

    def body_dep_numbers:
      [(.body // "")
        | scan("(?i)(?:depends on|stacked on|base pr:?)\\s*#([0-9]+)")
        | .[0] | tonumber];

    def commit_oids:
      [(.commits // [])[] | if type == "object" then (.oid // .commit.oid // "") else tostring end | select(. != "")];

    (. // []) as $all
    | [$all[] | select(((.state // "OPEN") | ascii_upcase) == "OPEN")] as $open
    | [$open[] | .number] as $open_nums
    | [$open[] | {number: .number, headRefOid: (.headRefOid // ""), headRefName: (.headRefName // "")}] as $open_heads
    | [
        $open[]
        | . as $pr
        | (required_checks_summary) as $req
        | (body_dep_numbers | map(. as $dep | select(($dep != $pr.number) and (($open_nums | index($dep)) != null)))) as $open_body_deps
        | (commit_oids) as $oids
        | ([$open_heads[]
            | . as $oh
            | select($oh.number != $pr.number and $oh.headRefOid != "" and (($oids | index($oh.headRefOid)) != null))
            | $oh.number]) as $open_commit_deps
        | (if ($pr.stacked // false) == true then
             "explicit stacked flag"
           elif ($pr.baseRefName // "main") != "main" then
             "base is " + ($pr.baseRefName // "") + " (not main)"
           elif ($open_body_deps | length) > 0 then
             "depends on open PR #" + ($open_body_deps | map(tostring) | join(", #"))
           elif ($open_commit_deps | length) > 0 then
             "contains head commit of open PR #" + ($open_commit_deps | map(tostring) | join(", #"))
           else
             null
           end) as $stack_reason
        | (has_label("hotfix")) as $is_hotfix
        | (has_label("train:solo")
           or (($pr.title // "") | startswith("release:"))
           or (($pr.headRefName // "") | startswith("release/"))) as $is_solo
        | (if $is_hotfix then 0 elif $is_solo then 1 else 2 end) as $tier
        | (if $is_hotfix and $is_solo then "hotfix+solo"
           elif $is_hotfix then "hotfix"
           elif $is_solo then "solo"
           else "normal" end) as $lane
        | {
            number: $pr.number,
            title: ($pr.title // ""),
            headRefName: ($pr.headRefName // ""),
            headRefOid: ($pr.headRefOid // ""),
            baseRefName: ($pr.baseRefName // "main"),
            isDraft: ($pr.isDraft // false),
            createdAt: ($pr.createdAt // ""),
            labels: label_names,
            has_ready: has_label("train:ready"),
            has_conflict: has_label("train:conflict"),
            has_failed: has_label("train:failed"),
            mergeable: ($pr.mergeable // "MERGEABLE"),
            checks_ok: $req.ok,
            checks_state: $req.state,
            stack_reason: $stack_reason,
            is_hotfix: $is_hotfix,
            is_solo: $is_solo,
            tier: $tier,
            lane: $lane
          }
        | . + {
            skip_reason: (
              if .isDraft then "draft"
              elif (.has_ready | not) then "missing train:ready"
              elif .has_conflict then "labelled train:conflict"
              elif .has_failed then "labelled train:failed"
              elif .stack_reason != null then ("stacked: " + .stack_reason)
              elif .mergeable == "CONFLICTING" then "mergeable=CONFLICTING"
              elif (.checks_ok | not) then ("own required checks not green (" + .checks_state + ")")
              else null
              end
            )
          }
      ] as $evaluated
    | [$evaluated[] | select(.skip_reason == null)]
      | sort_by([.tier, .createdAt, .number]) as $eligible
    | (if ($eligible | length) == 0 then
         []
       elif $eligible[0].is_solo then
         [$eligible[0]]
       else
         [$eligible[] | select(.is_solo | not)][0:$max_batch]
       end) as $selected
    | {
        total_open: ($open | length),
        draft_count: ([$evaluated[] | select(.isDraft)] | length),
        non_draft_count: ([$evaluated[] | select(.isDraft | not)] | length),
        ready_count: ([$evaluated[] | select(.has_ready)] | length),
        selected: $selected,
        skipped: [$evaluated[] | select(.has_ready and .skip_reason != null)],
        non_ready_non_draft: [$evaluated[] | select((.isDraft | not) and (.has_ready | not))]
      }
  '
}

# Given JSON `{"failing_crates": ["designos-doc", ...], "prs": [{"number": 1, "files": ["crates/designos-doc/src/lib.rs"]}]}`,
# identify whether exactly one PR's changed files intersect the failing crate(s).
attribute_failure_json() {
  jq '
    def file_crate:
      if test("^crates/[a-zA-Z0-9_-]+/") then
        capture("^crates/(?<c>[a-zA-Z0-9_-]+)/").c
      else
        null
      end;

    ([(.failing_crates // [])[] | select(. != "") | gsub("_"; "-")] | unique) as $failing
    | [(.prs // [])[]
        | . as $pr
        | ([($pr.files // [])[] | file_crate | select(. != null) | gsub("_"; "-")] | unique) as $crates
        | ([$crates[] | . as $c | select(($failing | index($c)) != null)] | unique) as $overlap
        | {
            number: $pr.number,
            crates: $crates,
            overlap: $overlap
          }
      ] as $by_pr
    | [$by_pr[] | select((.overlap | length) > 0)] as $matching
    | {
        failing_crates: $failing,
        intersecting_prs: [$matching[].number],
        culprit: (if ($failing | length) > 0 and ($matching | length) == 1 then $matching[0].number else null end),
        details: $by_pr
      }
  '
}

# Evaluate the 4 required check-runs on a commit SHA.
# Prints:
#   SUCCESS
#   PENDING:<names>
#   FAIL:<names>
required_state_on_sha() {
  local sha="$1"
  local out
  out=$(gh api "repos/$REPO/commits/$sha/check-runs?per_page=100" 2>/dev/null) || {
    printf 'PENDING:api-error\n'
    return 0
  }
  printf '%s' "$out" | jq -r '
    . as $root
    | (["Check", "Test (macos-15)", "Clippy", "Rustfmt"] | map(
        . as $req
        | [($root.check_runs // [])[] | select(.name == $req)] as $runs
        | if ($runs | length) == 0 then
            "PENDING:" + $req
          else
            ($runs | last) as $last
            | (($last.status // "") | ascii_downcase) as $st
            | (($last.conclusion // "") | ascii_downcase) as $conc
            | if $st != "completed" then
                "PENDING:" + $req
              elif ($conc == "success" or $conc == "skipped") then
                "SUCCESS"
              else
                "FAIL:" + $req
              end
          end
      )) as $states
    | ($states | map(select(startswith("FAIL:"))) | map(sub("^FAIL:"; ""))) as $failed
    | ($states | map(select(startswith("PENDING:"))) | map(sub("^PENDING:"; ""))) as $pending
    | if ($failed | length) > 0 then
        "FAIL:" + ($failed | join(", "))
      elif ($pending | length) > 0 then
        "PENDING:" + ($pending | join(", "))
      else
        "SUCCESS"
      end
  '
}

ensure_train_labels() {
  if [ "$DRY_RUN" = "1" ]; then
    return 0
  fi
  local spec name color desc encoded
  for spec in \
    "train:ready|0e8a16|Approved and ready for the batch merge train" \
    "train:solo|1d76db|Run alone in the batch merge train (e.g. release PR)" \
    "hotfix|b60205|Urgent hotfix prioritized at the front of the merge train" \
    "train:failed|d93f0b|Failed checks on the batch merge train" \
    "train:conflict|e4e669|Merge conflict when building the batch merge train branch"
  do
    IFS='|' read -r name color desc <<< "$spec"
    encoded="${name//:/%3A}"
    gh api "repos/$REPO/labels/$encoded" >/dev/null 2>&1 || \
      gh api "repos/$REPO/labels" -X POST -f name="$name" -f color="$color" -f description="$desc" >/dev/null 2>&1 || true
  done
}

extract_failing_crates() {
  local sha="$1"
  if [ -n "${FAILING_CRATES:-}" ]; then
    printf '%s\n' "$FAILING_CRATES" | tr ', ' '\n\n' | sed '/^$/d' | sort -u
    return 0
  fi
  local runs_json failed_ids id ann_json log_text
  runs_json=$(gh api "repos/$REPO/commits/$sha/check-runs?per_page=100" 2>/dev/null || printf '{"check_runs":[]}')
  failed_ids=$(printf '%s' "$runs_json" | jq -r '
    (.check_runs // [])[]
    | select(((.status // "") | ascii_downcase) == "completed"
             and ((.conclusion // "") | ascii_downcase) != "success"
             and ((.conclusion // "") | ascii_downcase) != "skipped"
             and ((.conclusion // "") | ascii_downcase) != "neutral")
    | .id
  ')
  {
    while IFS= read -r id; do
      [ -z "$id" ] && continue
      ann_json=$(gh api "repos/$REPO/check-runs/$id/annotations?per_page=100" 2>/dev/null || printf '[]')
      printf '%s' "$ann_json" | jq -r '.[] | (.path // ""), (.message // ""), (.title // "")' 2>/dev/null || true
      log_text=$(gh api "repos/$REPO/actions/jobs/$id/logs" 2>/dev/null || true)
      if [ -n "$log_text" ]; then
        printf '%s\n' "$log_text" | grep -E '(FAIL \[|--> crates/|Diff in .*crates/|error: could not compile)' || true
      fi
    done <<< "$failed_ids"
  } | sed -E 's/designos_([a-z0-9]+)/designos-\1/g' \
    | grep -Eo '(crates/[a-z0-9_-]+|designos-[a-z0-9_-]+)' \
    | sed 's|^crates/||' \
    | while IFS= read -r crate; do
        if [ -d "crates/$crate" ]; then
          printf '%s\n' "$crate"
        fi
      done \
    | sort -u
}

delete_train_branch() {
  local branch="$1"
  if [ "$DRY_RUN" = "1" ] || [ -z "$branch" ]; then
    return 0
  fi
  git push origin --delete "$branch" >/dev/null 2>&1 || true
}

run_live_batch() {
  local selected_json="$1"
  local retried=0

  ensure_train_labels

  local current_prs_json="$selected_json"
  while :; do
    local count
    count=$(printf '%s' "$current_prs_json" | jq 'length')
    if [ "$count" -eq 0 ]; then
      log "no PRs remain in batch; stopping"
      return 0
    fi

    local ts train_branch
    ts=$(date -u +%Y%m%dT%H%M%SZ)
    train_branch="train/$ts"

    log "fetching origin/main and building $train_branch for $count PR(s)"
    git fetch origin main
    git checkout -B "$train_branch" origin/main

    local merged_entries="[]"
    local idx=0
    while [ "$idx" -lt "$count" ]; do
      local pr sha title
      pr=$(printf '%s' "$current_prs_json" | jq -r ".[$idx].number")
      sha=$(printf '%s' "$current_prs_json" | jq -r ".[$idx].headRefOid")
      title=$(printf '%s' "$current_prs_json" | jq -r ".[$idx].title")
      idx=$((idx + 1))

      log "fetching PR #$pr head $sha ($title)"
      if ! git fetch origin "refs/pull/$pr/head" >/dev/null 2>&1; then
        log "WARNING: could not fetch refs/pull/$pr/head; skipping #$pr"
        continue
      fi
      local fetched_sha
      fetched_sha=$(git rev-parse FETCH_HEAD)
      if [ "$fetched_sha" != "$sha" ]; then
        log "PR #$pr head moved ($sha -> $fetched_sha); skipping for this train cycle"
        continue
      fi

      if git merge --no-ff -m "train: merge #$pr ($sha)" "$sha" >/dev/null 2>&1; then
        if git grep -nE '^(<<<<<<<|=======$|>>>>>>>)' -- . >/dev/null 2>&1; then
          log "conflict markers detected after merging #$pr; aborting merge of #$pr"
          git reset --hard HEAD~1
          gh pr edit "$pr" -R "$REPO" --add-label "train:conflict" --remove-label "train:ready" >/dev/null 2>&1 || true
          gh pr comment "$pr" -R "$REPO" --body "Batch train dropped #$pr (\`${sha:0:9}\`) because conflict markers remained after merging into \`$train_branch\`. Added \`train:conflict\` and removed \`train:ready\`." >/dev/null 2>&1 || true
          continue
        fi
        log "merged #$pr (${sha:0:9}) into $train_branch"
        merged_entries=$(printf '%s' "$merged_entries" | jq --argjson n "$pr" --arg s "$sha" --arg t "$title" '. + [{number: $n, headRefOid: $s, title: $t}]')
      else
        log "CONFLICT merging #$pr (${sha:0:9}) into $train_branch; dropping #$pr and labelling train:conflict"
        git merge --abort >/dev/null 2>&1 || git reset --hard HEAD
        gh pr edit "$pr" -R "$REPO" --add-label "train:conflict" --remove-label "train:ready" >/dev/null 2>&1 || true
        gh pr comment "$pr" -R "$REPO" --body "Batch train dropped #$pr (\`${sha:0:9}\`) due to a merge conflict when building \`$train_branch\`. Added \`train:conflict\` and removed \`train:ready\`; please rebase on \`main\` and re-add \`train:ready\`." >/dev/null 2>&1 || true
      fi
    done

    local merged_count
    merged_count=$(printf '%s' "$merged_entries" | jq 'length')
    if [ "$merged_count" -eq 0 ]; then
      log "no PRs merged cleanly into $train_branch; nothing to push"
      return 0
    fi

    local train_sha
    train_sha=$(git rev-parse HEAD)
    log "pushing $train_branch ($train_sha) with $merged_count PR(s)"
    git push origin "HEAD:refs/heads/$train_branch"

    # Wait briefly for GitHub Actions to register check-runs on the pushed train branch.
    sleep 20

    local elapsed=0
    local st="PENDING"
    while [ "$elapsed" -lt "$POLL_TIMEOUT" ]; do
      st=$(required_state_on_sha "$train_sha")
      case "$st" in
        SUCCESS|FAIL:*)
          break
          ;;
        *)
          log "waiting on $train_branch (${train_sha:0:9}): $st (elapsed ${elapsed}s / ${POLL_TIMEOUT}s)"
          sleep "$POLL_INTERVAL"
          elapsed=$((elapsed + POLL_INTERVAL))
          ;;
      esac
    done

    if [ "$st" = "SUCCESS" ]; then
      local batch_refs
      batch_refs=$(printf '%s' "$merged_entries" | jq -r '[.[].number | "#" + tostring] | join(" ")')
      log "train $train_branch (${train_sha:0:9}) GREEN for $batch_refs; squash-merging in order"

      local m_idx=0
      while [ "$m_idx" -lt "$merged_count" ]; do
        local pr sha
        pr=$(printf '%s' "$merged_entries" | jq -r ".[$m_idx].number")
        sha=$(printf '%s' "$merged_entries" | jq -r ".[$m_idx].headRefOid")
        m_idx=$((m_idx + 1))

        if gh pr merge "$pr" -R "$REPO" --squash --match-head-commit "$sha"; then
          local landed_sha
          landed_sha=$(gh pr view "$pr" -R "$REPO" --json mergeCommit --jq '.mergeCommit.oid // empty' 2>/dev/null || true)
          if [ -z "$landed_sha" ]; then
            git fetch origin main >/dev/null 2>&1 || true
            landed_sha=$(git rev-parse origin/main)
          fi
          gh pr edit "$pr" -R "$REPO" --remove-label "train:ready" >/dev/null 2>&1 || true
          gh pr comment "$pr" -R "$REPO" --body "landed in ${landed_sha} with ${batch_refs}" >/dev/null 2>&1 || true
          log "MERGED #$pr in ${landed_sha:0:9} (batch: $batch_refs)"
        else
          delete_train_branch "$train_branch"
          die "gh pr merge refused for #$pr (head $sha); stopping without --admin"
        fi
      done

      delete_train_branch "$train_branch"
      log "batch train completed: $batch_refs"
      return 0
    fi

    # Red or timed-out train run.
    local failing_checks
    if [[ "$st" == FAIL:* ]]; then
      failing_checks="${st#FAIL:}"
    else
      failing_checks="timeout after ${POLL_TIMEOUT}s ($st)"
    fi
    log "train $train_branch (${train_sha:0:9}) RED: failing checks: $failing_checks"

    local failing_crates_list
    failing_crates_list=$(extract_failing_crates "$train_sha" || true)
    delete_train_branch "$train_branch"

    if [ "$merged_count" -gt 1 ] && [ "$retried" -eq 0 ] && [ -n "$failing_crates_list" ]; then
      local attr_input="{\"failing_crates\": [], \"prs\": []}"
      attr_input=$(printf '%s\n' "$failing_crates_list" | jq -R . | jq -s '{failing_crates: ., prs: []}')

      local p_idx=0
      while [ "$p_idx" -lt "$merged_count" ]; do
        local pr sha files_json
        pr=$(printf '%s' "$merged_entries" | jq -r ".[$p_idx].number")
        sha=$(printf '%s' "$merged_entries" | jq -r ".[$p_idx].headRefOid")
        p_idx=$((p_idx + 1))
        files_json=$(git diff --name-only "origin/main...$sha" 2>/dev/null | jq -R . | jq -s .)
        attr_input=$(printf '%s' "$attr_input" | jq --argjson n "$pr" --argjson f "$files_json" '.prs += [{number: $n, files: $f}]')
      done

      local attr_result culprit
      attr_result=$(printf '%s' "$attr_input" | attribute_failure_json)
      culprit=$(printf '%s' "$attr_result" | jq -r '.culprit // empty')

      if [ -n "$culprit" ]; then
        local crates_inline
        crates_inline=$(printf '%s' "$attr_result" | jq -r '.failing_crates | join(", ")')
        log "isolated failure in crate(s) [$crates_inline] to single PR #$culprit; labelling #$culprit train:failed and rerunning remaining PRs once"
        gh pr edit "$culprit" -R "$REPO" --add-label "train:failed" --remove-label "train:ready" >/dev/null 2>&1 || true
        gh pr comment "$culprit" -R "$REPO" --body "Batch train failed on \`$train_branch\` (\`${train_sha:0:9}\`). Failing check(s): ${failing_checks} (crate(s): ${crates_inline}). Added \`train:failed\` and removed \`train:ready\`; rerunning the remaining batch once." >/dev/null 2>&1 || true
        current_prs_json=$(printf '%s' "$merged_entries" | jq --argjson c "$culprit" '[.[] | select(.number != $c)]')
        retried=1
        continue
      fi
    fi

    log "cannot isolate failure to a single PR (failing checks: $failing_checks); labelling all $merged_count PR(s) in batch train:failed and stopping"
    local f_idx=0
    while [ "$f_idx" -lt "$merged_count" ]; do
      local pr
      pr=$(printf '%s' "$merged_entries" | jq -r ".[$f_idx].number")
      f_idx=$((f_idx + 1))
      gh pr edit "$pr" -R "$REPO" --add-label "train:failed" --remove-label "train:ready" >/dev/null 2>&1 || true
      gh pr comment "$pr" -R "$REPO" --body "Batch train failed on \`$train_branch\` (\`${train_sha:0:9}\`). Failing check(s): ${failing_checks}. Added \`train:failed\` and removed \`train:ready\`." >/dev/null 2>&1 || true
    done
    return 1
  done
}

main() {
  local mode="run"
  local json_file=""

  while [ "$#" -gt 0 ]; do
    case "$1" in
      --select-json)
        mode="select-json"
        json_file="${2:-}"
        shift 2
        ;;
      --attribute-failure-json)
        mode="attribute-failure-json"
        json_file="${2:-}"
        shift 2
        ;;
      --dry-run)
        DRY_RUN=1
        shift
        ;;
      --max-batch)
        MAX_BATCH="${2:-5}"
        shift 2
        ;;
      *)
        die "unknown argument: $1"
        ;;
    esac
  done

  if ! [[ "$MAX_BATCH" =~ ^[1-9][0-9]*$ ]]; then
    die "MAX_BATCH must be a positive integer (got '$MAX_BATCH')"
  fi

  if [ "$mode" = "select-json" ]; then
    [ -n "$json_file" ] || die "--select-json requires a file path or '-'"
    if [ "$json_file" = "-" ]; then
      select_and_order_json "$MAX_BATCH"
    else
      select_and_order_json "$MAX_BATCH" < "$json_file"
    fi
    return 0
  fi

  if [ "$mode" = "attribute-failure-json" ]; then
    [ -n "$json_file" ] || die "--attribute-failure-json requires a file path or '-'"
    if [ "$json_file" = "-" ]; then
      attribute_failure_json
    else
      attribute_failure_json < "$json_file"
    fi
    return 0
  fi

  if [ "$DRY_RUN" = "0" ] && [ "${GITHUB_ACTIONS:-}" = "true" ] && [ -z "${TRAIN_TOKEN:-}" ]; then
    die "TRAIN_TOKEN secret is missing; cannot run live merge train writes without secrets.TRAIN_TOKEN"
  fi

  log "starting batch merge train (repo=$REPO, DRY_RUN=$DRY_RUN, MAX_BATCH=$MAX_BATCH, required=[${REQUIRED[*]}])"

  local raw_prs_json
  if [ -n "${PR_LIST_JSON_FILE:-}" ]; then
    raw_prs_json=$(cat "$PR_LIST_JSON_FILE")
  else
    raw_prs_json=$(gh pr list -R "$REPO" --state open --limit 100 \
      --json number,title,state,isDraft,baseRefName,headRefName,headRefOid,createdAt,labels,body,mergeable,statusCheckRollup)
  fi

  local plan_json
  plan_json=$(printf '%s' "$raw_prs_json" | select_and_order_json "$MAX_BATCH")

  local total_open draft_count non_draft_count ready_count selected_count skipped_count
  total_open=$(printf '%s' "$plan_json" | jq -r '.total_open')
  draft_count=$(printf '%s' "$plan_json" | jq -r '.draft_count')
  non_draft_count=$(printf '%s' "$plan_json" | jq -r '.non_draft_count')
  ready_count=$(printf '%s' "$plan_json" | jq -r '.ready_count')
  selected_count=$(printf '%s' "$plan_json" | jq -r '.selected | length')
  skipped_count=$(printf '%s' "$plan_json" | jq -r '.skipped | length')

  log "scanned $total_open open PR(s): $non_draft_count non-draft, $draft_count draft, $ready_count labelled train:ready"

  if [ "$skipped_count" -gt 0 ]; then
    while IFS= read -r line; do
      [ -z "$line" ] && continue
      log "skip $line"
    done < <(printf '%s' "$plan_json" | jq -r '.skipped[] | "#\(.number) (\(.headRefOid[0:9])): \(.skip_reason) — \(.title)"')
  fi

  if [ "$DRY_RUN" = "1" ] && [ "$ready_count" -eq 0 ] && [ "$non_draft_count" -gt 0 ]; then
    log "non-draft open PRs without train:ready ($non_draft_count):"
    while IFS= read -r line; do
      [ -z "$line" ] && continue
      log "  unlabelled $line"
    done < <(printf '%s' "$plan_json" | jq -r '.non_ready_non_draft[] | "#\(.number) (\(.headRefOid[0:9])) base=\(.baseRefName) checks=\(.checks_state) — \(.title)"')
  fi

  if [ "$selected_count" -eq 0 ]; then
    log "selected batch: (none) — no eligible train:ready PRs to merge"
    return 0
  fi

  local batch_refs
  batch_refs=$(printf '%s' "$plan_json" | jq -r '[.selected[].number | "#" + tostring] | join(" ")')
  log "selected $selected_count PR(s) in order: $batch_refs"
  while IFS= read -r line; do
    [ -z "$line" ] && continue
    log "  order $line"
  done < <(printf '%s' "$plan_json" | jq -r '.selected | to_entries[] | "[\(.key + 1)] #\(.value.number) (\(.value.headRefOid[0:9])) lane=\(.value.lane) created=\(.value.createdAt) — \(.value.title)"')

  if [ "$DRY_RUN" = "1" ]; then
    local ts_preview="train/$(date -u +%Y%m%dT%H%M%SZ)"
    log "[dry-run] would create branch $ts_preview from origin/main (not pushing in dry_run)"
    while IFS= read -r line; do
      [ -z "$line" ] && continue
      log "[dry-run] $line"
    done < <(printf '%s' "$plan_json" | jq -r --arg br "$ts_preview" '.selected[] | "would run: git merge --no-ff \(.headRefOid) into \($br) (#\(.number): \(.title))"')
    log "[dry-run] would push $ts_preview and wait for required contexts (Check, Test (macos-15), Clippy, Rustfmt) (poll ${POLL_INTERVAL}s, timeout ${POLL_TIMEOUT}s)"
    while IFS= read -r line; do
      [ -z "$line" ] && continue
      log "[dry-run] $line"
    done < <(printf '%s' "$plan_json" | jq -r --arg refs "$batch_refs" '.selected[] | "on green: would run: gh pr merge \(.number) --squash --match-head-commit \(.headRefOid), remove label train:ready, and comment \"landed in <sha> with \($refs)\""')
    log "[dry-run] would delete remote branch $ts_preview"
    return 0
  fi

  local selected_only
  selected_only=$(printf '%s' "$plan_json" | jq -c '.selected')
  run_live_batch "$selected_only"
}

main "$@"
