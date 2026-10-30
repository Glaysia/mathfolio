param([string]$ToolsDirectory = (Join-Path $env:USERPROFILE '.local/share/mathfolio-tools'))
$ErrorActionPreference = 'Stop'
New-Item -ItemType Directory -Force -Path $ToolsDirectory | Out-Null
$taskPackages = @(
    @{
        file = 'ghidra_12.1.4_PUBLIC_20260921.zip'
        url = 'https://github.com/NationalSecurityAgency/ghidra/releases/download/Ghidra_12.1.4_build/ghidra_12.1.4_PUBLIC_20260921.zip'
        hash = 'ddac49f903da9d5bac833e5cc79395098b9c33cfd3279be5f31bd00387d2d4db'
        directory = 'ghidra_12.1.4_PUBLIC'
    },
    @{
        file = 'OpenJDK21U-jdk_x64_windows_hotspot_21.0.12.1_1.zip'
        url = 'https://github.com/adoptium/temurin21-binaries/releases/download/jdk-21.0.12.1%2B1/OpenJDK21U-jdk_x64_windows_hotspot_21.0.12.1_1.zip'
        hash = 'f9d6e191ab098c0d416e7d588a24420a8621cd2f4720dab2459b8b7b2d2d8b4e'
        directory = 'jdk-21.0.12.1+1'
    }
)
foreach ($taskPackage in $taskPackages) {
    $taskArchive = Join-Path $ToolsDirectory $taskPackage.file
    if (-not (Test-Path -LiteralPath $taskArchive)) {
        & curl.exe --fail --location --silent --show-error --output $taskArchive $taskPackage.url
        if ($LASTEXITCODE -ne 0) { throw "Download failed: $($taskPackage.file)" }
    }
    if ((Get-FileHash -LiteralPath $taskArchive -Algorithm SHA256).Hash.ToLowerInvariant() -ne $taskPackage.hash) {
        throw "SHA256 mismatch: $taskArchive"
    }
    $taskDirectory = Join-Path $ToolsDirectory $taskPackage.directory
    if (-not (Test-Path -LiteralPath $taskDirectory)) {
        Expand-Archive -LiteralPath $taskArchive -DestinationPath $ToolsDirectory
    }
    Write-Output "Verified: $($taskPackage.directory)"
}
$env:GHIDRA_INSTALL_DIR = Join-Path $ToolsDirectory 'ghidra_12.1.4_PUBLIC'
$env:JAVA_HOME = Join-Path $ToolsDirectory 'jdk-21.0.12.1+1'
& rea doctor --provider ghidra --json
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
