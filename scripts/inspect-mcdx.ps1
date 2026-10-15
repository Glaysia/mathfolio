param([Parameter(Mandatory)][string]$Path)
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.IO.Compression.FileSystem
$taskInput = Get-Item -LiteralPath $Path
if ($taskInput.Length -gt 64MB) { throw 'Input exceeds the 64 MiB inspection budget.' }
$taskZip = [IO.Compression.ZipFile]::OpenRead($taskInput.FullName)
try {
    if ($taskZip.Entries.Count -gt 4096) { throw 'Too many package entries.' }
    $taskTotal = ($taskZip.Entries | Measure-Object Length -Sum).Sum
    if ($taskTotal -gt 32MB) { throw 'Package exceeds the 32 MiB expanded inspection budget.' }
    $taskParts = @{}
    foreach ($taskPart in @('mathcad/worksheet.xml', 'mathcad/result.xml', 'mathcad/settings/calculation.xml', 'docProps/app.xml')) {
        $taskEntry = $taskZip.GetEntry($taskPart)
        if ($null -eq $taskEntry) { continue }
        $taskSettings = [Xml.XmlReaderSettings]::new()
        $taskSettings.DtdProcessing = [Xml.DtdProcessing]::Prohibit
        $taskSettings.XmlResolver = $null
        $taskSettings.MaxCharactersInDocument = 16MB
        $taskStream = $taskEntry.Open()
        $taskReader = [Xml.XmlReader]::Create($taskStream, $taskSettings)
        try {
            $taskXml = [Xml.XmlDocument]::new()
            $taskXml.XmlResolver = $null
            $taskXml.Load($taskReader)
            $taskParts[$taskPart] = $taskXml
        } finally {
            $taskReader.Dispose()
            $taskStream.Dispose()
        }
    }
    $taskWorksheet = $taskParts['mathcad/worksheet.xml']
    if ($null -eq $taskWorksheet) { throw 'No mathcad/worksheet.xml part.' }
    $taskRegions = @($taskWorksheet.SelectNodes('//*[local-name()="region"]'))
    $taskCachedCount = 0
    if ($null -ne $taskParts['mathcad/result.xml']) {
        $taskCachedCount = @($taskParts['mathcad/result.xml'].SelectNodes('//*[local-name()="resultData"]')).Count
    }
    [pscustomobject]@{
        source = $taskInput.FullName
        sha256 = (Get-FileHash -LiteralPath $taskInput.FullName -Algorithm SHA256).Hash.ToLowerInvariant()
        entries = @($taskZip.Entries | Select-Object FullName, Length)
        worksheet_namespace = $taskWorksheet.DocumentElement.NamespaceURI
        region_count = $taskRegions.Count
        math_region_count = @($taskRegions | Where-Object { $_.SelectSingleNode('./*[local-name()="math"]') }).Count
        text_region_count = @($taskRegions | Where-Object { $_.SelectSingleNode('./*[local-name()="text"]') }).Count
        math_nodes = @($taskWorksheet.SelectNodes('//*[namespace-uri()="http://schemas.mathsoft.com/math50"]') | Group-Object LocalName | Sort-Object Count -Descending | Select-Object Name, Count)
        regions = @($taskRegions | ForEach-Object {
            [pscustomobject]@{
                id = $_.GetAttribute('region-id')
                left = $_.GetAttribute('left')
                top = $_.GetAttribute('top')
                children = @($_.ChildNodes | Where-Object NodeType -EQ Element | ForEach-Object LocalName)
            }
        })
        calculation_settings_xml = $taskParts['mathcad/settings/calculation.xml'].OuterXml
        app_properties_xml = $taskParts['docProps/app.xml'].OuterXml
        cached_result_count = $taskCachedCount
        limitations = @('Static XML observations only; cached results were not recalculated.', 'One sample does not establish general format semantics or evaluation order.')
    } | ConvertTo-Json -Depth 8
} finally {
    $taskZip.Dispose()
}
