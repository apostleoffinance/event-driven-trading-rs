[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)]
    [string]$WorkerServiceName,
    [Parameter(Mandatory = $true)]
    [string]$DaemonServiceName,
    [int]$TimeoutSeconds = 240,
    [int]$PollSeconds = 5,
    [string]$EvidencePath = ".\mt5-restart-evidence.json"
)
$ErrorActionPreference = "Stop"

if ($env:OS -ne "Windows_NT") { throw "Restart verification must run on the target Windows MT5 host." }
if ($TimeoutSeconds -lt 30 -or $PollSeconds -lt 1 -or $PollSeconds -gt 30) { throw "TimeoutSeconds must be >=30 and PollSeconds must be 1-30." }
foreach ($name in @("DATABASE_URL", "MT5_ACCOUNT_ID", "MT5_TERMINAL_ID")) {
    $value = [Environment]::GetEnvironmentVariable($name)
    if ([string]::IsNullOrWhiteSpace($value) -or $value.Trim() -ne $value) { throw "Required environment variable is missing or invalid: $name" }
}
$databaseUrl = $env:DATABASE_URL
$accountId = $env:MT5_ACCOUNT_ID
$terminalId = $env:MT5_TERMINAL_ID
$worker = Get-Service -Name $WorkerServiceName
$daemon = Get-Service -Name $DaemonServiceName
if ($worker.Status -ne "Running") { throw "Worker service '$WorkerServiceName' must already be running before the first smoke pass." }
if ($daemon.Status -ne "Running") { throw "Daemon service '$DaemonServiceName' must be running; this script will not start or stop the daemon." }

function Get-LatestSnapshot {
    $query = @"
WITH market AS (
  SELECT sequence, captured_at
  FROM public.mt5_bridge_market_data_snapshots
  WHERE account_id = :'account_id' AND terminal_id = :'terminal_id'
  ORDER BY captured_at DESC LIMIT 1
), reconciliation AS (
  SELECT reconciled_sequence, captured_at
  FROM public.mt5_bridge_reconciliation_snapshots
  WHERE account_id = :'account_id' AND terminal_id = :'terminal_id'
  ORDER BY created_at DESC LIMIT 1
)
SELECT json_build_object(
  'market_sequence', market.sequence,
  'market_captured_at', market.captured_at,
  'reconciliation_sequence', reconciliation.reconciled_sequence,
  'sequence_aligned', (market.sequence = reconciliation.reconciled_sequence),
  'reconciliation_captured_at', reconciliation.captured_at
)::text
FROM market CROSS JOIN reconciliation;
"@
    $previousPgDatabase = $env:PGDATABASE
    $env:PGDATABASE = $databaseUrl
    try {
        $raw = & psql -X -qAt -v ON_ERROR_STOP=1 -v "account_id=$accountId" -v "terminal_id=$terminalId" -c $query 2>$null | Select-Object -Last 1
        if ($LASTEXITCODE -ne 0 -or [string]::IsNullOrWhiteSpace($raw)) { return $null }
    } finally {
        $env:PGDATABASE = $previousPgDatabase
    }
    try { return ($raw | ConvertFrom-Json) } catch { return $null }
}
function Invoke-Smoke([string]$PassName) {
    Write-Host ("[{0}] Starting smoke pass at {1}" -f $PassName, [DateTimeOffset]::UtcNow.ToString("o"))
    & "$PSScriptRoot\mt5-demo-smoke.ps1" -TimeoutSeconds $TimeoutSeconds -PollSeconds $PollSeconds
    if (-not $?) { throw "$PassName smoke pass failed. Do not treat restart conformance as passed." }
}
function Write-Evidence([System.Collections.IDictionary]$Record) {
    $full = [System.IO.Path]::GetFullPath($EvidencePath)
    $parent = Split-Path -Parent $full
    if ($parent -and -not (Test-Path -LiteralPath $parent)) { New-Item -ItemType Directory -Path $parent -Force | Out-Null }
    $Record | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath $full -Encoding utf8
    Write-Host "Evidence record written to: $full"
}

$record = [ordered]@{
    started_at_utc = [DateTimeOffset]::UtcNow.ToString("o")
    worker_service = $WorkerServiceName
    daemon_service = $DaemonServiceName
    first_smoke = "NOT_RUN"
    worker_stopped_at_utc = $null
    worker_restarted_at_utc = $null
    daemon_remained_running = $false
    pre_stop_market_sequence = $null
    post_start_baseline_market_sequence = $null
    post_restart_market_sequence = $null
    post_restart_reconciliation_sequence = $null
    post_restart_sequence_aligned = $false
    post_restart_market_captured_at = $null
    post_restart_reconciliation_captured_at = $null
    sequence_advanced = $false
    post_restart_reconciliation_fresh = $false
    second_smoke = "NOT_RUN"
    worker_restore_attempted = $false
    worker_restored = $false
    worker_restore_error = $null
    result = "BLOCKED"
}

try {
    Invoke-Smoke "FIRST"
    $record.first_smoke = "PASS"
    $before = Get-LatestSnapshot
    if ($null -eq $before) { throw "No persisted market-data/reconciliation pair found before worker restart." }
    if (-not [bool]$before.sequence_aligned) { throw "Baseline market and reconciliation snapshots do not share the same sequence." }
    $record.pre_stop_market_sequence = [long]$before.market_sequence
    Write-Host ("Baseline market sequence: {0}; captured at {1}" -f $before.market_sequence, $before.market_captured_at)

    Stop-Service -Name $WorkerServiceName -ErrorAction Stop
    $worker.WaitForStatus([System.ServiceProcess.ServiceControllerStatus]::Stopped, [TimeSpan]::FromSeconds(30))
    # Record the stop time only after Windows confirms the worker is stopped.
    $record.worker_stopped_at_utc = [DateTimeOffset]::UtcNow.ToString("o")
    $daemon.Refresh()
    if ($daemon.Status -ne "Running") { throw "Daemon did not remain running after worker stop. This script will not restart the daemon." }
    $record.daemon_remained_running = $true
    Write-Host ("Worker stopped; daemon remains Running at {0}" -f [DateTimeOffset]::UtcNow.ToString("o"))

    Start-Service -Name $WorkerServiceName -ErrorAction Stop
    $worker = Get-Service -Name $WorkerServiceName
    $worker.WaitForStatus([System.ServiceProcess.ServiceControllerStatus]::Running, [TimeSpan]::FromSeconds(30))
    # Record restart only after SCM confirms the service is Running.
    $record.worker_restarted_at_utc = [DateTimeOffset]::UtcNow.ToString("o")
    Write-Host ("Worker service started at {0}; waiting for new market data and reconciliation." -f $record.worker_restarted_at_utc)

    $restartTime = [DateTimeOffset]::Parse($record.worker_restarted_at_utc)
    $deadline = [DateTimeOffset]::UtcNow.AddSeconds($TimeoutSeconds)
    $restartBaseline = $null
    do {
        $restartBaseline = Get-LatestSnapshot
        if ($null -ne $restartBaseline -and [bool]$restartBaseline.sequence_aligned) { break }
        Start-Sleep -Seconds $PollSeconds
    } while ([DateTimeOffset]::UtcNow -lt $deadline)
    if ($null -eq $restartBaseline -or -not [bool]$restartBaseline.sequence_aligned) {
        throw "No sequence-aligned snapshot pair was available after SCM confirmed worker restart."
    }
    $record.post_start_baseline_market_sequence = [long]$restartBaseline.market_sequence
    $deadline = [DateTimeOffset]::UtcNow.AddSeconds($TimeoutSeconds)
    $latest = $null
    do {
        $latest = Get-LatestSnapshot
        if ($null -ne $latest) {
            $marketAt = [DateTimeOffset]::Parse([string]$latest.market_captured_at)
            $reconAt = [DateTimeOffset]::Parse([string]$latest.reconciliation_captured_at)
            $advanced = [long]$latest.market_sequence -gt [long]$record.pre_stop_market_sequence
            $advancedAfterStart = [long]$latest.market_sequence -gt [long]$record.post_start_baseline_market_sequence
            $sequenceAligned = [bool]$latest.sequence_aligned
            $freshAfterRestart = $marketAt -ge $restartTime -and $reconAt -ge $restartTime
            if ($advanced -and $advancedAfterStart -and $sequenceAligned -and $freshAfterRestart) { break }
        }
        Start-Sleep -Seconds $PollSeconds
    } while ([DateTimeOffset]::UtcNow -lt $deadline)

    if ($null -eq $latest) { throw "No post-restart persisted snapshot pair appeared before timeout." }
    $marketAt = [DateTimeOffset]::Parse([string]$latest.market_captured_at)
    $reconAt = [DateTimeOffset]::Parse([string]$latest.reconciliation_captured_at)
    $record.post_restart_market_sequence = [long]$latest.market_sequence
    $record.post_restart_reconciliation_sequence = [long]$latest.reconciliation_sequence
    $record.post_restart_sequence_aligned = [bool]$latest.sequence_aligned
    $record.post_restart_market_captured_at = $marketAt.ToString("o")
    $record.post_restart_reconciliation_captured_at = $reconAt.ToString("o")
    $record.sequence_advanced = ([long]$latest.market_sequence -gt [long]$record.pre_stop_market_sequence)
    $record.post_restart_reconciliation_fresh = ($marketAt -ge $restartTime -and $reconAt -ge $restartTime)
    if (-not $record.post_restart_sequence_aligned) { throw "Market-data and reconciliation snapshots do not share the same sequence after worker restart." }
    if (-not $record.sequence_advanced) { throw "Market-data sequence did not advance across worker restart." }
    if ([long]$latest.market_sequence -le [long]$record.post_start_baseline_market_sequence) { throw "No new market/reconciliation pair arrived after SCM confirmed worker restart." }
    if (-not $record.post_restart_reconciliation_fresh) { throw "Fresh reconciliation did not persist after SCM confirmed worker restart." }
    $daemon.Refresh()
    if ($daemon.Status -ne "Running") { throw "Daemon is not running after worker recovery." }

    Write-Host ("PASS: market sequence {0} -> {1}; reconciliation sequence {2}; snapshots captured after worker stop." -f $record.pre_stop_market_sequence, $record.post_restart_market_sequence, $record.post_restart_reconciliation_sequence)
    Invoke-Smoke "SECOND"
    $record.second_smoke = "PASS"
    $record.result = "PASS_RESTART_RECOVERY"
} catch {
    $record.failure = $_.Exception.Message
    Write-Error $_.Exception.Message
    throw
} finally {
    # This script owns the worker stop. Restore the service even if an assertion
    # fails after the stop so a failed test cannot strand the demo connection.
    try {
        $worker = Get-Service -Name $WorkerServiceName -ErrorAction Stop
        if ($worker.Status -ne "Running") {
            $record.worker_restore_attempted = $true
            Start-Service -Name $WorkerServiceName -ErrorAction Stop
            $worker = Get-Service -Name $WorkerServiceName
            $worker.WaitForStatus([System.ServiceProcess.ServiceControllerStatus]::Running, [TimeSpan]::FromSeconds(30))
        }
        $record.worker_restored = ((Get-Service -Name $WorkerServiceName).Status -eq "Running")
    } catch {
        $record.worker_restore_error = $_.Exception.Message
        $record.result = "FAILED_WORKER_RESTORE"
    }
    $record.finished_at_utc = [DateTimeOffset]::UtcNow.ToString("o")
    Write-Evidence $record
}
