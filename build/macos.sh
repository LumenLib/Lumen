#!/bin/bash
set -e

# 配置信息
APP_NAME="Lumen"
BINARY_NAME="lumen"
BUNDLE_ID="com.haifeng.lumen"
VERSION=$(grep '^version =' Cargo.toml | head -1 | cut -d '"' -f 2)

echo "📦 开始打包 ${APP_NAME}.app (版本: ${VERSION})..."

# 1. 编译最新的 release binary
echo "🛠️ 正在编译最新代码..."
cargo build --release

# 2. 创建目录结构
APP_PATH="dist/${APP_NAME}.app"
CONTENTS_PATH="${APP_PATH}/Contents"
MACOS_PATH="${CONTENTS_PATH}/MacOS"
RESOURCES_PATH="${CONTENTS_PATH}/Resources"

rm -rf "${APP_PATH}"
mkdir -p "${MACOS_PATH}"
mkdir -p "${RESOURCES_PATH}"

# 3. 复制可执行文件
echo "🚀 正在复制可执行文件..."
cp "target/release/${BINARY_NAME}" "${MACOS_PATH}/${BINARY_NAME}"
chmod +x "${MACOS_PATH}/${BINARY_NAME}"

# 4. 准备图标
if [ -f "assets/AppIcon.icns" ]; then
    echo "🎨 正在复制图标..."
    cp "assets/AppIcon.icns" "${RESOURCES_PATH}/icon.icns"
else
    echo "⚠️ 未找到 assets/AppIcon.icns，尝试运行 generate_icns.sh..."
    if [ -f "build/macos/generate_icns.sh" ]; then
        chmod +x "build/macos/generate_icns.sh"
        ./build/macos/generate_icns.sh
        cp "assets/AppIcon.icns" "${RESOURCES_PATH}/icon.icns"
    else
        echo "❌ 找不到图标文件，打包继续但无图标。"
    fi
fi

# 5. 生成 Info.plist
echo "📝 生成 Info.plist..."
cat > "${CONTENTS_PATH}/Info.plist" <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleExecutable</key>
    <string>${BINARY_NAME}</string>
    <key>CFBundleIdentifier</key>
    <string>${BUNDLE_ID}</string>
    <key>CFBundleName</key>
    <string>${APP_NAME}</string>
    <key>CFBundleDisplayName</key>
    <string>${APP_NAME}</string>
    <key>CFBundleIconFile</key>
    <string>icon.icns</string>
    <key>CFBundleShortVersionString</key>
    <string>${VERSION}</string>
    <key>CFBundleVersion</key>
    <string>${VERSION}</string>
    <key>CFBundlePackageType</key>
    <string>APPL</string>
    <key>CFBundleSignature</key>
    <string>????</string>
    <key>LSMinimumSystemVersion</key>
    <string>10.15</string>
    <key>NSHighResolutionCapable</key>
    <true/>
    <key>NSQuitAlwaysKeepsWindows</key>
    <false/>
</dict>
</plist>
EOF

# 6. (可选) 代码签名 - 本地运行一般不需要，但加上占位符
# codesign --force --deep --sign - "${APP_PATH}"

echo "✅ 打包完成: ${APP_PATH}"
echo "🌟 你现在可以双击运行 ${APP_PATH} 或将其移动到 /Applications 目录。"
