#!/bin/zsh
set -eu -o pipefail
HERE="${0:A:h}"
APP="$HERE/target/Claude Swap.app"
cargo build --release --manifest-path "$HERE/Cargo.toml"
rm -rf "$APP"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"
cp "$HERE/target/release/cswap-gui" "$APP/Contents/MacOS/cswap-gui"
cp "$HERE/assets/AppIcon.icns" "$APP/Contents/Resources/AppIcon.icns"
cat > "$APP/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleName</key><string>Claude Swap</string>
  <key>CFBundleDisplayName</key><string>Claude Swap</string>
  <key>CFBundleIdentifier</key><string>dev.claude-swap.gui</string>
  <key>CFBundleVersion</key><string>0.1.0</string>
  <key>CFBundleShortVersionString</key><string>0.1.0</string>
  <key>CFBundleExecutable</key><string>cswap-gui</string>
  <key>CFBundleIconFile</key><string>AppIcon</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>LSMinimumSystemVersion</key><string>12.0</string>
  <key>NSHighResolutionCapable</key><true/>
  <key>LSEnvironment</key><dict><key>PATH</key><string>$HOME/.local/bin:/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin</string></dict>
</dict>
</plist>
PLIST
codesign --force --deep --sign - "$APP"
print -- "built: $APP"
