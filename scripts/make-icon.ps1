# Draws a 1024x1024 app icon (violet-to-cyan disc with a lowercase j) into
# app/app-icon.png. Run from the repository root, then `pnpm --dir app tauri icon app-icon.png`.
Add-Type -AssemblyName System.Drawing
$size = 1024
$bmp = New-Object System.Drawing.Bitmap $size, $size
$g = [System.Drawing.Graphics]::FromImage($bmp)
$g.SmoothingMode = 'AntiAlias'
$g.TextRenderingHint = 'AntiAliasGridFit'
$g.Clear([System.Drawing.Color]::Transparent)
$rect = New-Object System.Drawing.Rectangle 32, 32, ($size - 64), ($size - 64)
$brush = New-Object System.Drawing.Drawing2D.LinearGradientBrush $rect, ([System.Drawing.Color]::FromArgb(255, 155, 140, 255)), ([System.Drawing.Color]::FromArgb(255, 127, 212, 255)), 45
$g.FillEllipse($brush, $rect)
$font = New-Object System.Drawing.Font 'Segoe UI', 560, ([System.Drawing.FontStyle]::Bold), ([System.Drawing.GraphicsUnit]::Pixel)
$format = New-Object System.Drawing.StringFormat
$format.Alignment = 'Center'
$format.LineAlignment = 'Center'
$text = New-Object System.Drawing.SolidBrush ([System.Drawing.Color]::FromArgb(255, 7, 10, 22))
$g.DrawString('j', $font, $text, (New-Object System.Drawing.RectangleF 0, -30, $size, $size), $format)
$bmp.Save((Join-Path (Get-Location) 'app/app-icon.png'), [System.Drawing.Imaging.ImageFormat]::Png)
$g.Dispose(); $bmp.Dispose()
Write-Output 'Wrote app/app-icon.png'
