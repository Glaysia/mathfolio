param([Parameter(Mandatory)][string]$Manifest)
$ErrorActionPreference='Stop'
Add-Type -AssemblyName System.IO.Compression.FileSystem
$taskManifestPath=(Resolve-Path -LiteralPath $Manifest).Path
$taskDirectory=Split-Path $taskManifestPath -Parent
$taskCases=Get-Content -LiteralPath $taskManifestPath -Raw | ConvertFrom-Json
$taskApp=New-Object -ComObject MathcadPrime.Application
try {
    $taskVersion=$taskApp.GetVersion()
    if($taskVersion -ne '12.0.0.1'){throw "Wrong reference version: $taskVersion"}
    $taskRecords=foreach($taskCase in $taskCases){
        $taskSheet=$null
        try {
            if(Test-Path -LiteralPath $taskCase.output){throw "Output already exists: $($taskCase.output)"}
            $taskSheet=$taskApp.Open($taskCase.input)
            $taskSheet.Synchronize()
            # Synchronize dispatches calculation; completion is checked from saved statuses below.
            Start-Sleep -Milliseconds 1000
            $taskSheet.SaveAs($taskCase.output)
            $taskSheet.Close(0)
            [Runtime.InteropServices.Marshal]::ReleaseComObject($taskSheet)|Out-Null
            $taskSheet=$null
            $taskArchive=[IO.Compression.ZipFile]::OpenRead($taskCase.output)
            try {
                $taskReader=[IO.StreamReader]::new($taskArchive.GetEntry('mathcad/result.xml').Open())
                try{[xml]$taskXml=$taskReader.ReadToEnd()}finally{$taskReader.Dispose()}
                $taskResults=@($taskXml.SelectNodes('//*[local-name()="resultData"]') | ForEach-Object {
                    [pscustomobject]@{result_id=$_.GetAttribute('result-id');status=$_.GetAttribute('calculation-status');xml=$_.OuterXml}
                })
            }finally{$taskArchive.Dispose()}
            $taskPending=@($taskResults|Where-Object status -NE 'Synchronized').Count
            [pscustomobject]@{id=$taskCase.id;question=$taskCase.question;version=$taskVersion;input_sha256=(Get-FileHash -LiteralPath $taskCase.input -Algorithm SHA256).Hash;output_sha256=(Get-FileHash -LiteralPath $taskCase.output -Algorithm SHA256).Hash;results=$taskResults;settled=($taskPending -eq 0 -and $taskResults.Count -gt 0);error=$null}
            Write-Host "Observed $($taskCase.id)"
        }catch{
            [pscustomobject]@{id=$taskCase.id;question=$taskCase.question;version=$taskVersion;error=$_.Exception.Message}
            Write-Warning "$($taskCase.id): $($_.Exception.Message)"
        }finally{
            if($null -ne $taskSheet){
                # Only this probe's own document object is closed; never CloseAll/Quit.
                try{$taskSheet.Close(0)}catch{}
                [Runtime.InteropServices.Marshal]::ReleaseComObject($taskSheet)|Out-Null
            }
        }
    }
    $taskRecords|ConvertTo-Json -Depth 8|Set-Content (Join-Path $taskDirectory 'observations.json') -Encoding utf8
    if(@($taskRecords|Where-Object { $_.error -or -not $_.settled }).Count -gt 0){
        throw 'Some probes failed or remain Pending; observations are not a complete reference run.'
    }
}finally{
    [Runtime.InteropServices.Marshal]::ReleaseComObject($taskApp)|Out-Null
}
