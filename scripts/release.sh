#!/bin/sh
# Cuts a release: sets the version in the repository and commits it, so
# what is shipped and what a local build calls itself are the same version.
#
#   scripts/release.sh patch|minor|major|<x.y.z>            a pull request
#   scripts/release.sh patch|minor|major|<x.y.z> --direct   commit, tag, push main
#   scripts/release.sh ... --direct --no-push               the same, kept local
#
# From an up-to-date, clean main. The pull request, once merged, is
# released by the "Auto release" workflow; --direct pushes the
# tag itself, which starts the Release workflow.
set -eu
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

WHAT="${1:-}"
PUSH=1
DIRECT=0
[ -n "$WHAT" ] || { echo "usage: scripts/release.sh patch|minor|major|<x.y.z> [--direct [--no-push]]" >&2; exit 2; }
shift
for FLAG in "$@"; do
  case "$FLAG" in
    --direct) DIRECT=1 ;;
    --no-push) PUSH=0 ;;
    *) echo "unknown option: $FLAG" >&2; exit 2 ;;
  esac
done

NOW="$(scripts/version.sh)"
IFS=. read -r MAJOR MINOR PATCH <<EOV
$NOW
EOV
case "$WHAT" in
  patch) NEXT="$MAJOR.$MINOR.$((PATCH + 1))" ;;
  minor) NEXT="$MAJOR.$((MINOR + 1)).0" ;;
  major) NEXT="$((MAJOR + 1)).0.0" ;;
  v*) NEXT="${WHAT#v}" ;;
  *) NEXT="$WHAT" ;;
esac
echo "$NEXT" | grep -Eq '^[0-9]+\.[0-9]+\.[0-9]+$' || { echo "not a version: $NEXT" >&2; exit 2; }

BRANCH="$(git rev-parse --abbrev-ref HEAD)"
[ "$BRANCH" = "main" ] || { echo "releases are cut from main (on $BRANCH)" >&2; exit 1; }
# Files of this repository only: untracked ones (vendor/) do not count.
[ -z "$(git status --porcelain --untracked-files=no)" ] || { echo "uncommitted changes; commit or stash them first" >&2; exit 1; }
git fetch --quiet origin main --tags
[ "$(git rev-parse HEAD)" = "$(git rev-parse origin/main)" ] || { echo "main is not at origin/main; pull or push first" >&2; exit 1; }
git rev-parse -q --verify "refs/tags/v$NEXT" >/dev/null && { echo "v$NEXT is tagged already" >&2; exit 1; }

[ "$DIRECT" = 1 ] || git checkout --quiet -b "release-$NEXT"
scripts/set-version.sh "$NEXT"
# Cargo.lock names the crate's version too.
cargo update --quiet --offline --package sophon 2>/dev/null || cargo check --quiet
git add nsh.json Cargo.toml Cargo.lock
git commit --quiet -m "Release $NEXT"

if [ "$DIRECT" = 0 ]; then
  git push --quiet -u origin "release-$NEXT"
  gh pr create --base main --title "Release $NEXT" --body "Sets the version to $NEXT. Merging this starts the release."
  git checkout --quiet main
  echo "$NOW -> $NEXT: merge the pull request to release"
  exit 0
fi

git tag "v$NEXT"
echo "$NOW -> $NEXT: committed and tagged v$NEXT"
if [ "$PUSH" = 1 ]; then
  # Together: the tag starts the Release workflow, on a commit main has.
  git push --atomic origin main "v$NEXT"
  echo "pushed; the Release workflow is building v$NEXT"
else
  echo "not pushed: git push --atomic origin main v$NEXT"
fi
