#!/bin/bash
set -e

APP_NAME="Lumen"
BINARY_NAME="lumen"
VERSION=$(grep '^version =' Cargo.toml | head -1 | cut -d '"' -f 2)
OUTPUT_DIR="dist"
BUNDLE_DIR="${OUTPUT_DIR}/${APP_NAME}-${VERSION}-Linux-x64"

echo "=== Lumen Linux Build ==="
echo "Version: ${VERSION}"

# Step 1: Build
echo "[1/3] Building release..."
cargo build --release

# Step 2: Portable tarball
echo "[2/3] Creating portable bundle..."
rm -rf "${BUNDLE_DIR}"
mkdir -p "${BUNDLE_DIR}"

cp "target/release/${BINARY_NAME}" "${BUNDLE_DIR}/"
# README.md 不包含在打包文件中
cp -r assets "${BUNDLE_DIR}/" --parents 2>/dev/null || cp -r "assets" "${BUNDLE_DIR}/"

echo "Done"

# Step 3: Optional AppImage (if linuxdeploy is available)
if command -v linuxdeploy &> /dev/null && command -v appimagetool &> /dev/null; then
    echo "[3/3] Building AppImage..."
    APPDIR="${OUTPUT_DIR}/${APP_NAME}.AppDir"
    rm -rf "${APPDIR}"

    linuxdeploy \
        --appdir "${APPDIR}" \
        --executable "target/release/${BINARY_NAME}" \
        --desktop-file "assets/${BINARY_NAME}.desktop" \
        --icon-file "assets/icon.svg" \
        --output appimage \
        2>&1 || echo "AppImage build failed (non-fatal)"

    mv "${APP_NAME}*.AppImage" "${OUTPUT_DIR}/" 2>/dev/null || true
else
    echo "[3/3] Skipping AppImage (linuxdeploy not found)"
    echo "  Install: wget https://github.com/linuxdeploy/linuxdeploy/releases/download/continuous/linuxdeploy-x86_64.AppImage"
fi

# Results
echo "=== Build Complete! ==="
echo "Output: ${OUTPUT_DIR}/"
ls -lh "${OUTPUT_DIR}/"
