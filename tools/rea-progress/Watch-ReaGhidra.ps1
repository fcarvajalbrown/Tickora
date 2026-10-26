[CmdletBinding()]
param(
    [string]$RuntimeRoot,
    [int]$IntervalSeconds = 5,
    [switch]$Once
)

$ErrorActionPreference = 'Stop'

function Find-RuntimeRoot {
    $candidates = Get-ChildItem -Path $env:TEMP -Directory -Filter 'rea-ghidra-*' -ErrorAction SilentlyContinue |
        Where-Object { Test-Path (Join-Path $_.FullName 'ghidra.log') } |
        Sort-Object CreationTime -Descending
    if (-not $candidates) { throw "No rea-ghidra-* folder with a ghidra.log under $env:TEMP" }
    $candidates[0].FullName
}

function Get-Phase([string[]]$Lines, [bool]$JavaAlive) {
    $joined = $Lines -join "`n"
    if (-not $JavaAlive) {
        if ($joined -match 'REPORT: Analysis succeeded') { return 'done' }
        return 'stopped'
    }
    if ($joined -match '\(HeadlessAnalyzer\) SCRIPT:') { return 'REA post-scripts' }
    if ($joined -match 'REPORT: Analysis succeeded') { return 'analysis finished, saving' }
    if ($joined -match 'ANALYZING all memory and code') { return 'auto-analysis' }
    if ($joined -match 'IMPORTING:') { return 'import' }
    'startup'
}

function Get-GhidraJava([datetime]$Since) {
    Get-Process -Name java -ErrorAction SilentlyContinue |
        Where-Object { $_.StartTime -ge $Since.AddSeconds(-30) } |
        Sort-Object StartTime -Descending |
        Select-Object -First 1
}

function Format-Elapsed([timespan]$Span) {
    '{0:00}:{1:00}:{2:00}' -f [int][math]::Floor($Span.TotalHours), $Span.Minutes, $Span.Seconds
}

if (-not $RuntimeRoot) { $RuntimeRoot = Find-RuntimeRoot }
if (-not (Test-Path $RuntimeRoot)) {
    Write-Host "Run folder $RuntimeRoot does not exist: no REA Ghidra run is active there." -ForegroundColor Yellow
    exit 1
}
$logPath = Join-Path $RuntimeRoot 'ghidra.log'
$started = (Get-Item $RuntimeRoot).CreationTime

while ($true) {
    if (-not (Test-Path $logPath)) {
        Write-Host ("Run folder removed by REA after {0}: the run has ended. Check the REA command's exit code and output." -f (Format-Elapsed ((Get-Date) - $started))) -ForegroundColor Yellow
        break
    }
    $lines = @(Get-Content -Path $logPath -ErrorAction SilentlyContinue)
    $java = Get-GhidraJava $started
    $phase = Get-Phase $lines ($null -ne $java)

    $lastAnalyzer = ($lines | Select-String -Pattern '\((\w+Analyzer)\)' -AllMatches |
        Select-Object -Last 1 | ForEach-Object { $_.Matches[-1].Groups[1].Value })
    $errors = @($lines | Select-String -CaseSensitive -Pattern ' ERROR |OutOfMemoryError')
    $oom = @($errors | Where-Object { $_.Line -match 'OutOfMemoryError' }).Count
    $logAge = (Get-Date) - (Get-Item $logPath).LastWriteTime

    if (-not $Once) { Clear-Host }
    Write-Host "REA Ghidra run  $RuntimeRoot"
    Write-Host ("Phase          {0}" -f $phase) -ForegroundColor Cyan
    Write-Host ("Elapsed        {0}" -f (Format-Elapsed ((Get-Date) - $started)))
    Write-Host ("Last analyzer  {0}" -f ($(if ($lastAnalyzer) { $lastAnalyzer } else { '-' })))
    Write-Host ("Log            {0} lines, last write {1:N0}s ago" -f $lines.Count, $logAge.TotalSeconds)
    if ($java) {
        Write-Host ("Java           pid {0}, {1:N2} GB working set, {2:N0}s CPU" -f $java.Id, ($java.WorkingSet64 / 1GB), $java.TotalProcessorTime.TotalSeconds)
    } else {
        Write-Host 'Java           not running'
    }
    if ($errors.Count -gt 0) {
        Write-Host ("Errors         {0} ({1} OutOfMemory)" -f $errors.Count, $oom) -ForegroundColor Red
        Write-Host ("Last error     {0}" -f $errors[-1].Line.Trim()) -ForegroundColor Red
    } else {
        Write-Host 'Errors         0' -ForegroundColor Green
    }

    if ($Once -or $phase -in @('done', 'stopped')) { break }
    Start-Sleep -Seconds $IntervalSeconds
}
