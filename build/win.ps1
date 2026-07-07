$ErrorActionPreference = "Stop"
$AppName = "Lumen"
$Version = (Select-String '^version = "(.+)"' Cargo.toml).Matches[0].Groups[1].Value
$OutputDir = "dist"

Write-Host "`n=== Lumen Windows Build Tool ===" -ForegroundColor Cyan
Write-Host "Version: $Version`n" -ForegroundColor Cyan

# Step 1: Clean output directory
Write-Host "[1/3] Cleaning output directory..." -ForegroundColor Yellow
Remove-Item -Recurse -Force "$OutputDir\$AppName-*" -ErrorAction SilentlyContinue
Write-Host "Done`n" -ForegroundColor Green

# Step 2: Build
Write-Host "[2/3] Building release version..." -ForegroundColor Yellow
$buildStart = Get-Date
cargo build --release
if ($LASTEXITCODE -ne 0) { Write-Host "Build failed!" -ForegroundColor Red; exit 1 }
$buildTime = ((Get-Date) - $buildStart).TotalSeconds
Write-Host "Done ($([math]::Round($buildTime, 1))s)`n" -ForegroundColor Green

# Step 3: Package
Write-Host "[3/3] Packaging..." -ForegroundColor Yellow

$portableDir = "$OutputDir\$AppName-$Version-Windows-x64"
$zipPath = "$OutputDir\$AppName-$Version-Windows-x64-Portable.zip"

Remove-Item -Recurse -Force $portableDir -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Path $portableDir -Force | Out-Null
Copy-Item "target\release\lumen.exe" $portableDir
Write-Host "  Creating portable ZIP..." -ForegroundColor Gray

$7z = Get-Command "7z" -ErrorAction SilentlyContinue
if ($7z) {
    & $7z.Path a -tzip -mx5 -bd "$zipPath" "$portableDir\*" 2>&1 | Out-Null
} else {
    Compress-Archive -Path "$portableDir\*" -DestinationPath $zipPath -CompressionLevel Optimal
}
Remove-Item -Recurse -Force $portableDir

if (-not $SkipInstaller) {
    $iscc = Get-Command "iscc" -ErrorAction SilentlyContinue
    if ($iscc) {
        Write-Host "  Creating installer..." -ForegroundColor Gray
        & $iscc.Path "build\windows\installer.iss" "/dMyAppVersion=$Version" 2>&1 | Out-Null
        if ($LASTEXITCODE -ne 0) { Write-Host "  Installer failed" -ForegroundColor Red }
    } else {
        Write-Host "  Skipped installer (iscc not in PATH)" -ForegroundColor Gray
    }
}

Write-Host "Done`n" -ForegroundColor Green

# Results
Write-Host "=== Build Complete! ===" -ForegroundColor Green
Write-Host "`nGenerated files:" -ForegroundColor Yellow
Get-ChildItem $OutputDir | Where-Object { $_.Extension -in @(".exe", ".zip") } | ForEach-Object {
    $size = [math]::Round($_.Length / 1MB, 2)
    Write-Host "  - $($_.Name) ($size MB)" -ForegroundColor White
}
Write-Host "`nOutput directory: $(Resolve-Path $OutputDir)`n" -ForegroundColor Gray
