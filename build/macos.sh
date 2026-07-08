#!/bin/bash
set -e

# 配置信息
APP_NAME="Lumen"
BINARY_NAME="lumen"
BUNDLE_ID="com.haifeng.lumen"
VERSION=$(grep '^version =' Cargo.toml | head -1 | cut -d '"' -f 2)
OUTPUT_DIR="dist"

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
    echo "🎨 正在复制已有 assets/AppIcon.icns 图标..."
    cp "assets/AppIcon.icns" "${RESOURCES_PATH}/icon.icns"
else
    echo "⚠️ 未找到 assets/AppIcon.icns，生成临时图标..."
    if [ -f "build/macos/generate_icns.sh" ]; then
        chmod +x "build/macos/generate_icns.sh"
        ./build/macos/generate_icns.sh
        if [ -f "target/AppIcon.icns" ]; then
            cp "target/AppIcon.icns" "${RESOURCES_PATH}/icon.icns"
        else
            echo "❌ 生成图标失败"
        fi
    else
        echo "❌ 找不到图标文件，打包继续但无图标。"
    fi
fi

# 6. 生成 Info.plist
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

# 7. (可选) 代码签名 - 本地运行一般不需要，但加上占位符
# codesign --force --deep --sign - "${APP_PATH}"

# 8. 准备 DMG 根目录
DMG_ROOT="dist/dmg_root"
rm -rf "${DMG_ROOT}"
mkdir -p "${DMG_ROOT}"

# 将 Lumen.app 移动到 DMG 根目录
mv "${APP_PATH}" "${DMG_ROOT}/"

# 创建指向系统的 /Applications 目录的软链接
ln -s /Applications "${DMG_ROOT}/Applications"

# 9. 创建 .dmg
echo "📦 正在创建 .dmg..."
hdiutil create -volname "${APP_NAME} ${VERSION}" \
    -srcfolder "${DMG_ROOT}" \
    -ov -format UDZO \
    "${OUTPUT_DIR}/${APP_NAME}-${VERSION}-macOS-$(uname -m).dmg"

# 清理临时目录
rm -rf "${DMG_ROOT}"
echo "✅ 打包完成"
echo "🌟 你现在可以双击运行 ${OUTPUT_DIR}/${APP_NAME}-${VERSION}-macOS-arm64.dmg 并将 ${APP_NAME} 拖动到 /Applications 目录中。"
