# Regenerates every ts-rs output folder and fails when the result differs
# from the committed files (spec section 6.1). Generated folders are emptied
# first, so a type that was removed in Rust shows up as a deleted file.
# Run from the repository root with cargo on PATH.
$ErrorActionPreference = 'Stop'
$folders = 'app/src/lib/protocol', 'app/src/lib/backend'
foreach ($folder in $folders) {
    Get-ChildItem $folder -Filter *.ts -ErrorAction SilentlyContinue | Remove-Item
}
cargo test -q -p jojo-proto --test bindings
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
cargo test -q -p jojo-app --test bindings
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
$changes = git status --porcelain -- $folders
if ($changes) {
    Write-Error "Generated types are out of date. Commit these changes:`n$changes"
    exit 1
}
Write-Output "Generated types are up to date."
