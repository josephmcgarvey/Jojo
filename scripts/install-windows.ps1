# Installs or upgrades Jojo on Windows from the latest GitHub release.
#
#   irm https://raw.githubusercontent.com/josephmcgarvey/Jojo/master/scripts/install-windows.ps1 | iex
#
# The installer is downloaded with Invoke-WebRequest, which does not add the
# "downloaded from the internet" mark (the Zone.Identifier stream) that
# browsers add, so SmartScreen does not stop the unsigned installer. It runs
# silently (/S) and installs for the current user into
# %LOCALAPPDATA%\Jojo, without an administrator prompt. Set
# $env:JOJO_VERSION (for example "0.2.5") before running to install a
# specific release instead of the latest.
#
# Running sessions are not affected: they belong to the jojod daemon, which
# keeps running while the window (jojo.exe) is closed and replaced.

$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'
# Windows PowerShell 5.1 may default to TLS 1.0, which GitHub refuses.
[Net.ServicePointManager]::SecurityProtocol = [Net.ServicePointManager]::SecurityProtocol -bor [Net.SecurityProtocolType]::Tls12

$repo = 'josephmcgarvey/Jojo'
if ($env:JOJO_VERSION) {
    $api = "https://api.github.com/repos/$repo/releases/tags/v$($env:JOJO_VERSION.TrimStart('v'))"
} else {
    $api = "https://api.github.com/repos/$repo/releases/latest"
}

$release = Invoke-RestMethod -Uri $api -Headers @{ 'User-Agent' = 'jojo-installer' }
$asset = $release.assets | Where-Object { $_.name -like 'Jojo_*_x64-setup.exe' } | Select-Object -First 1
if (-not $asset) { throw "No Jojo_*_x64-setup.exe found in release $($release.tag_name)." }

$installer = Join-Path ([IO.Path]::GetTempPath()) $asset.name
Write-Host "Downloading $($asset.name)"
Invoke-WebRequest -Uri $asset.browser_download_url -OutFile $installer -Headers @{ 'User-Agent' = 'jojo-installer' }

try {
    # Close the window so its files can be replaced; jojod and its sessions
    # keep running.
    $window = Get-Process -Name 'jojo' -ErrorAction SilentlyContinue
    if ($window) {
        Write-Host 'Closing Jojo (sessions keep running)'
        $window | ForEach-Object { $_.CloseMainWindow() | Out-Null }
        $window | Wait-Process -Timeout 5 -ErrorAction SilentlyContinue
        Get-Process -Name 'jojo' -ErrorAction SilentlyContinue | Stop-Process -Force
    }

    Write-Host 'Installing'
    $process = Start-Process -FilePath $installer -ArgumentList '/S' -Wait -PassThru
    if ($process.ExitCode -ne 0) { throw "The installer exited with code $($process.ExitCode)." }
} finally {
    Remove-Item $installer -ErrorAction SilentlyContinue
}

$app = Join-Path $env:LOCALAPPDATA 'Jojo\jojo.exe'
if (-not (Test-Path $app)) { throw "Jojo was not found at $app after installing." }
Write-Host "Installed $($asset.name -replace '_x64-setup\.exe$', ''). Opening Jojo."
Start-Process -FilePath $app
