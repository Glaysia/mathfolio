# Run this script in an elevated PowerShell terminal.
$ErrorActionPreference = 'Stop'
$taskIdentity = [Security.Principal.WindowsIdentity]::GetCurrent()
$taskPrincipal = [Security.Principal.WindowsPrincipal]::new($taskIdentity)
if (-not $taskPrincipal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) {
    throw 'MSVC installation requires an administrator PowerShell terminal.'
}

$taskVswhere = "${env:ProgramFiles(x86)}\Microsoft Visual Studio\Installer\vswhere.exe"
$taskVsPath = $null
if (Test-Path $taskVswhere) {
    $taskVsCandidates = & $taskVswhere -all -products '*' -property installationPath
    $taskVsPath = $taskVsCandidates | Where-Object { Test-Path $_ } | Select-Object -First 1
}
if ($taskVsPath) {
    $taskVsSetup = "${env:ProgramFiles(x86)}\Microsoft Visual Studio\Installer\setup.exe"
    $taskVsArguments = 'modify --installPath "{0}" --channelId VisualStudio.17.Release --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended --quiet --norestart' -f $taskVsPath
    $taskVsProcess = Start-Process -FilePath $taskVsSetup -ArgumentList $taskVsArguments -WorkingDirectory $env:TEMP -WindowStyle Hidden -Wait -PassThru
    $taskVsExit = $taskVsProcess.ExitCode
} else {
    winget install --id Microsoft.VisualStudio.2022.BuildTools --exact --silent --accept-package-agreements --accept-source-agreements --override '--installPath "C:\BuildTools\Mathcraft2022" --quiet --wait --norestart --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended'
    $taskVsExit = $LASTEXITCODE
}
if ($taskVsExit -notin @(0, 3010)) {
    throw "Visual Studio installer failed: $taskVsExit"
}
if ($taskVsExit -eq 3010) {
    Write-Warning 'Windows reports that a restart is required. No restart was performed.'
}

$taskCargoBin = Join-Path $env:USERPROFILE '.cargo\bin'
if (-not (Test-Path (Join-Path $taskCargoBin 'rustup.exe'))) {
    winget install --id Rustlang.Rustup --exact --silent --accept-package-agreements --accept-source-agreements --override '-y --profile default --default-toolchain stable --default-host x86_64-pc-windows-msvc'
    if ($LASTEXITCODE -ne 0) { throw "Rustup installation failed: $LASTEXITCODE" }
}
$env:PATH = "$taskCargoBin;$env:PATH"
rustup toolchain install 1.99.0 --profile default --component rust-analyzer
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
& (Join-Path $PSScriptRoot 'check.ps1')
exit $LASTEXITCODE
