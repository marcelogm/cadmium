#!/usr/bin/env bash
# Prints the GitHub Release body for a tag: the commits since the previous tag
# and one download link per package. Needs the full git history and tags.
# Usage: packaging/release-notes.sh v0.1.0
set -euo pipefail

tag=${1:?tag}
repo="https://github.com/marcelogm/cadmium"
base="$repo/releases/download/$tag"

# Commits since the previous tag, or the whole history on the first release.
if previous=$(git describe --tags --abbrev=0 "$tag^" 2>/dev/null); then
    range="$previous..$tag"
    compare="**Full changelog:** [$previous...$tag]($repo/compare/$previous...$tag)"
else
    range="$tag"
    compare="**Full changelog:** [commits]($repo/commits/$tag)"
fi

link() {
    echo "[$1]($base/$1)"
}

echo "## Changes"
echo
git log --no-merges --reverse --format="- %s ([%h]($repo/commit/%H))" "$range"
echo
echo "$compare"
echo

cat <<NOTES
## Downloads

| System | Package | x86_64 (Intel/AMD) | arm64 |
|---|---|---|---|
| Linux | Flatpak | $(link Cadmium-x86_64.flatpak) | $(link Cadmium-aarch64.flatpak) |
| Linux | AppImage | $(link Cadmium-x86_64.AppImage) | $(link Cadmium-aarch64.AppImage) |
| Windows | Installer | $(link Cadmium-windows-x64-setup.exe) | $(link Cadmium-windows-arm64-setup.exe) |
| Windows | Portable (zip) | $(link Cadmium-windows-x64.zip) | $(link Cadmium-windows-arm64.zip) |

### How to install

- **Flatpak:** \`flatpak install --user Cadmium-x86_64.flatpak\` (needs Flathub set up, which
  provides the GNOME 51 runtime).
- **AppImage:** \`chmod +x Cadmium-x86_64.AppImage\` and run the file. Works on any
  distribution; the Mesa Vulkan drivers (AMD and Intel) are bundled.
- **Windows, installer:** run the \`setup.exe\`; Cadmium shows up in the Start menu.
- **Windows, zip:** unzip the whole folder and open \`cadmium.exe\`.

All of them need a graphics card with Vulkan. Checksums are in \`SHA256SUMS\`.
NOTES
