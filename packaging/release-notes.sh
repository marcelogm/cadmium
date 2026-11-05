#!/usr/bin/env bash
# Prints the GitHub Release body for a tag: one download link per package.
# Usage: packaging/release-notes.sh v0.1.0
set -euo pipefail

tag=${1:?tag}
base="https://github.com/marcelogm/cadmium/releases/download/$tag"

link() {
    echo "[$1]($base/$1)"
}

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
