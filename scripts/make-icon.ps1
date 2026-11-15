# Renders assets/jojo-icon.svg (the J of the wordmark on a dark blue disc)
# to a 1024x1024 PNG with a transparent background at app/app-icon.png,
# using Microsoft Edge in headless mode. Run from the repository root, then
# `pnpm --dir app tauri icon app-icon.png` to regenerate every icon size
# under app/src-tauri/icons (including icon.ico and icon.icns).
$ErrorActionPreference = 'Stop'
$root = Split-Path $PSScriptRoot -Parent
$svg = Join-Path $root 'assets/jojo-icon.svg'
$png = Join-Path $root 'app/app-icon.png'
$edge = @(
    "${env:ProgramFiles(x86)}\Microsoft\Edge\Application\msedge.exe",
    "$env:ProgramFiles\Microsoft\Edge\Application\msedge.exe"
) | Where-Object { Test-Path $_ } | Select-Object -First 1
if (-not $edge) { throw 'Microsoft Edge was not found.' }

# A page that shows the SVG at exactly 1024x1024 on a transparent page.
$page = Join-Path ([IO.Path]::GetTempPath()) 'jojo-icon-render.html'
$svgUrl = 'file:///' + ($svg -replace '\\', '/')
Set-Content -Path $page -Encoding utf8 -Value "<html><body style=`"margin:0;background:transparent`"><img src=`"$svgUrl`" width=`"1024`" height=`"1024`" style=`"display:block`"></body></html>"

$arguments = @(
    '--headless=new', '--disable-gpu', '--hide-scrollbars',
    '--default-background-color=00000000', '--window-size=1024,1024',
    "--screenshot=$png", ('file:///' + ($page -replace '\\', '/'))
)
$process = Start-Process -FilePath $edge -ArgumentList $arguments -Wait -PassThru
Remove-Item $page -ErrorAction SilentlyContinue
if ($process.ExitCode -ne 0 -or -not (Test-Path $png)) { throw 'Edge did not write the PNG.' }
Write-Output "Wrote $png"
