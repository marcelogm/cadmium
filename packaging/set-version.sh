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

# The workspace version, which both crates inherit.
sed -i "/^\[workspace.package\]/,/^\[/s/^version = \".*\"/version = \"$version\"/" Cargo.toml

# Keep Cargo.lock in step so --locked builds still pass.
sed -i "/^name = \"cadmium-\(app\|engine\)\"\$/{n;s/^version = \".*\"/version = \"$version\"/}" Cargo.lock

# Add the release to the metainfo unless it is already listed.
if ! grep -q "<release version=\"$version\"" "$metainfo"; then
    sed -i "s|<releases>|<releases>\n    <release version=\"$version\" date=\"$(date -u +%Y-%m-%d)\"/>|" "$metainfo"
fi

grep -q "^version = \"$version\"" Cargo.toml
echo "set-version: $version"
