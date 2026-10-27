[CmdletBinding()]
param(
    [int]$TimeoutSeconds = 180,
    [int]$PollSeconds = 10
)

$ErrorActionPreference = "Stop"

function Require-Env([string]$Name) {
    $value = [Environment]::GetEnvironmentVariable($Name)
    if ([string]::IsNullOrWhiteSpace($value) -or $value.Trim() -ne $value) {
        throw "Required environment variable is missing or has surrounding whitespace: $Name"
    }
    return $value
}

function Require-File([string]$Name) {
    $path = Require-Env $Name
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
        throw "File configured by $Name does not exist: $path"
    }
    return (Resolve-Path -LiteralPath $path).Path
}

if (-not $IsWindows -and $env:OS -ne "Windows_NT") {
    throw "This smoke test must run on the target Windows MT5 host."
}
if ($TimeoutSeconds -lt 30 -or $PollSeconds -lt 1 -or $PollSeconds -gt 30) {
    throw "TimeoutSeconds must be >= 30 and PollSeconds must be between 1 and 30."
}

$terminalPath = Require-File "MT5_TERMINAL_PATH"
$serverCert = Require-File "MT5_BRIDGE_SERVER_CERT"
$serverKey = Require-File "MT5_BRIDGE_SERVER_KEY"
$clientCa = Require-File "MT5_BRIDGE_CLIENT_CA"
$clientCert = Require-File "MT5_BRIDGE_CLIENT_CERT"
$clientKey = Require-File "MT5_BRIDGE_CLIENT_KEY"
$sequenceFile = Require-Env "MT5_SEQUENCE_FILE"
$databaseUrl = Require-Env "DATABASE_URL"
$terminalId = Require-Env "MT5_TERMINAL_ID"
$server = Require-Env "MT5_SERVER"
$accountId = Require-Env "MT5_ACCOUNT_ID"
$login = Require-Env "MT5_LOGIN"
$password = Require-Env "MT5_PASSWORD"
$symbols = Require-Env "MT5_SYMBOLS"
$expectedCurrency = Require-Env "MT5_EXPECTED_ACCOUNT_CURRENCY"
$fingerprint = (Require-Env "MT5_BRIDGE_PRINCIPAL_SHA256").ToLowerInvariant()
$bridgeHost = if ([string]::IsNullOrWhiteSpace($env:MT5_BRIDGE_HOST)) { "127.0.0.1" } else { $env:MT5_BRIDGE_HOST }
$bridgePort = if ([string]::IsNullOrWhiteSpace($env:MT5_BRIDGE_PORT)) { 9443 } else { [int]$env:MT5_BRIDGE_PORT }

if ($fingerprint -notmatch '^[0-9a-f]{64}$') {
    throw "MT5_BRIDGE_PRINCIPAL_SHA256 must be 64 hexadecimal characters."
}
if ($login -notmatch '^[0-9]+$') {
    throw "MT5_LOGIN must be the numeric demo login."
}
$symbolList = @($symbols.Split(","))
if ($symbolList.Count -lt 1 -or $symbolList.Count -gt 64 -or ($symbolList | Select-Object -Unique).Count -ne $symbolList.Count -or ($symbolList | Where-Object { [string]::IsNullOrWhiteSpace($_) -or $_ -ne $_.Trim() }).Count -gt 0) {
    throw "MT5_SYMBOLS must contain 1-64 unique, exact broker symbol names."
}

Write-Host "[1/5] Checking read-only MT5 account, configured symbols, and worker certificate..."
$env:SMOKE_TERMINAL_PATH = $terminalPath
$env:SMOKE_LOGIN = $login
$env:SMOKE_PASSWORD = $password
$env:SMOKE_SERVER = $server
$env:SMOKE_EXPECTED_CURRENCY = $expectedCurrency
$env:SMOKE_SYMBOLS = ($symbolList -join ",")
$env:SMOKE_CLIENT_CERT = $clientCert
$env:SMOKE_CLIENT_KEY = $clientKey
$env:SMOKE_SERVER_CERT = $serverCert
$env:SMOKE_SERVER_KEY = $serverKey
$env:SMOKE_EXPECTED_FINGERPRINT = $fingerprint

$pythonProbe = @'
import hashlib, os, ssl, sys
from datetime import datetime, timezone
from pathlib import Path

def fail(message):
    print("FAIL: " + message, file=sys.stderr)
    raise SystemExit(1)

try:
    cert_text = Path(os.environ["SMOKE_CLIENT_CERT"]).read_text(encoding="ascii")
    der = ssl.PEM_cert_to_DER_cert(cert_text)
    actual = hashlib.sha256(der).hexdigest()
    expected = os.environ["SMOKE_EXPECTED_FINGERPRINT"].lower()
    if actual != expected:
        fail("client certificate DER fingerprint does not match MT5_BRIDGE_PRINCIPAL_SHA256")
    for cert_var, key_var in (("SMOKE_CLIENT_CERT", "SMOKE_CLIENT_KEY"), ("SMOKE_SERVER_CERT", "SMOKE_SERVER_KEY")):
        context = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER if cert_var == "SMOKE_SERVER_CERT" else ssl.PROTOCOL_TLS_CLIENT)
        context.load_cert_chain(os.environ[cert_var], os.environ[key_var])
    import MetaTrader5 as mt5
except Exception as exc:
    fail("certificate or MetaTrader5 preflight failed: " + type(exc).__name__)

initialized = False
try:
    initialized = bool(mt5.initialize(
        path=os.environ["SMOKE_TERMINAL_PATH"],
        login=int(os.environ["SMOKE_LOGIN"]),
        password=os.environ["SMOKE_PASSWORD"],
        server=os.environ["SMOKE_SERVER"],
    ))
    if not initialized:
        fail("MetaTrader5.initialize failed (terminal/broker diagnostics intentionally redacted)")
    account = mt5.account_info()
    if account is None:
        fail("MT5 account_info unavailable")
    if int(account.login) != int(os.environ["SMOKE_LOGIN"]):
        fail("authenticated MT5 login does not match configured MT5_LOGIN")
    if str(account.server) != os.environ["SMOKE_SERVER"]:
        fail("authenticated MT5 server does not match MT5_SERVER")
    if str(account.currency) != os.environ["SMOKE_EXPECTED_CURRENCY"]:
        fail("authenticated MT5 account currency does not match MT5_EXPECTED_ACCOUNT_CURRENCY")
    now = datetime.now(timezone.utc).timestamp()
    symbols = os.environ["SMOKE_SYMBOLS"].split(",")
    for symbol in symbols:
        if not mt5.symbol_select(symbol, True):
            fail("configured broker symbol could not be selected: " + symbol)
        info = mt5.symbol_info(symbol)
        tick = mt5.symbol_info_tick(symbol)
        if info is None or tick is None:
            fail("symbol metadata or tick missing for: " + symbol)
        if not (float(tick.bid) > 0 and float(tick.ask) >= float(tick.bid)):
            fail("invalid bid/ask for configured symbol: " + symbol)
        age = now - int(tick.time)
        if age < -5 or age > 60:
            fail("quote timestamp outside the permitted freshness window for: " + symbol)
        if not (float(info.trade_tick_size) > 0 and float(info.trade_tick_value) > 0 and
                float(info.trade_contract_size) > 0 and float(info.volume_min) > 0 and
                float(info.volume_step) > 0 and float(info.volume_max) >= float(info.volume_min)):
            fail("broker sizing metadata missing or inconsistent for: " + symbol)
    print("PASS: demo account identity, account currency, client certificate fingerprint, and fresh broker metadata/ticks validated.")
finally:
    if initialized:
        mt5.shutdown()
'@

$pythonOutput = & python -c $pythonProbe 2>&1
if ($LASTEXITCODE -ne 0) {
    $pythonOutput | ForEach-Object { Write-Host $_ }
    throw "MT5 preflight failed. No order was submitted."
}
$pythonOutput | ForEach-Object { Write-Host $_ }

Write-Host "[2/5] Checking PostgreSQL CLI and required migrated tables..."
if (-not (Get-Command psql -ErrorAction SilentlyContinue)) {
    throw "psql was not found. Install PostgreSQL client tools on this host."
}
$previousPgDatabase = $env:PGDATABASE
$env:PGDATABASE = $databaseUrl
try {
    $tableCheck = @"
SELECT CASE WHEN
    to_regclass('public.mt5_bridge_market_data_snapshots') IS NOT NULL
    AND to_regclass('public.mt5_bridge_reconciliation_snapshots') IS NOT NULL
THEN 'ok' ELSE 'missing' END;
"@
    $tableState = (& psql -X -qAt -v ON_ERROR_STOP=1 -c $tableCheck 2>$null | Select-Object -Last 1)
    if ($LASTEXITCODE -ne 0 -or $tableState.Trim() -ne "ok") {
        throw "PostgreSQL connection or required MT5 snapshot migrations failed. Confirm DATABASE_URL and apply migrations."
    }
} finally {
    $env:PGDATABASE = $previousPgDatabase
}

Write-Host "[3/5] Checking the Rust daemon listener..."
$tcp = Test-NetConnection -ComputerName $bridgeHost -Port $bridgePort -WarningAction SilentlyContinue
if (-not $tcp.TcpTestSucceeded) {
    throw "No TCP listener found at $($bridgeHost):$bridgePort. Start mt5-bridge-daemon before this smoke test."
}

Write-Host "[4/5] Waiting for fresh, repeated market-data and reconciliation snapshots..."
$accountSql = $accountId.Replace("'", "''")
$terminalSql = $terminalId.Replace("'", "''")
$query = @"
SELECT json_build_object(
  'market_age_seconds', EXTRACT(EPOCH FROM (NOW() - m.captured_at)),
  'market_sequence', m.sequence,
  'market_server', m.server,
  'account_currency', m.account_currency,
  'symbols', (SELECT COALESCE(json_agg(value->>'symbol' ORDER BY value->>'symbol'), '[]'::json)
              FROM jsonb_array_elements(m.snapshot->'instruments') AS instrument(value)),
  'quotes_fresh', (SELECT COALESCE(bool_and(
      EXTRACT(EPOCH FROM (NOW() - (value->>'tick_time')::timestamptz)) BETWEEN -5 AND 60
      AND (value->>'bid')::numeric > 0
      AND (value->>'ask')::numeric >= (value->>'bid')::numeric
    ), false) FROM jsonb_array_elements(m.snapshot->'quotes') AS quote(value))
  ,
  'reconciliation_age_seconds', EXTRACT(EPOCH FROM (NOW() - r.captured_at)),
  'reconciliation_sequence', r.reconciled_sequence
)::text
FROM (
  SELECT * FROM mt5_bridge_market_data_snapshots
  WHERE account_id = '$accountSql' AND terminal_id = '$terminalSql'
) m
CROSS JOIN LATERAL (
  SELECT * FROM mt5_bridge_reconciliation_snapshots
  WHERE account_id = '$accountSql' AND terminal_id = '$terminalSql'
  ORDER BY created_at DESC LIMIT 1
) r;
"@

$deadline = (Get-Date).AddSeconds($TimeoutSeconds)
$observedSequences = [System.Collections.Generic.HashSet[long]]::new()
$lastStatus = $null
do {
    $previousPgDatabase = $env:PGDATABASE
    $env:PGDATABASE = $databaseUrl
    try {
        $raw = & psql -X -qAt -v ON_ERROR_STOP=1 -c $query 2>$null | Select-Object -Last 1
        $psqlExit = $LASTEXITCODE
    } finally {
        $env:PGDATABASE = $previousPgDatabase
    }
    if ($psqlExit -eq 0 -and -not [string]::IsNullOrWhiteSpace($raw)) {
        try {
            $status = $raw | ConvertFrom-Json
            $lastStatus = $status
            $marketAge = [double]$status.market_age_seconds
            $reconAge = [double]$status.reconciliation_age_seconds
            $actualSymbols = @($status.symbols | ForEach-Object { [string]$_ } | Sort-Object)
            $expectedSymbols = @($symbolList | Sort-Object)
            $symbolsMatch = (($actualSymbols -join [Environment]::NewLine) -ceq ($expectedSymbols -join [Environment]::NewLine))
            $identityMatches = ([string]$status.market_server -ceq $server -and [string]$status.account_currency -ceq $expectedCurrency)
            $fresh = ($marketAge -ge 0 -and $marketAge -le 60 -and $reconAge -ge 0 -and $reconAge -le 60)
            if ($fresh -and $identityMatches -and $symbolsMatch -and [bool]$status.quotes_fresh) {
                [void]$observedSequences.Add([long]$status.market_sequence)
                Write-Host ("  market sequence={0}; market age={1:N1}s; reconciliation age={2:N1}s; symbols match={3}" -f $status.market_sequence, $marketAge, $reconAge, $symbolsMatch)
                if ($observedSequences.Count -ge 2) { break }
            } else {
                Write-Host "  Snapshots are present but freshness, identity, symbols, or quote validation has not passed yet."
            }
        } catch {
            Write-Host "  Waiting for valid database snapshot rows..."
        }
    }
    if ((Get-Date) -lt $deadline) { Start-Sleep -Seconds $PollSeconds }
} while ((Get-Date) -lt $deadline)

if ($observedSequences.Count -lt 2) {
    throw "Smoke test timed out without two distinct fresh market-data sequences and fresh reconciliation evidence. Last observed row: $($lastStatus | ConvertTo-Json -Compress -Depth 6). No order was submitted."
}

Write-Host "[5/5] Read-only MT5 demo smoke test passed."
Write-Host "Evidence: terminal/account identity, certificate fingerprint, fresh exact-symbol metadata/ticks, PostgreSQL persistence, and advancing market-data sequence with fresh reconciliation."
Write-Host "Manual release gates still required: restart the worker and repeat this test to verify reconnect/reconciliation; review private-key ACLs and certificate rotation/revocation; retain redacted logs and the timestamped output."
Write-Host "SAFETY: This test performs read-only account/market-data queries. It does not submit, modify, or cancel orders."
