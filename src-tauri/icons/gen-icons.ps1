# 生成 Herbie 品牌图标:圆角方块 + 主色 #4a9eff(与 src/renderer/src/style.css 的 --accent 一致)+ 白色字母 "H"
# 用法:powershell -ExecutionPolicy Bypass -File src-tauri/icons/gen-icons.ps1
# 先画出 1024x1024 源 PNG,再交给 `tauri icon` 生成 32x32.png / 128x128.png / 128x128@2x.png / icon.ico / icon.icns
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Drawing

$outDir = $PSScriptRoot
$repoRoot = Split-Path $PSScriptRoot -Parent
$source = Join-Path $outDir 'app-icon-source.png'

$size = 1024
$radius = 200
$d = $radius * 2

$bmp = New-Object System.Drawing.Bitmap($size, $size)
$g = [System.Drawing.Graphics]::FromImage($bmp)
$g.SmoothingMode = [System.Drawing.Drawing2D.SmoothingMode]::AntiAlias
$g.TextRenderingHint = [System.Drawing.Text.TextRenderingHint]::AntiAliasGridFit
$g.Clear([System.Drawing.Color]::Transparent)

# 圆角方块路径
$path = New-Object System.Drawing.Drawing2D.GraphicsPath
$path.AddArc(0, 0, $d, $d, 180, 90)
$path.AddArc($size - $d, 0, $d, $d, 270, 90)
$path.AddArc($size - $d, $size - $d, $d, $d, 0, 90)
$path.AddArc(0, $size - $d, $d, $d, 90, 90)
$path.CloseFigure()

# 纯色填充(品牌蓝,与 --accent 一致)
$fill = New-Object System.Drawing.SolidBrush([System.Drawing.Color]::FromArgb(255, 0x4a, 0x9e, 0xff))
$g.FillPath($fill, $path)

# 中央白色字母 "H"
$font = New-Object System.Drawing.Font('Segoe UI', 600, [System.Drawing.FontStyle]::Bold, [System.Drawing.GraphicsUnit]::Pixel)
$sf = New-Object System.Drawing.StringFormat
$sf.Alignment = [System.Drawing.StringAlignment]::Center
$sf.LineAlignment = [System.Drawing.StringAlignment]::Center
$textRect = New-Object System.Drawing.RectangleF(0, 0, $size, $size)
$g.DrawString('H', $font, [System.Drawing.Brushes]::White, $textRect, $sf)

$bmp.Save($source, [System.Drawing.Imaging.ImageFormat]::Png)
$g.Dispose()
$bmp.Dispose()
Write-Host "source icon: $source"

# 交给 tauri icon 生成整套图标(覆盖 32x32.png / 128x128.png / 128x128@2x.png / icon.ico / icon.icns)
Push-Location $repoRoot
try {
    pnpm tauri icon $source
    if ($LASTEXITCODE -ne 0) { throw "tauri icon failed with exit code $LASTEXITCODE" }
} finally {
    Pop-Location
}
Write-Host "done."
