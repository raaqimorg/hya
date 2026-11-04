#!/usr/bin/env bash
# Regenerates Resources/AppIcon.icns from assets/icon/mark-black.svg.
# Run this by hand whenever the mark changes; the build does not call it.
set -euo pipefail

menubar_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
repo_root="$(cd "$menubar_dir/../.." && pwd)"
mark="$repo_root/assets/icon/mark-black.svg"

command -v rsvg-convert >/dev/null || { echo "generate-icon.sh needs rsvg-convert (brew install librsvg)" >&2; exit 1; }
command -v iconutil >/dev/null || { echo "generate-icon.sh needs iconutil (macOS only)" >&2; exit 1; }

work_dir="$(mktemp -d)"
trap 'rm -rf "$work_dir"' EXIT
iconset="$work_dir/AppIcon.iconset"
mkdir -p "$iconset"

# name:size pairs for a standard macOS .iconset.
for entry in \
    icon_16x16:16 \
    icon_16x16@2x:32 \
    icon_32x32:32 \
    icon_32x32@2x:64 \
    icon_128x128:128 \
    icon_128x128@2x:256 \
    icon_256x256:256 \
    icon_256x256@2x:512 \
    icon_512x512:512 \
    icon_512x512@2x:1024 \
; do
    name="${entry%%:*}"
    size="${entry##*:}"
    rsvg-convert -w "$size" -h "$size" --background-color=white "$mark" -o "$iconset/$name.png"
done

iconutil -c icns "$iconset" -o "$menubar_dir/Resources/AppIcon.icns"
echo "Wrote $menubar_dir/Resources/AppIcon.icns"
