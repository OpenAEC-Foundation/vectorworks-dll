# While the oracle server runs a job: presses OK on the message boxes Vectorworks shows (an
# export that has nothing to export, a warning), and logs their text to jobs\dialogs.log, so a
# job never waits for someone at the screen. Ends when the server ends or after -Minutes.
param([string]$Root = "$env:USERPROFILE\vwx-samples", [int]$Minutes = 480)
Add-Type -Path (Join-Path $PSScriptRoot 'Msaa.cs') -ReferencedAssemblies Accessibility
$log = Join-Path $Root 'jobs\dialogs.log'
$alive = Join-Path $Root 'jobs\server.alive'
$until = (Get-Date).AddMinutes($Minutes)
while ((Get-Date) -lt $until -and (Test-Path $alive)) {
    $vw = Get-Process | Where-Object { $_.ProcessName -like 'Vectorworks20*' -and $_.MainWindowHandle -ne 0 } | Select-Object -First 1
    if ($vw) {
        foreach ($w in [Msaa]::Windows($vw.Id)) {
            $p = $w.Split('|')
            if ($p[1] -eq '#32770') {
                $h = [long]$p[0]
                $text = [Msaa]::DialogText($h)
                $sample = (Get-ChildItem (Join-Path $Root 'ladder') -Recurse -Filter *.vwx | Sort-Object LastWriteTime | Select-Object -Last 1).Name
                Add-Content -Path $log -Value ("{0:HH:mm:ss} after {1}: [{2}] {3}" -f (Get-Date), $sample, $p[2], $text) -Encoding utf8
                [void][Msaa]::Press($h, 1)
            }
        }
    }
    Start-Sleep -Milliseconds 700
}
