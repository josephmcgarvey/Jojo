# Builds jojod and copies it, with the verified ConPTY pair, into
# app/src-tauri/resources/daemon, which Tauri bundles as the `daemon`
# resource folder. Run before `tauri dev` or `tauri build`.
param([ValidateSet('debug', 'release')][string]$Profile = 'debug')
$ErrorActionPreference = 'Stop'
$root = Split-Path $PSScriptRoot -Parent
Push-Location $root
try {
    if ($Profile -eq 'release') { cargo build -p jojod --release } else { cargo build -p jojod }
    if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
    pwsh -NoProfile -File scripts/fetch-conpty.ps1
    if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
    $dest = 'app/src-tauri/resources/daemon'
    New-Item -ItemType Directory -Force $dest | Out-Null
    Copy-Item "target/$Profile/jojod.exe" $dest -Force
    Copy-Item 'resources/conpty/x64/conpty.dll', 'resources/conpty/x64/OpenConsole.exe' $dest -Force
    Get-ChildItem $dest | Format-Table Name, Length
} finally {
    Pop-Location
}
