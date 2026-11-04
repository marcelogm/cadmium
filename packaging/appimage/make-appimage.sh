#!/bin/sh
# Builds dist/Cadmium-<arch>.AppImage with every library bundled, glibc and Mesa included,
# so it runs on any Linux distro. Runs inside ghcr.io/pkgforge-dev/archlinux after
# pkgforge-dev/anylinux-setup-action, which puts quick-sharun and get-debloated-pkgs on PATH.
set -eu

data=crates/cadmium-app/data
app_id=io.github.marcelogm.Cadmium

ARCH=$(uname -m)
VERSION=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -n1)
export ARCH VERSION
export OUTPATH=./dist
export OUTNAME=Cadmium-"$ARCH".AppImage
export DESKTOP=/usr/share/applications/$app_id.desktop
export ICON=/usr/share/icons/hicolor/scalable/apps/$app_id.svg
export STARTUPWMCLASS=$app_id
export GTK_FIX_NONSENSE=1
export DEPLOY_VULKAN=1

pacman -Syu --noconfirm --needed rust pkgconf gtk4 libadwaita vulkan-icd-loader
# Debloated Mesa only: the debloated gtk4-mini is built without the Vulkan renderer,
# which Cadmium needs to hand frames to GTK as dmabufs.
get-debloated-pkgs --add-mesa --prefer-nano

cargo build --release --locked -p cadmium-app

install -Dm755 target/release/cadmium /usr/bin/cadmium
install -Dm644 "$data/$app_id.desktop" "$DESKTOP"
install -Dm644 "$data/app-icons/$app_id.svg" "$ICON"
install -Dm644 "$data/$app_id.metainfo.xml" /usr/share/metainfo/"$app_id".metainfo.xml

quick-sharun /usr/bin/cadmium
quick-sharun --make-appimage
quick-sharun --test ./dist/"$OUTNAME"
