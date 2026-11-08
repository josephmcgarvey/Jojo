# Builds jojod and copies it, with the verified ConPTY pair, into
# app/src-tauri/resources/daemon, which Tauri bundles as the `daemon`
# resource folder. Run before `tauri dev` or `tauri build`.
#
# It also builds the Linux daemons the app installs on remote machines
# (spec 8.2 step 4) with cargo-zigbuild, always in release (a debug Linux
# binary carries tens of megabytes of debug information to upload over
# SSH), and stages them as jojod-linux-x86_64 and jojod-linux-aarch64. A
# release staging fails without cargo-zigbuild; a debug staging warns and
# leaves remote machines unavailable.
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
    # The bundle takes every file in $dest (the `resources/daemon/*` glob,
    # which also matches the tracked .gitkeep so unstaged checkouts still
    # build), so remove anything a previous staging run left behind.
    Get-ChildItem $dest -File | Where-Object Name -ne '.gitkeep' | Remove-Item -Force
    Copy-Item "target/$Profile/jojod.exe" $dest -Force
    Copy-Item 'resources/conpty/x64/conpty.dll', 'resources/conpty/x64/OpenConsole.exe' $dest -Force
    $linux = @{ 'x86_64-unknown-linux-musl' = 'jojod-linux-x86_64'; 'aarch64-unknown-linux-musl' = 'jojod-linux-aarch64' }
    if (Get-Command cargo-zigbuild -ErrorAction SilentlyContinue) {
        foreach ($target in $linux.Keys) {
            cargo zigbuild -p jojod --release --target $target
            if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
            Copy-Item "target/$target/release/jojod" (Join-Path $dest $linux[$target]) -Force
        }
    } elseif ($Profile -eq 'release') {
        Write-Error 'cargo-zigbuild is required to build the Linux daemons (cargo install cargo-zigbuild --locked).'
        exit 1
    } else {
        Write-Warning 'cargo-zigbuild not found: the Linux daemons are not staged, so remote machines cannot be added.'
    }
    Get-ChildItem $dest | Format-Table Name, Length
} finally {
    Pop-Location
}
