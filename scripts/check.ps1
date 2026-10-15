$ErrorActionPreference = 'Stop'
Set-Location (Split-Path $PSScriptRoot -Parent)

if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
    $taskCargoBin = Join-Path $env:USERPROFILE '.cargo\bin'
    $env:PATH = "$taskCargoBin;$env:PATH"
}

cargo fmt --all -- --check
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
cargo clippy --workspace --all-targets --locked -- -D warnings
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
cargo test --workspace --locked
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
cargo build --workspace --locked
exit $LASTEXITCODE
