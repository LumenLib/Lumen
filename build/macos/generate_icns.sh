#!/bin/bash

# Check if input image exists (prefer SVG for clarity)
if [ -f "assets/icon.svg" ]; then
    INPUT_ICON="assets/icon.svg"
    echo "🔍 Using SVG input: $INPUT_ICON"
    
    # Convert SVG to a high-res temporary PNG first to ensure maximum clarity
    # sips will render the vector path at the requested size
    TEMP_PNG="assets/temp_app_icon_1024.png"
    sips -s format png -z 1024 1024 "$INPUT_ICON" --out "$TEMP_PNG" > /dev/null
    PROCESS_ICON="$TEMP_PNG"
    IS_TEMP_PNG=true
elif [ -f "assets/app_icon.png" ]; then
    INPUT_ICON="assets/app_icon.png"
    echo "ℹ️ Using PNG input: $INPUT_ICON"
    PROCESS_ICON="$INPUT_ICON"
    IS_TEMP_PNG=false
else
    echo "❌ Error: Neither assets/app_icon.svg nor assets/app_icon.png found"
    exit 1
fi

# Create a temporary iconset directory
ICONSET_DIR="assets/app_icon.iconset"
mkdir -p "$ICONSET_DIR"

# Generate icons of various sizes
sips -z 16 16     "$PROCESS_ICON" --out "$ICONSET_DIR/icon_16x16.png" > /dev/null
sips -z 32 32     "$PROCESS_ICON" --out "$ICONSET_DIR/icon_16x16@2x.png" > /dev/null
sips -z 32 32     "$PROCESS_ICON" --out "$ICONSET_DIR/icon_32x32.png" > /dev/null
sips -z 64 64     "$PROCESS_ICON" --out "$ICONSET_DIR/icon_32x32@2x.png" > /dev/null
sips -z 128 128   "$PROCESS_ICON" --out "$ICONSET_DIR/icon_128x128.png" > /dev/null
sips -z 256 256   "$PROCESS_ICON" --out "$ICONSET_DIR/icon_128x128@2x.png" > /dev/null
sips -z 256 256   "$PROCESS_ICON" --out "$ICONSET_DIR/icon_256x256.png" > /dev/null
sips -z 512 512   "$PROCESS_ICON" --out "$ICONSET_DIR/icon_256x256@2x.png" > /dev/null
sips -z 512 512   "$PROCESS_ICON" --out "$ICONSET_DIR/icon_512x512.png" > /dev/null
sips -z 1024 1024 "$PROCESS_ICON" --out "$ICONSET_DIR/icon_512x512@2x.png" > /dev/null

# Convert iconset to icns
iconutil -c icns "$ICONSET_DIR" -o assets/AppIcon.icns

# # Clean up
# rm -rf "$ICONSET_DIR"
# if [ "$IS_TEMP_PNG" = true ]; then
#     rm "$PROCESS_ICON"
# fi

echo "✨ Successfully created assets/AppIcon.icns from $(basename $INPUT_ICON)"
