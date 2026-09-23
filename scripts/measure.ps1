# Misura i budget di performance definiti in ROADMAP.md sulla build release.
# Uso: npm run measure            (build + misura)
#      npm run measure -- -SkipBuild
param([switch]$SkipBuild)

$ErrorActionPreference = "Stop"
$root = Split-Path $PSScriptRoot -Parent
$release = Join-Path $root "src-tauri\target\release"

if (-not $SkipBuild) {
    Push-Location $root
    npm run tauri build
    if ($LASTEXITCODE -ne 0) { throw "Build fallita" }
    Pop-Location
}

$exe = Join-Path $release "steroitor.exe"
$installer = Get-ChildItem (Join-Path $release "bundle\nsis\*.exe") | Sort-Object LastWriteTime | Select-Object -Last 1

function Get-ProcessTree([int]$rootId) {
    $all = Get-CimInstance Win32_Process | Select-Object ProcessId, ParentProcessId
    $ids = @($rootId)
    $frontier = @($rootId)
    while ($frontier.Count -gt 0) {
        $children = @($all | Where-Object { $frontier -contains $_.ParentProcessId } | ForEach-Object { [int]$_.ProcessId })
        $ids += $children
        $frontier = $children
    }
    $ids
}

function Get-PrivateMemoryMB([int[]]$ids) {
    $bytes = (Get-CimInstance Win32_PerfFormattedData_PerfProc_Process |
        Where-Object { $ids -contains [int]$_.IDProcess } |
        Measure-Object WorkingSetPrivate -Sum).Sum
    [math]::Round($bytes / 1MB, 1)
}

$sample = Join-Path $env:TEMP "steroitor-measure.txt"
Set-Content $sample "riga di prova" -Encoding utf8
$marker = Join-Path $env:TEMP "steroitor-ready.txt"
Remove-Item $marker -ErrorAction SilentlyContinue

$env:STEROITOR_MEASURE_FILE = $marker
$process = Start-Process $exe -ArgumentList "`"$sample`"" -PassThru
Remove-Item Env:STEROITOR_MEASURE_FILE

$deadline = (Get-Date).AddSeconds(15)
while (-not (Test-Path $marker) -and (Get-Date) -lt $deadline) { Start-Sleep -Milliseconds 50 }
if (-not (Test-Path $marker)) { throw "L'app non ha segnalato app_ready entro 15 s" }
$startupMs = [int](Get-Content $marker)

Start-Sleep -Seconds 3
$ramMb = Get-PrivateMemoryMB (Get-ProcessTree $process.Id)
Get-ProcessTree $process.Id | ForEach-Object { Stop-Process -Id $_ -Force -ErrorAction SilentlyContinue }

$results = @(
    [pscustomobject]@{ Metrica = "Installer (MB)"; Valore = [math]::Round($installer.Length / 1MB, 1); Target = 10; Limite = 15 }
    [pscustomobject]@{ Metrica = "Eseguibile (MB)"; Valore = [math]::Round((Get-Item $exe).Length / 1MB, 1); Target = $null; Limite = $null }
    [pscustomobject]@{ Metrica = "RAM privata, 1 file (MB)"; Valore = $ramMb; Target = 80; Limite = 120 }
    [pscustomobject]@{ Metrica = "Avvio -> editor pronto (ms)"; Valore = $startupMs; Target = 500; Limite = 1000 }
)

$results | ForEach-Object {
    $_ | Add-Member Esito $(if ($null -eq $_.Limite) { "-" } elseif ($_.Valore -gt $_.Limite) { "FUORI LIMITE" } elseif ($_.Valore -gt $_.Target) { "sopra target" } else { "ok" })
}
$results | Format-Table -AutoSize

if ($results | Where-Object Esito -eq "FUORI LIMITE") { exit 1 }
