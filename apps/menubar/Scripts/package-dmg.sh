#!/usr/bin/env bash
set -euo pipefail

menubar_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
repo_root="$(cd "$menubar_dir/../.." && pwd)"

version="${1:-$(git -C "$repo_root" describe --tags --abbrev=0)}"

cargo build --release --locked -p hya-cli --target aarch64-apple-darwin --manifest-path "$repo_root/Cargo.toml"

cd "$menubar_dir"
swift build -c release --arch arm64

bin_path="$(swift build -c release --arch arm64 --show-bin-path)"

app_dir="$menubar_dir/dist/Hya.app"
rm -rf "$app_dir"
mkdir -p "$app_dir/Contents/MacOS" "$app_dir/Contents/Resources"

cp "$bin_path/Hya" "$app_dir/Contents/MacOS/Hya"
cp "$repo_root/target/aarch64-apple-darwin/release/hya" "$app_dir/Contents/MacOS/hya-cli"
cp "$menubar_dir/Resources/AppIcon.icns" "$app_dir/Contents/Resources/AppIcon.icns"
sed "s/\${HYA_VERSION}/$version/" "$menubar_dir/Resources/Info.plist" > "$app_dir/Contents/Info.plist"

codesign --force --deep --sign - "$app_dir"

hdiutil create -volname Hya -srcfolder "$app_dir" -ov -format UDZO "$menubar_dir/dist/Hya.dmg"
