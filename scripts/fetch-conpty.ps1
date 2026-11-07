# Downloads the Microsoft.Windows.Console.ConPTY NuGet package, hashes the
# extracted x64 conpty.dll and OpenConsole.exe inside the work directory, and
# only then copies them into resources/conpty/x64 if those hashes match
# resources/conpty/SHA256SUMS: nothing under resources/conpty/x64 changes on
# a mismatch. Run from the repository root. Pass -WriteSums once after
# changing the version to record the extracted files' hashes before copying
# them in.
param([switch]$WriteSums)
$ErrorActionPreference = 'Stop'
$Version = '1.25.260930003'
$url = "https://api.nuget.org/v3-flatcontainer/microsoft.windows.console.conpty/$Version/microsoft.windows.console.conpty.$Version.nupkg"
$work = Join-Path $env:TEMP "conpty-$Version"
New-Item -ItemType Directory -Force $work | Out-Null
$pkg = Join-Path $work 'pkg.zip'
Invoke-WebRequest $url -OutFile $pkg
Expand-Archive $pkg -DestinationPath (Join-Path $work 'x') -Force

$extracted = [ordered]@{}
foreach ($name in 'conpty.dll', 'OpenConsole.exe') {
    $file = Get-ChildItem (Join-Path $work 'x') -Recurse -Filter $name |
        Where-Object { $_.FullName -match '[\\/](x64|win-x64)[\\/]' } |
        Select-Object -First 1
    if (-not $file) { throw "x64 $name not found in package $Version" }
    $extracted[$name] = $file.FullName
}

$actual = $extracted.Keys | Sort-Object | ForEach-Object {
    "{0}  {1}" -f (Get-FileHash $extracted[$_] -Algorithm SHA256).Hash.ToLower(), $_
}

$sums = 'resources/conpty/SHA256SUMS'
$out = 'resources/conpty/x64'
function Copy-Extracted {
    New-Item -ItemType Directory -Force $out | Out-Null
    foreach ($name in $extracted.Keys) {
        Copy-Item $extracted[$name] (Join-Path $out $name) -Force
    }
}

if ($WriteSums) {
    Set-Content $sums ($actual + "# Microsoft.Windows.Console.ConPTY $Version")
    Copy-Extracted
    Write-Output "Recorded hashes for $Version."
} else {
    $expected = Get-Content $sums | Where-Object { $_ -notmatch '^#' }
    if (Compare-Object $expected $actual) { throw "Hash mismatch for ConPTY $Version; see $sums" }
    Copy-Extracted
    Write-Output "ConPTY $Version verified."
}
