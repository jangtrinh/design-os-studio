#!/usr/bin/env bash
# Claim or release a roadmap task on the "design:os studio — Roadmap" project in ONE step,
# so the board never shows a task as Ready while somebody is already working on it.
#
#   scripts/roadmap-claim.sh claim   <issue#> <machine> <session>
#   scripts/roadmap-claim.sh release <issue#> <machine> [Ready|In review] [handoff]
#
# claim: refuses when the task is not Ready, or another agent:* label / claim comment exists.
# Then: label agent:<machine> + claim comment + Status=In progress + Machine=<machine>,
# re-reads the comments and backs out if an earlier claim from someone else appears (first wins).
# Run it BEFORE cloning, building or any other setup.
set -euo pipefail

REPO="jangtrinh/design-os-studio"
OWNER="jangtrinh"
PROJECT_NUMBER=9
PROJECT_ID="PVT_kwHOAGoIHs4BnbrN"
STATUS_FIELD="PVTSSF_lAHOAGoIHs4BnbrNzhmAV80"
# Status option ids (macOS ships bash 3.2, so no associative arrays).
status_option() {
  case "$1" in
    Ready) echo deb0f96a ;; "In progress") echo b427e533 ;; "In review") echo 1bccb78c ;; *) echo "" ;;
  esac
}

die() { echo "roadmap-claim: $*" >&2; exit 1; }
[ $# -ge 3 ] || die "usage: $0 claim <issue#> <machine> <session> | release <issue#> <machine> [Ready|In review] [handoff]"
action=$1 issue=$2 machine=$3

item_id=$(gh api graphql -f query="query{repository(owner:\"$OWNER\",name:\"${REPO#*/}\"){issue(number:$issue){projectItems(first:10){nodes{id project{number}}}}}}" \
  --jq ".data.repository.issue.projectItems.nodes[]|select(.project.number==$PROJECT_NUMBER)|.id")
[ -n "$item_id" ] || die "#$issue is not on project $PROJECT_NUMBER"
machine_field=$(gh project field-list "$PROJECT_NUMBER" --owner "$OWNER" --format json --jq '.fields[]|select(.name=="Machine")|.id')

set_status() { gh project item-edit --id "$item_id" --project-id "$PROJECT_ID" --field-id "$STATUS_FIELD" --single-select-option-id "$(status_option "$1")" >/dev/null; }
# One small query for this issue only; listing the whole board trips GitHub's secondary rate limit.
current_status() { gh api graphql -f query="query{repository(owner:\"$OWNER\",name:\"${REPO#*/}\"){issue(number:$issue){projectItems(first:10){nodes{project{number} fieldValueByName(name:\"Status\"){... on ProjectV2ItemFieldSingleSelectValue{name}}}}}}}" \
  --jq ".data.repository.issue.projectItems.nodes[]|select(.project.number==$PROJECT_NUMBER)|.fieldValueByName.name"; }
claims() { gh issue view "$issue" --repo "$REPO" --json comments --jq '[.comments[]|select(.body|test("^claim "))|.body]'; }

case "$action" in
claim)
  session=${4:?session id required}
  status=$(current_status)
  [ "$status" = "Ready" ] || die "#$issue is '$status', not Ready — pick another task"
  others=$(gh issue view "$issue" --repo "$REPO" --json labels --jq "[.labels[].name|select(startswith(\"agent:\") and .!=\"agent:$machine\")]|join(\",\")")
  [ -z "$others" ] || die "#$issue already carries $others — pick another task"

  gh label create "agent:$machine" --repo "$REPO" --color c5def5 --force >/dev/null
  gh issue edit "$issue" --repo "$REPO" --add-label "agent:$machine" --remove-label handoff >/dev/null 2>&1 \
    || gh issue edit "$issue" --repo "$REPO" --add-label "agent:$machine" >/dev/null
  set_status "In progress"
  gh project item-edit --id "$item_id" --project-id "$PROJECT_ID" --field-id "$machine_field" --text "$machine" >/dev/null
  mine="claim $machine/$session $(date -u +%Y-%m-%dT%H:%MZ)"
  gh issue comment "$issue" --repo "$REPO" --body "$mine" >/dev/null

  # First claim since the task last became Ready wins. Earlier claims followed by a
  # release comment do not count.
  winner=$(gh issue view "$issue" --repo "$REPO" --json comments \
    --jq '[.comments[]|select(.body|test("^(claim|release) "))|.body]|(map(test("^release "))|rindex(true)) as $r|.[(if $r==null then 0 else $r+1 end):]|map(select(startswith("claim ")))|first')
  if [ "$winner" != "$mine" ]; then
    gh issue edit "$issue" --repo "$REPO" --remove-label "agent:$machine" >/dev/null
    gh issue comment "$issue" --repo "$REPO" --body "release $machine/$session (lost the claim race to: $winner)" >/dev/null
    die "lost the claim race to: $winner"
  fi
  echo "claimed #$issue for $machine/$session (Status=In progress)"
  ;;
release)
  to=${4:-Ready}
  [ -n "$(status_option "$to")" ] || die "release target must be Ready or 'In review'"
  set_status "$to"
  gh issue edit "$issue" --repo "$REPO" --remove-label "agent:$machine" >/dev/null || true
  [ "${5:-}" = "handoff" ] && gh issue edit "$issue" --repo "$REPO" --add-label handoff >/dev/null
  if [ "$to" = "Ready" ]; then
    gh project item-edit --id "$item_id" --project-id "$PROJECT_ID" --field-id "$machine_field" --clear >/dev/null
  fi
  gh issue comment "$issue" --repo "$REPO" --body "release $machine -> $to" >/dev/null
  echo "released #$issue (Status=$to)"
  ;;
*) die "unknown action '$action'" ;;
esac
