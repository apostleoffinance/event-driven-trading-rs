[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)][string]$TerminalPath,
    [Parameter(Mandatory = $true)][string]$DaemonArtifactPath,
    [Parameter(Mandatory = $true)][string]$WorkerCertificatePath,
    [Parameter(Mandatory = $true)][string]$PythonPath,
    [Parameter(Mandatory = $true)][string]$WorkerVersion,
    [Parameter(Mandatory = $true)][string]$DaemonVersion,
    [Parameter(Mandatory = $true)][string]$ConfigurationRevision,
    [Parameter(Mandatory = $true)][string]$OrganizationRef,
    [Parameter(Mandatory = $true)][string]$QuantAccountRef,
    [Parameter(Mandatory = $true)][string]$ExternalAccountRef,
    [Parameter(Mandatory = $true)][string]$BindingRef,
    [Parameter(Mandatory = $true)][string]$BindingRevision,
    [Parameter(Mandatory = $true)][string]$BrokerRef,
    [Parameter(Mandatory = $true)][string]$BrokerServerRef,
    [Parameter(Mandatory = $true)][string]$TerminalId,
    [Parameter(Mandatory = $true)][string]$ObservedServer,
    [Parameter(Mandatory = $true)][string]$ObservedLoginHmacSha256,
    [string]$SourceRoot = (Resolve-Path (Join-Path $PSScriptRoot "..\..\..")).Path,
    [string]$OutputPath = ".\mt5-demo-acceptance-evidence.json"
)
$ErrorActionPreference = "Stop"

if ($env:OS -ne "Windows_NT") {
    throw "Manifest capture must run on the approved target Windows MT5 host."
}
foreach ($path in @($TerminalPath, $DaemonArtifactPath, $WorkerCertificatePath, $PythonPath)) {
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
        throw "A required artifact path does not exist."
    }
}
if (-not (Test-Path -LiteralPath $SourceRoot -PathType Container)) {
    throw "SourceRoot does not exist."
}
if ($ObservedLoginHmacSha256 -notmatch '^[0-9a-f]{64}$') {
    throw "ObservedLoginHmacSha256 must be a lowercase HMAC-SHA-256 digest."
}
if ([string]::IsNullOrWhiteSpace($TerminalId) -or [string]::IsNullOrWhiteSpace($ObservedServer)) {
    throw "Terminal and observed broker server references are required."
}

$workerRoot = Join-Path $SourceRoot "adapters\mt5-python\mt5_adapter"
if (-not (Test-Path -LiteralPath $workerRoot -PathType Container)) {
    throw "The MT5 worker package directory was not found."
}
$workerFiles = @(Get-ChildItem -LiteralPath $workerRoot -Filter "*.py" -File -Recurse | Sort-Object FullName)
if ($workerFiles.Count -eq 0) {
    throw "No worker Python source files were found to hash."
}
$manifestBuilder = [System.Text.StringBuilder]::new()
foreach ($file in $workerFiles) {
    $relative = $file.FullName.Substring($workerRoot.Length).TrimStart([char[]]@('\', '/'))
    $fileHash = (Get-FileHash -LiteralPath $file.FullName -Algorithm SHA256).Hash.ToLowerInvariant()
    [void]$manifestBuilder.AppendLine("$relative|$fileHash")
}
$sha = [System.Security.Cryptography.SHA256]::Create()
try {
    $workerBytes = [System.Text.Encoding]::UTF8.GetBytes($manifestBuilder.ToString())
    $workerHash = ([BitConverter]::ToString($sha.ComputeHash($workerBytes))).Replace("-", "").ToLowerInvariant()
} finally {
    $sha.Dispose()
}

$commit = (& git -C $SourceRoot rev-parse HEAD 2>$null)
if ($LASTEXITCODE -ne 0 -or $commit -notmatch '^[0-9a-f]{40}$') {
    throw "Could not resolve the source Git commit."
}
$gitStatus = (& git -C $SourceRoot status --porcelain 2>$null | Out-String)
if ($LASTEXITCODE -ne 0) {
    throw "Could not inspect source working-tree status."
}
$workingTreeClean = [string]::IsNullOrWhiteSpace($gitStatus)

$previousCertPath = $env:MT5_MANIFEST_PUBLIC_CERT
try {
    $env:MT5_MANIFEST_PUBLIC_CERT = (Resolve-Path -LiteralPath $WorkerCertificatePath).Path
    $fingerprintOutput = & $PythonPath -c 'import hashlib, os, ssl; from pathlib import Path; der=ssl.PEM_cert_to_DER_cert(Path(os.environ["MT5_MANIFEST_PUBLIC_CERT"]).read_text(encoding="ascii")); print(hashlib.sha256(der).hexdigest())'
    if ($LASTEXITCODE -ne 0 -or $fingerprintOutput -notmatch '^[0-9a-f]{64}$') {
        throw "Could not calculate the public worker certificate fingerprint."
    }
    $pythonOutput = & $PythonPath -c 'import json, platform, sys, MetaTrader5; print(json.dumps({"python_version":platform.python_version(),"python_architecture":platform.architecture()[0],"metatrader5_package_version":getattr(MetaTrader5,"__version__","unknown")}))'
    if ($LASTEXITCODE -ne 0) {
        throw "Could not inspect the official MetaTrader5 Python package."
    }
    $pythonInfo = $pythonOutput | ConvertFrom-Json
} finally {
    $env:MT5_MANIFEST_PUBLIC_CERT = $previousCertPath
}

$os = Get-CimInstance -ClassName Win32_OperatingSystem
$identity = [System.Security.Principal.WindowsIdentity]::GetCurrent()
$terminalItem = Get-Item -LiteralPath $TerminalPath
$daemonHash = (Get-FileHash -LiteralPath $DaemonArtifactPath -Algorithm SHA256).Hash.ToLowerInvariant()
$terminalHash = (Get-FileHash -LiteralPath $TerminalPath -Algorithm SHA256).Hash.ToLowerInvariant()
$capturedAt = [DateTimeOffset]::UtcNow.ToString("o")

$acceptanceTests = @(
    foreach ($testId in 1..21) {
        [ordered]@{
            test_id = $testId
            result = "NOT_RUN"
            observed_at_utc = $null
            evidence_ref = ""
        }
    }
)

$record = [ordered]@{
    evidence_schema_version = 1
    milestone = "MT5-MVP-01A"
    protocol_schema_version = 2
    broker_environment = "DEMO"
    quant_environment = "PAPER"
    execution_enabled = $false
    source_commit_sha = $commit.Trim()
    captured_at_utc = $capturedAt
    artifacts = [ordered]@{
        worker_sha256 = $workerHash
        daemon_sha256 = $daemonHash
        terminal_sha256 = $terminalHash
        worker_certificate_sha256 = $fingerprintOutput.Trim()
        worker_version = $WorkerVersion
        daemon_version = $DaemonVersion
        terminal_build = [string]$terminalItem.VersionInfo.FileVersion
        python_version = [string]$pythonInfo.python_version
        python_architecture = [string]$pythonInfo.python_architecture
        metatrader5_package_version = [string]$pythonInfo.metatrader5_package_version
        configuration_revision = $ConfigurationRevision
    }
    identity = [ordered]@{
        organization_ref = $OrganizationRef
        quant_account_ref = $QuantAccountRef
        external_account_ref = $ExternalAccountRef
        binding_ref = $BindingRef
        binding_revision = $BindingRevision
        broker_ref = $BrokerRef
        broker_server_ref = $BrokerServerRef
        terminal_id = $TerminalId
        observed_server = $ObservedServer
        observed_login_hmac_sha256 = $ObservedLoginHmacSha256
    }
    host = [ordered]@{
        windows_edition = [string]$os.Caption
        windows_build = [string]$os.BuildNumber
        architecture = [string]$os.OSArchitecture
        service_identity_sid = [string]$identity.User.Value
        working_tree_clean = $workingTreeClean
    }
    acceptance_tests = $acceptanceTests
}

$fullOutput = [System.IO.Path]::GetFullPath($OutputPath)
$parent = Split-Path -Parent $fullOutput
if ($parent -and -not (Test-Path -LiteralPath $parent)) {
    New-Item -ItemType Directory -Path $parent -Force | Out-Null
}
$record | ConvertTo-Json -Depth 12 | Set-Content -LiteralPath $fullOutput -Encoding utf8
Write-Host "Redacted host/artifact manifest written. Acceptance tests remain NOT_RUN until individually evidenced."
Write-Host "Output: $fullOutput"
