param(
    [string]$InstallDirectory = 'C:\Program Files\PTC\Mathcad Prime 12.0.0.1',
    [string]$OutputDirectory = (Join-Path $PSScriptRoot '../artifacts/rea/mathcad')
)
$ErrorActionPreference = 'Stop'
if (-not (Test-Path -LiteralPath $InstallDirectory -PathType Container)) {
    throw "Mathcad installation directory does not exist: $InstallDirectory"
}
New-Item -ItemType Directory -Force -Path $OutputDirectory | Out-Null
$taskFiles = Get-ChildItem -LiteralPath $InstallDirectory -File
$taskFiles | Select-Object Name, Length | ConvertTo-Json | Set-Content (Join-Path $OutputDirectory 'root-files.json') -Encoding utf8
$taskModules = @(
    'MathcadPrime.exe', 'MathcadPrime.dll', 'Ptc.Core.dll',
    'Ptc.Calculation.Core.dll', 'Ptc.Calculation.dll', 'Ptc.EquationEditor.dll',
    'Ptc.Controls.Worksheet.dll', 'Ptc.Controls.Whiteboard.dll',
    'Ptc.MathcadPrime.Automation.dll', 'MCDXSerializer.dll',
    'mceng.dll', 'mcdepman.dll', 'mccore12.dll', 'mcdtranslator.dll',
    'eficore.dll', 'eficore_.dll'
)
$taskSummary = foreach ($taskModule in $taskModules) {
    $taskPath = Join-Path $InstallDirectory $taskModule
    if (-not (Test-Path -LiteralPath $taskPath -PathType Leaf)) { continue }
    $taskEvidenceText = & rea inspect-managed-artifact $taskPath --json
    if ($LASTEXITCODE -ne 0) { throw "REA failed for $taskModule" }
    $taskEvidenceText | Set-Content (Join-Path $OutputDirectory "$taskModule.artifact.json") -Encoding utf8
    $taskEvidence = $taskEvidenceText | ConvertFrom-Json
    [pscustomobject]@{
        name = $taskModule
        sha256 = $taskEvidence.subject.digest.sha256
        architecture = $taskEvidence.subject.architecture
        classification = $taskEvidence.normalized_result.classification.status
        managed_architecture = $taskEvidence.normalized_result.classification.managed_architecture
        implementation = $taskEvidence.normalized_result.classification.implementation
        runtime_family = $taskEvidence.normalized_result.classification.runtime_family
        assembly = $taskEvidence.normalized_result.assembly
        evidence_id = $taskEvidence.evidence_id
    }
}
$taskSummary | ConvertTo-Json -Depth 8 | Set-Content (Join-Path $OutputDirectory 'module-summary.json') -Encoding utf8
$taskSummary | Select-Object name, architecture, classification
