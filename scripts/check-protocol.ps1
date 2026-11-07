# Regenerates the TypeScript protocol types and fails when they differ from
# the committed files (spec section 6.1: the Rust structs are the source of
# truth). Run from the repository root.
$ErrorActionPreference = 'Stop'
cargo test -q -p jojo-proto --test bindings
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
$changes = git status --porcelain -- app/src/lib/protocol
if ($changes) {
    Write-Error "Generated protocol types are out of date. Commit these changes:`n$changes"
    exit 1
}
Write-Output "Protocol types are up to date."
