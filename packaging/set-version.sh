#!/bin/sh
# Stamps a version on the checkout before packaging, so a release takes its
# version from the git tag instead of a bump in Cargo.toml.
# Usage: packaging/set-version.sh 0.2.0
set -eu

version=${1:?version}
case $version in
    [0-9]*.[0-9]*.[0-9]*) ;;
    *) echo "set-version: '$version' is not a version like 1.2.3" >&2; exit 1 ;;
esac

metainfo=crates/cadmium-app/data/io.github.marcelogm.Cadmium.metainfo.xml

# The workspace version, which every workspace crate inherits.
sed -i "/^\[workspace.package\]/,/^\[/s/^version = \".*\"/version = \"$version\"/" Cargo.toml

# Keep Cargo.lock in step so --locked builds still pass: stamp each workspace
# member, named in its own manifest, and nothing else.
members=$(sed -n 's/^members = \[\(.*\)\]$/\1/p' Cargo.toml | tr -d '" ' | tr ',' ' ')
[ -n "$members" ] || { echo "set-version: no workspace members in Cargo.toml" >&2; exit 1; }
for member in $members; do
    name=$(sed -n 's/^name = "\(.*\)"$/\1/p' "$member/Cargo.toml" | head -n 1)
    sed -i "/^name = \"$name\"\$/{n;s/^version = \".*\"/version = \"$version\"/}" Cargo.lock
    grep -A1 "^name = \"$name\"\$" Cargo.lock | grep -q "^version = \"$version\"\$" ||
        { echo "set-version: $name is not in Cargo.lock" >&2; exit 1; }
done

# Add the release to the metainfo unless it is already listed.
if ! grep -q "<release version=\"$version\"" "$metainfo"; then
    sed -i "s|<releases>|<releases>\n    <release version=\"$version\" date=\"$(date -u +%Y-%m-%d)\"/>|" "$metainfo"
fi

grep -q "^version = \"$version\"" Cargo.toml
echo "set-version: $version"
