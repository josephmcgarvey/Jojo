# Downloads the Microsoft.Windows.Console.ConPTY NuGet package and extracts
# the x64 conpty.dll and OpenConsole.exe into resources/conpty/x64, then
# verifies them against resources/conpty/SHA256SUMS. Run from the repository
# root. Pass -WriteSums once after changing the version to record new hashes.
param([switch]$WriteSums)
$ErrorActionPreference = 'Stop'
$Version = '1.25.260930003'
$url = "https://api.nuget.org/v3-flatcontainer/microsoft.windows.console.conpty/$Version/microsoft.windows.console.conpty.$Version.nupkg"
$work = Join-Path $env:TEMP "conpty-$Version"
New-Item -ItemType Directory -Force $work | Out-Null
$pkg = Join-Path $work 'pkg.zip'
Invoke-WebRequest $url -OutFile $pkg
Expand-Archive $pkg -DestinationPath (Join-Path $work 'x') -Force
$out = 'resources/conpty/x64'
New-Item -ItemType Directory -Force $out | Out-Null
foreach ($name in 'conpty.dll', 'OpenConsole.exe') {
    $file = Get-ChildItem (Join-Path $work 'x') -Recurse -Filter $name |
        Where-Object { $_.FullName -match '[\\/](x64|win-x64)[\\/]' } |
        Select-Object -First 1
    if (-not $file) { throw "x64 $name not found in package $Version" }
    Copy-Item $file.FullName (Join-Path $out $name) -Force
}
$sums = 'resources/conpty/SHA256SUMS'
$actual = Get-ChildItem $out -File | Sort-Object Name | ForEach-Object {
    "{0}  {1}" -f (Get-FileHash $_.FullName -Algorithm SHA256).Hash.ToLower(), $_.Name
}
if ($WriteSums) {
    Set-Content $sums ($actual + "# Microsoft.Windows.Console.ConPTY $Version")
    Write-Output "Recorded hashes for $Version."
} else {
    $expected = Get-Content $sums | Where-Object { $_ -notmatch '^#' }
    if (Compare-Object $expected $actual) { throw "Hash mismatch for ConPTY $Version; see $sums" }
    Write-Output "ConPTY $Version verified."
}
