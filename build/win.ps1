# Lumen Windows Build Script
param([switch]$Clean, [switch]$SkipBuild, [switch]$SkipInstaller)
$ErrorActionPreference = "Stop"
$AppName = "Lumen"
$Version = (Select-String '^version = "(.+)"' Cargo.toml).Matches[0].Groups[1].Value
$OutputDir = "dist"

Write-Host "`n=== Lumen Windows Build Tool ===" -ForegroundColor Cyan
Write-Host "Version: $Version`n" -ForegroundColor Cyan

# Step 1: Clean
if ($Clean) {
    Write-Host "[1/4] Cleaning old build files..." -ForegroundColor Yellow
    Remove-Item -Recurse -Force "target\release\build\$AppName-*" -ErrorAction SilentlyContinue
    Remove-Item -Recurse -Force "target\release\deps\lumen*" -ErrorAction SilentlyContinue
    Remove-Item -Recurse -Force "$OutputDir\$AppName-*" -ErrorAction SilentlyContinue
    Write-Host "Done`n" -ForegroundColor Green
}

# Step 2: Build
if (-not $SkipBuild) {
    Write-Host "[2/4] Building release version..." -ForegroundColor Yellow
    $buildStart = Get-Date
    cargo build --release
    if ($LASTEXITCODE -ne 0) { Write-Host "Build failed!" -ForegroundColor Red; exit 1 }
    $buildTime = ((Get-Date) - $buildStart).TotalSeconds
    Write-Host "Done ($([math]::Round($buildTime, 1))s)`n" -ForegroundColor Green
}

# Step 3: Portable ZIP
Write-Host "[3/4] Creating portable version..." -ForegroundColor Yellow
$portableDir = "$OutputDir\$AppName-$Version-Windows-x64"
Remove-Item -Recurse -Force $portableDir -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Path $portableDir -Force | Out-Null
Copy-Item "target\release\lumen.exe" $portableDir
# README.md 不包含在打包文件中
"@echo off`nstart """" ""%~dp0lumen.exe""" | Out-File "$portableDir\Run.bat" -Encoding ASCII
$zipPath = "$OutputDir\$AppName-$Version-Windows-x64-Portable.zip"
Remove-Item $zipPath -ErrorAction SilentlyContinue
Compress-Archive -Path $portableDir -DestinationPath $zipPath -CompressionLevel Optimal
Remove-Item -Recurse -Force $portableDir
Write-Host "Done`n" -ForegroundColor Green

# Step 4: Installer
if (-not $SkipInstaller) {
    Write-Host "[4/4] Creating installer..." -ForegroundColor Yellow
    $iscc = Get-Command "iscc" -ErrorAction SilentlyContinue
    if ($iscc) {
        & $iscc.Path "build\windows\installer.iss" 2>&1 | Out-Null
        if ($LASTEXITCODE -eq 0) { Write-Host "Done`n" -ForegroundColor Green }
        else { Write-Host "Failed`n" -ForegroundColor Red }
    } else {
        Write-Host "Skipped (Inno Setup not found in PATH)`n" -ForegroundColor Gray
    }
}

# Results
Write-Host "=== Build Complete! ===" -ForegroundColor Green
Write-Host "`nGenerated files:" -ForegroundColor Yellow
Get-ChildItem $OutputDir | Where-Object {$_.Extension -in @(".exe",".zip")} | ForEach-Object {
    $size = [math]::Round($_.Length / 1MB, 2)
    Write-Host "  - $($_.Name) ($size MB)" -ForegroundColor White
}
Write-Host "`nOutput directory: $(Resolve-Path $OutputDir)`n" -ForegroundColor Gray
