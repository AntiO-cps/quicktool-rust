#!/usr/bin/env bash
# Build QuickTool Rust for macOS (Apple Silicon by default) and package
# QuickTool.app plus a distributable DMG. Run on a real Mac or a macOS CI
# runner: the final link embeds the macOS SDK, which Apple only ships there.
#
# Usage:
#   bash scripts/Build-Rust-macOS.sh                 # aarch64 (M series)
#   ARCH=x86_64 bash scripts/Build-Rust-macOS.sh     # Intel
set -euo pipefail

project="$(cd "$(dirname "$0")/.." && pwd)"
arch="${ARCH:-aarch64}"
target="${arch}-apple-darwin"
version="$(sed -n 's/^version = "\(.*\)"$/\1/p' "$project/Cargo.toml" | head -1)"
if [ -z "$version" ]; then echo "Cargo version missing" >&2; exit 1; fi

cd "$project"
if [ "$arch" != "$(uname -m)" ]; then
  rustup target add "$target"
  cargo build --release --bin quicktool-rust --target "$target"
  exe="target/$target/release/quicktool-rust"
else
  cargo build --release --bin quicktool-rust
  exe="target/release/quicktool-rust"
fi

app="target/QuickTool.app"
rm -rf "$app"
mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources"
sed "s/{{VERSION}}/$version/; s/{{ARCH}}/$arch/" \
  "$project/scripts/macos/Info.plist.in" > "$app/Contents/Info.plist"
cp "$exe" "$app/Contents/MacOS/QuickTool"
cp "$project/assets/assets/quicktool.icns" "$app/Contents/Resources/quicktool.icns"
# The app resolves assets beside the executable, mirroring the Windows layout.
cp -R "$project/assets/licenses" "$app/Contents/MacOS/assets-licenses-tmp"
mkdir -p "$app/Contents/MacOS/assets/assets"
mv "$app/Contents/MacOS/assets-licenses-tmp" "$app/Contents/MacOS/assets/licenses"
cp "$project/assets/assets/quicktool.ico" "$app/Contents/MacOS/assets/assets/"
mkdir -p "$app/Contents/MacOS/assets/packages"
for module in "$project/assets/modules/"*/; do
  cp -R "$module" "$app/Contents/MacOS/assets/packages/$(basename "$module")"
done
cp -R "$project/docs" "$app/Contents/MacOS/docs"
cat > "$app/Contents/MacOS/README.txt" <<'EOF'
QuickTool Rust (macOS)
Run QuickTool.app. Keep the assets directory beside the executable.
Settings and history: ~/Library/Application Support/QuickTool-Rust
This rewrite uses a native Rust UI and does not require Electron or Node.js.
Windows-only integrations (acrylic, registry search, Everything, PATH launch)
degrade gracefully; tools that cannot run report it in the panel.
EOF
# All 13 modules ship installed, matching the Windows installer default.
for module in "$project/assets/modules/"*/; do
  name="$(basename "$module")"
  mkdir -p "$app/Contents/MacOS/assets/modules"
  cp -R "$module" "$app/Contents/MacOS/assets/modules/$name"
done

# Apple Silicon requires at least an ad-hoc signature.
codesign --force --deep -s - "$app"

staging="target/dmg-staging"
rm -rf "$staging"
mkdir -p "$staging"
cp -R "$app" "$staging/QuickTool.app"
ln -s /Applications "$staging/Applications"
dmg="target/QuickTool-Rust-$version-macos-$arch.dmg"
rm -f "$dmg"
hdiutil create -volname "QuickTool Rust" -srcfolder "$staging" -ov -format UDZO "$dmg"
hdiutil verify "$dmg" >/dev/null

shasum -a 256 "$dmg" > "$dmg.sha256"
cp "$dmg" "$dmg.sha256" "$project/releases/" 2>/dev/null || {
  mkdir -p "$project/releases"; cp "$dmg" "$dmg.sha256" "$project/releases/";
}
echo "DMG: $project/releases/QuickTool-Rust-$version-macos-$arch.dmg"
