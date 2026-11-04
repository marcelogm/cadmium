#!/usr/bin/env bash
# Builds the portable Cadmium folder from an MSYS2 shell (UCRT64 or CLANGARM64).
# Usage: packaging/windows/bundle.sh <output dir>
set -euo pipefail
shopt -s nullglob

dist=${1:?output dir}
prefix=${MSYSTEM_PREFIX:?run from an MSYS2 shell}
pixbuf=lib/gdk-pixbuf-2.0/2.10.0

mkdir -p "$dist/$pixbuf" "$dist/share/glib-2.0/schemas" "$dist/share/icons"
cp target/release/cadmium.exe "$dist/"
cp -r "$prefix/$pixbuf/loaders" "$dist/$pixbuf/"
cp "$prefix/share/glib-2.0/schemas/gschemas.compiled" "$dist/share/glib-2.0/schemas/"
cp -r "$prefix/share/icons/Adwaita" "$prefix/share/icons/hicolor" "$dist/share/icons/"

for binary in "$dist/cadmium.exe" "$dist/$pixbuf"/loaders/*.dll; do ntldd -R "$binary"; done \
    | tr '\\' / | grep -io "[a-z]:/[^ ]*$prefix/bin/[^ ]*\.dll" | sort -fu \
    | while read -r dll; do cp "$(cygpath -u "$dll")" "$dist/"; done

(cd "$dist" && gdk-pixbuf-query-loaders "$pixbuf"/loaders/*.dll) \
    | sed -E 's|^"[^"]*(lib[\\/]+gdk-pixbuf-2\.0)|"\1|' > "$dist/$pixbuf/loaders.cache"
cat "$dist/$pixbuf/loaders.cache"

cat > "$dist/LEIA-ME.txt" <<'TEXT'
Cadmium - simulador de pintura a óleo

1. Descompacte esta pasta inteira em qualquer lugar.
2. Abra cadmium.exe.

O Cadmium precisa de uma placa de vídeo com Vulkan e com o driver atualizado
(NVIDIA, AMD, Intel ou Qualcomm). Mesas digitalizadoras funcionam pelo Windows Ink.
TEXT

ls "$dist"
du -sh "$dist"
