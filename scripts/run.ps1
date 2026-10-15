param([string]$Open)
$ErrorActionPreference = 'Stop'
Set-Location (Split-Path $PSScriptRoot -Parent)
$env:PATH = "$env:USERPROFILE\.cargo\bin;$env:PATH"
if ($Open) {
    cargo run -p mathcraft-app --locked -- --open $Open
} else {
    cargo run -p mathcraft-app --locked
}
exit $LASTEXITCODE
