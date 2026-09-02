#!/bin/bash
# 打包成 .app bundle。裸可执行文件在 macOS 上会占 Dock 图标，
# 只有 bundle 里的 Info.plist 写了 LSUIElement 才能变成无 Dock 的后台应用。
set -euo pipefail
cd "$(dirname "$0")"

APP="build/Claude Float.app"
BIN="$APP/Contents/MacOS/csb-float"

echo "编译 release…"
cargo build --release -q

rm -rf "$APP"
mkdir -p "$APP/Contents/MacOS"
cp target/release/csb-float "$BIN"

cat > "$APP/Contents/Info.plist" <<'PLIST'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleName</key><string>Claude Float</string>
  <key>CFBundleDisplayName</key><string>Claude 状态浮窗</string>
  <key>CFBundleIdentifier</key><string>com.local.claudefloat</string>
  <key>CFBundleExecutable</key><string>csb-float</string>
  <key>CFBundleVersion</key><string>0.1.0</string>
  <key>CFBundleShortVersionString</key><string>0.1.0</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>LSMinimumSystemVersion</key><string>11.0</string>
  <!-- 关键：无 Dock 图标、不进 App 切换器，纯后台附件应用 -->
  <key>LSUIElement</key><true/>
  <key>NSHighResolutionCapable</key><true/>
</dict>
</plist>
PLIST

codesign --force --deep -s - "$APP" 2>/dev/null || true
echo "已生成 $APP"
