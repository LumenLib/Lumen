# build/windows/export_icons.ps1
$svg = "assets/icon.svg"
$output_dir = "assets/png_exports"

if (!(Test-Path $output_dir)) { New-Item -ItemType Directory -Path $output_dir }

$sizes = @(16, 32, 48, 64, 128, 256, 512, 1024)

foreach ($size in $sizes) {
    $filename = "$output_dir/icon_${size}x${size}.png"
    # 使用 inkscape 命令行导出
    # --export-filename: 输出路径
    # --export-width/height: 指定尺寸
    # --export-background-opacity: 0 保证透明背景
    inkscape --export-filename="$filename" --export-width=$size --export-height=$size --export-background-opacity=0 "$svg"
    Write-Host "已导出: $filename"
}

# 组合成 Windows .ico 文件 (使用 ImageMagick)
magick convert "$output_dir/icon_16x16.png" "$output_dir/icon_32x32.png" "$output_dir/icon_48x48.png" "$output_dir/icon_64x64.png" "$output_dir/icon_128x128.png" "$output_dir/icon_256x256.png" assets/app_icon.ico
Write-Host "已组合生成 assets/app_icon.ico"
