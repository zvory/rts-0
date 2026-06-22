#!/usr/bin/env bash
set -euo pipefail

if [[ "$(uname -s)" != "Darwin" ]]; then
  echo "maccursor-spike requires macOS." >&2
  exit 1
fi

if ! command -v swiftc >/dev/null 2>&1; then
  echo "maccursor-spike requires swiftc from Xcode command line tools." >&2
  exit 1
fi

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BUILD_DIR="$SCRIPT_DIR/.build"
APP_DIR="$BUILD_DIR/MacCursorSpike.app"
CONTENTS_DIR="$APP_DIR/Contents"
MACOS_DIR="$CONTENTS_DIR/MacOS"
BINARY="$MACOS_DIR/MacCursorSpike"
MODULE_CACHE="$BUILD_DIR/module-cache"
PLIST="$CONTENTS_DIR/Info.plist"
PKGINFO="$CONTENTS_DIR/PkgInfo"

mkdir -p "$MACOS_DIR" "$MODULE_CACHE"
swiftc -module-cache-path "$MODULE_CACHE" "$SCRIPT_DIR/src/MacCursorSpike.swift" -o "$BINARY"

cat > "$PLIST" <<'PLIST'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN"
  "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleExecutable</key>
  <string>MacCursorSpike</string>
  <key>CFBundleIdentifier</key>
  <string>dev.bewegungskrieg.MacCursorSpike</string>
  <key>CFBundleName</key>
  <string>MacCursorSpike</string>
  <key>CFBundlePackageType</key>
  <string>APPL</string>
  <key>CFBundleShortVersionString</key>
  <string>0.1</string>
  <key>CFBundleVersion</key>
  <string>1</string>
  <key>LSMinimumSystemVersion</key>
  <string>13.0</string>
  <key>NSHighResolutionCapable</key>
  <true/>
</dict>
</plist>
PLIST
printf "APPL????" > "$PKGINFO"

codesign --force --sign - "$APP_DIR" >/dev/null 2>&1

if [[ "${1:-}" == "--self-test" ]]; then
  exec "$BINARY" "$@"
fi

exec open -W -n "$APP_DIR" --args "$@"
