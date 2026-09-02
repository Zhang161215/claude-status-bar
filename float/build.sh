#!/bin/bash
# 打包成 .app bundle。裸可执行文件在 macOS 上会占 Dock 图标，
# 只有 bundle 里的 Info.plist 写了 LSUIElement 才能变成无 Dock 的后台应用。
set -euo pipefail
cd "$(dirname "$0")"

APP="build/Claude Float.app"
BIN="$APP/Contents/MacOS/csb-float"

# 两个 target 都编再 lipo 合并。只跑 `cargo build --release` 只会产出本机架构，
# 在 Apple Silicon 的 CI runner 上出来的包，Intel Mac 根本跑不起来。
ARM=aarch64-apple-darwin
X86=x86_64-apple-darwin
rustup target add $ARM $X86 >/dev/null 2>&1 || true

rm -rf "$APP"
mkdir -p "$APP/Contents/MacOS"

echo "编译 $ARM …"
cargo build --release -q --target $ARM
if cargo build --release -q --target $X86 2>/dev/null; then
    lipo -create "target/$ARM/release/csb-float" "target/$X86/release/csb-float" -output "$BIN"
    echo "已合并为通用二进制"
else
    echo "⚠️  $X86 编译失败（缺工具链），仅打包 $ARM"
    cp "target/$ARM/release/csb-float" "$BIN"
fi
echo "架构: $(lipo -archs "$BIN")"

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
