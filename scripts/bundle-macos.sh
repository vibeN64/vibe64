#!/usr/bin/env bash
# Builds VibeN64 and assembles target/VibeN64.app, without needing cargo-bundle.
#
# MoltenVK (the Vulkan-on-Metal layer) has to ship inside the app. It is taken from
# Homebrew (`brew install molten-vk`) or from the path in MOLTENVK_DYLIB.
set -euo pipefail
cd "$(dirname "$0")/.."

TOOLCHAIN=$(sed -n 's/^channel = "\(.*\)"/\1/p' rust-toolchain.toml)
HOST=$(rustc -vV | sed -n 's/^host: //p')
# .cargo/config.toml asks for llvm-ar, which is not on PATH with a stock Xcode setup
export AR="${AR:-$HOME/.rustup/toolchains/$TOOLCHAIN-$HOST/lib/rustlib/$HOST/bin/llvm-ar}"

MOLTENVK="${MOLTENVK_DYLIB:-/opt/homebrew/opt/molten-vk/lib/libMoltenVK.dylib}"
if [ ! -f "$MOLTENVK" ]; then
  echo "libMoltenVK.dylib not found at $MOLTENVK" >&2
  echo "Install it with 'brew install molten-vk' or set MOLTENVK_DYLIB." >&2
  exit 1
fi

cargo build --release

APP=target/VibeN64.app
VERSION=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)
rm -rf "$APP"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources" "$APP/Contents/Frameworks"
cp target/release/viben64 "$APP/Contents/MacOS/viben64"
cp "$MOLTENVK" "$APP/Contents/Frameworks/libMoltenVK.dylib"

ICONSET=$(mktemp -d)/viben64.iconset
mkdir -p "$ICONSET"
for size in 16 32 128 256 512; do
  sips -z $size $size data/icon/viben64.png --out "$ICONSET/icon_${size}x${size}.png" >/dev/null
  sips -z $((size * 2)) $((size * 2)) data/icon/viben64.png --out "$ICONSET/icon_${size}x${size}@2x.png" >/dev/null
done
iconutil -c icns "$ICONSET" -o "$APP/Contents/Resources/viben64.icns"

cat > "$APP/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleName</key><string>VibeN64</string>
  <key>CFBundleDisplayName</key><string>VibeN64</string>
  <key>CFBundleIdentifier</key><string>io.github.viben64.viben64</string>
  <key>CFBundleExecutable</key><string>viben64</string>
  <key>CFBundleIconFile</key><string>viben64.icns</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleInfoDictionaryVersion</key><string>6.0</string>
  <key>CFBundleShortVersionString</key><string>$VERSION</string>
  <key>CFBundleVersion</key><string>$(date +%Y%m%d.%H%M%S)</string>
  <key>LSApplicationCategoryType</key><string>public.app-category.games</string>
  <key>LSMinimumSystemVersion</key><string>15.0</string>
  <key>NSHighResolutionCapable</key><true/>
</dict>
</plist>
PLIST

# ad-hoc signature, enough to run on this Mac
codesign --force --deep --entitlements data/macos/entitlements_dev.plist -s - "$APP"
echo "Built $APP"
