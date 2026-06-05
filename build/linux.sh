#!/bin/bash
set -e

APP_NAME="Lumen"
BINARY_NAME="lumen"
VERSION=$(grep '^version =' Cargo.toml | head -1 | cut -d '"' -f 2)
OUTPUT_DIR="dist"

echo "=== Lumen Linux Build ==="
echo "Version: ${VERSION}"

# Step 1: Build
echo "[1/2] Building release..."
cargo build --release

# Step 2: Build .deb
echo "[2/2] Building .deb package..."
DEB_DIR="${OUTPUT_DIR}/lumen_${VERSION}_amd64"
rm -rf "${DEB_DIR}"
mkdir -p "${DEB_DIR}/DEBIAN"
mkdir -p "${DEB_DIR}/usr/bin"
mkdir -p "${DEB_DIR}/usr/lib/lumen"
mkdir -p "${DEB_DIR}/usr/share/applications"
mkdir -p "${DEB_DIR}/usr/share/icons/hicolor/512x512/apps"

cat > "${DEB_DIR}/DEBIAN/control" <<EOF
Package: lumen
Version: ${VERSION}
Section: utils
Priority: optional
Architecture: amd64
Depends: libxcb1, libxkbcommon-x11-0
Maintainer: Lumen Team
Homepage: https://github.com/LumenLib/Lumen
Description: A modern literature manager built with GPUI
 Lumen is a GPUI-based literature reference manager
 with PDF reading, metadata management, and sync capabilities.
EOF

cp "target/release/${BINARY_NAME}" "${DEB_DIR}/usr/bin/"
cp "assets/${BINARY_NAME}.desktop" "${DEB_DIR}/usr/share/applications/"
cp "assets/icon.svg" "${DEB_DIR}/usr/share/icons/hicolor/512x512/apps/lumen.svg"

dpkg-deb --build "${DEB_DIR}"
mv "${OUTPUT_DIR}/lumen_${VERSION}_amd64.deb" "${OUTPUT_DIR}/Lumen-${VERSION}-Linux-amd64.deb"
rm -rf "${DEB_DIR}"

echo "Done"

# Results
echo "=== Build Complete! ==="
echo "Output: ${OUTPUT_DIR}/"
ls -lh "${OUTPUT_DIR}/"
