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

| Sistema | Pacote | x86_64 (Intel/AMD) | arm64 |
|---|---|---|---|
| Linux | Flatpak | $(link Cadmium-x86_64.flatpak) | $(link Cadmium-aarch64.flatpak) |
| Linux | AppImage | $(link Cadmium-x86_64.AppImage) | $(link Cadmium-aarch64.AppImage) |
| Windows | Instalador | $(link Cadmium-windows-x64-setup.exe) | $(link Cadmium-windows-arm64-setup.exe) |
| Windows | Portátil (zip) | $(link Cadmium-windows-x64.zip) | $(link Cadmium-windows-arm64.zip) |

### Como instalar

- **Flatpak:** \`flatpak install --user Cadmium-x86_64.flatpak\` (precisa do Flathub configurado,
  de onde vem o runtime GNOME 51).
- **AppImage:** \`chmod +x Cadmium-x86_64.AppImage\` e abra o arquivo. Funciona em qualquer
  distribuição; os drivers Vulkan do Mesa (AMD e Intel) já vêm dentro.
- **Windows, instalador:** execute o \`setup.exe\`; o Cadmium aparece no menu Iniciar.
- **Windows, zip:** descompacte a pasta inteira e abra \`cadmium.exe\`.

Todos precisam de uma placa de vídeo com Vulkan. Os checksums estão em \`SHA256SUMS\`.
NOTES
