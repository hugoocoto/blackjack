#!/bin/sh
# Releases a new version: asks for it (suggesting the next patch), bumps
# Cargo.toml and Cargo.lock, checks the build, then commits, tags vX.Y.Z and
# pushes both. The tag starts the Release workflow (GitHub release, crates.io,
# itch.io).
#
# Run it from main with a clean working tree.
set -e

root=$(cd "$(dirname "$0")" && pwd)
cd "$root"

branch=$(git rev-parse --abbrev-ref HEAD)
if [ "$branch" != main ]; then
    echo "On branch $branch; release from main." >&2
    exit 1
fi
if [ -n "$(git status --porcelain)" ]; then
    echo "The working tree has changes; commit or stash them first:" >&2
    git status --short >&2
    exit 1
fi

current=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)
major=${current%%.*}
rest=${current#*.}
minor=${rest%%.*}
patch=${rest#*.}
suggested="$major.$minor.$((patch + 1))"

printf 'Current version is %s. New version [%s]: ' "$current" "$suggested"
read -r version
version=${version:-$suggested}
version=${version#v}

if ! echo "$version" | grep -Eq '^[0-9]+\.[0-9]+\.[0-9]+$'; then
    echo "$version is not a version like 1.2.3." >&2
    exit 1
fi
if [ "$version" = "$current" ] ||
    [ "$(printf '%s\n%s\n' "$current" "$version" | sort -V | tail -1)" != "$version" ]; then
    echo "$version is not newer than $current." >&2
    exit 1
fi
if git rev-parse -q --verify "refs/tags/v$version" >/dev/null; then
    echo "Tag v$version already exists." >&2
    exit 1
fi

# Google Play's version code: 1.2.3 is 10203.
code=$(echo "$version" | awk -F. '$2 < 100 && $3 < 100 { print $1 * 10000 + $2 * 100 + $3 }')
if [ -z "$code" ]; then
    echo "$version has a part over 99, which the Android version code can't hold." >&2
    exit 1
fi

sed -i "0,/^version = \"$current\"/s//version = \"$version\"/" Cargo.toml
sed -i "s/^version_code = [0-9]*/version_code = $code/" Cargo.toml
cargo update -p hushjack --offline
# The release builds use --locked, so check that the lockfile is up to date.
cargo check --locked

git --no-pager diff
printf 'Commit "Release %s", tag v%s and push? [y/N] ' "$version" "$version"
read -r answer
case $answer in
    y | Y) ;;
    *)
        git checkout -- Cargo.toml Cargo.lock
        echo "Cancelled; Cargo.toml and Cargo.lock are back to $current."
        exit 1
        ;;
esac

git add Cargo.toml Cargo.lock
git commit -m "Release $version"
git tag "v$version"
# Both or neither, so a tag never points at a commit missing from main.
git push --atomic origin main "v$version"
echo "Pushed v$version; the Release workflow is at https://github.com/hugoocoto/blackjack/actions"
