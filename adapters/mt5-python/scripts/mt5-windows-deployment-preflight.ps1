[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)][string]$WorkerServiceName,
    [Parameter(Mandatory = $true)][string]$DaemonServiceName,
    [Parameter(Mandatory = $true)][string]$ExpectedWorkerIdentity,
    [Parameter(Mandatory = $true)][string]$ExpectedDaemonIdentity,
    [Parameter(Mandatory = $true)][string]$ExpectedTerminalIdentity,
    [Parameter(Mandatory = $true)][string]$TerminalPath,
    [Parameter(Mandatory = $true)][string]$DaemonArtifactPath,
    [Parameter(Mandatory = $true)][string]$PythonPath,
    [Parameter(Mandatory = $true)][string]$ExpectedPythonVersion,
    [Parameter(Mandatory = $true)][string]$ExpectedMetaTrader5Version,
    [Parameter(Mandatory = $true)][string]$ExpectedTerminalFileVersion,
    [Parameter(Mandatory = $true)][string]$ExpectedWindowsBuild,
    [switch]$RequireRunningServices,
    [string]$EvidencePath = ".\mt5-windows-preflight.json"
)
$ErrorActionPreference = "Stop"

# Read-only preflight: never starts/stops services, opens MT5, connects to PostgreSQL,
# reads broker credentials, or prints environment values.
if ($env:OS -ne "Windows_NT") { throw "This preflight must run on the target Windows host." }

$checks = [System.Collections.Generic.List[object]]::new()
$blockers = [System.Collections.Generic.List[string]]::new()
function Add-Check([string]$Name, [bool]$Passed, [string]$Detail) {
    $checks.Add([ordered]@{ name = $Name; result = $(if ($Passed) { "PASS" } else { "BLOCKED" }); detail = $Detail })
    if (-not $Passed) { $blockers.Add($Name) }
}
function Require-File([string]$Label, [string]$Path) {
    if (-not (Test-Path -LiteralPath $Path -PathType Leaf)) {
        Add-Check $Label $false "Required file is absent."
        return $null
    }
    return (Get-Item -LiteralPath $Path)
}
function Get-ServiceIdentity([string]$Name) {
    $escaped = $Name.Replace("'", "''")
    $service = Get-CimInstance -ClassName Win32_Service -Filter "Name='$escaped'" -ErrorAction SilentlyContinue
    if ($null -eq $service) {
        Add-Check "service:$Name" $false "Windows service is not installed."
        return $null
    }
    return $service
}

$os = Get-CimInstance -ClassName Win32_OperatingSystem
Add-Check "windows_build" ([string]$os.BuildNumber -eq $ExpectedWindowsBuild) ("Observed build: " + [string]$os.BuildNumber)
Add-Check "windows_64_bit" ([Environment]::Is64BitOperatingSystem) ("Architecture: " + [string]$os.OSArchitecture)

$terminal = Require-File "terminal_executable" $TerminalPath
$daemon = Require-File "daemon_artifact" $DaemonArtifactPath
$python = Require-File "python_executable" $PythonPath
if ($null -ne $terminal) {
    $terminalVersion = [string]$terminal.VersionInfo.FileVersion
    Add-Check "terminal_file_version" ($terminalVersion -eq $ExpectedTerminalFileVersion) ("Observed file version: " + $terminalVersion)
    $terminalHash = (Get-FileHash -LiteralPath $terminal.FullName -Algorithm SHA256).Hash.ToLowerInvariant()
} else { $terminalHash = $null }
if ($null -ne $daemon) {
    $daemonHash = (Get-FileHash -LiteralPath $daemon.FullName -Algorithm SHA256).Hash.ToLowerInvariant()
} else { $daemonHash = $null }

if ($null -ne $python) {
    $probe = 'import importlib.metadata, json, platform; print(json.dumps({"python_version": platform.python_version(), "architecture": platform.architecture()[0], "metatrader5_version": importlib.metadata.version("MetaTrader5")}))'
    $probeOutput = & $python.FullName -c $probe 2>$null
    $probeExit = $LASTEXITCODE
    if ($probeExit -eq 0) {
        try {
            $pythonInfo = $probeOutput | ConvertFrom-Json
            Add-Check "python_version" ([string]$pythonInfo.python_version -eq $ExpectedPythonVersion) ("Observed version: " + [string]$pythonInfo.python_version)
            Add-Check "python_64_bit" ([string]$pythonInfo.architecture -eq "64bit") ("Observed architecture: " + [string]$pythonInfo.architecture)
            Add-Check "metatrader5_package" ([string]$pythonInfo.metatrader5_version -eq $ExpectedMetaTrader5Version) ("Observed package version: " + [string]$pythonInfo.metatrader5_version)
        } catch {
            Add-Check "python_provider_probe" $false "Python provider metadata was not valid JSON."
        }
    } else {
        Add-Check "python_provider_probe" $false "Python could not import the official MetaTrader5 package."
    }
}

$worker = Get-ServiceIdentity $WorkerServiceName
$daemonService = Get-ServiceIdentity $DaemonServiceName
if ($null -ne $worker) {
    Add-Check "worker_service_identity" ($worker.StartName -eq $ExpectedWorkerIdentity) ("Configured identity: " + [string]$worker.StartName)
    if ($RequireRunningServices) { Add-Check "worker_service_running" ($worker.State -eq "Running") ("Observed state: " + [string]$worker.State) }
}
if ($null -ne $daemonService) {
    Add-Check "daemon_service_identity" ($daemonService.StartName -eq $ExpectedDaemonIdentity) ("Configured identity: " + [string]$daemonService.StartName)
    if ($RequireRunningServices) { Add-Check "daemon_service_running" ($daemonService.State -eq "Running") ("Observed state: " + [string]$daemonService.State) }
}
if ($RequireRunningServices -and $null -ne $terminal) {
    $expectedPath = [IO.Path]::GetFullPath($terminal.FullName)
    $instances = @(Get-CimInstance Win32_Process -Filter "Name='terminal64.exe'" | Where-Object {
        $_.ExecutablePath -and ([IO.Path]::GetFullPath([string]$_.ExecutablePath) -ieq $expectedPath)
    })
    Add-Check "single_terminal_instance" ($instances.Count -eq 1) ("Matching terminal process count: " + $instances.Count)
    if ($instances.Count -eq 1) {
        $owner = Invoke-CimMethod -InputObject $instances[0] -MethodName GetOwner
        $terminalOwner = if ($owner.ReturnValue -eq 0) { [string]$owner.Domain + "\" + [string]$owner.User } else { "" }
        Add-Check "terminal_process_identity" ($terminalOwner -eq $ExpectedTerminalIdentity) "Terminal process owner is compared with the approved identity; no credentials are displayed."
    }
}

$requiredEnvironment = @(
    "DATABASE_URL", "MT5_TERMINAL_PATH", "MT5_LOGIN", "MT5_SERVER", "MT5_SYMBOLS",
    "MT5_TERMINAL_ID", "MT5_ACCOUNT_ID", "MT5_EXPECTED_ACCOUNT_CURRENCY", "MT5_BRIDGE_HOST",
    "MT5_BRIDGE_CA_FILE", "MT5_BRIDGE_CLIENT_CERT", "MT5_BRIDGE_CLIENT_KEY", "MT5_BRIDGE_PORT",
    "MT5_BRIDGE_SERVER_CERT", "MT5_BRIDGE_SERVER_KEY", "MT5_BRIDGE_CLIENT_CA", "MT5_BRIDGE_BIND",
    "MT5_BRIDGE_PRINCIPAL_SHA256", "MT5_ORGANIZATION_ID", "MT5_EXTERNAL_ACCOUNT_ID",
    "MT5_BINDING_ID", "MT5_BROKER_ID", "MT5_BROKER_SERVER_ID", "MT5_PASSWORD_DPAPI_FILE"
)
foreach ($name in $requiredEnvironment) {
    $value = [Environment]::GetEnvironmentVariable($name)
    $present = -not [string]::IsNullOrWhiteSpace($value) -and $value.Trim() -eq $value
    Add-Check ("environment:" + $name) $present $(if ($present) { "Present; value intentionally not displayed." } else { "Missing or whitespace-padded; value intentionally not displayed." })
}
$loginValue = [Environment]::GetEnvironmentVariable("MT5_LOGIN")
Add-Check "numeric_demo_login_configured" ($loginValue -match '^[0-9]+
Add-Check "plaintext_password_absent" ([string]::IsNullOrEmpty($plainPassword)) "MT5_PASSWORD must not be set; use the CurrentUser DPAPI file."

$dpapiPath = [Environment]::GetEnvironmentVariable("MT5_PASSWORD_DPAPI_FILE")
if (-not [string]::IsNullOrWhiteSpace($dpapiPath)) {
    Add-Check "dpapi_secret_file_exists" (Test-Path -LiteralPath $dpapiPath -PathType Leaf) "DPAPI secret-file presence only; content is never read."
}
foreach ($name in @("MT5_BRIDGE_CLIENT_CERT", "MT5_BRIDGE_CLIENT_KEY", "MT5_BRIDGE_SERVER_CERT", "MT5_BRIDGE_SERVER_KEY")) {
    $path = [Environment]::GetEnvironmentVariable($name)
    if (-not [string]::IsNullOrWhiteSpace($path)) {
        Add-Check ("file:" + $name) (Test-Path -LiteralPath $path -PathType Leaf) "Configured file presence only; key/certificate contents are not read."
    }
}
$pin = [Environment]::GetEnvironmentVariable("MT5_BRIDGE_PRINCIPAL_SHA256")
Add-Check "worker_certificate_pin_format" ($pin -match '^[0-9a-fA-F]{64}$') "Expected 64-character SHA-256 fingerprint; value not displayed."

$record = [ordered]@{
    schema_version = 1
    check_type = "read_only_windows_deployment_preflight"
    captured_at_utc = [DateTimeOffset]::UtcNow.ToString("o")
    host = [ordered]@{
        windows_caption = [string]$os.Caption
        windows_build = [string]$os.BuildNumber
        architecture = [string]$os.OSArchitecture
    }
    artifacts = [ordered]@{
        terminal_path = if ($null -ne $terminal) { $terminal.FullName } else { $null }
        terminal_sha256 = $terminalHash
        daemon_sha256 = $daemonHash
        python_path = if ($null -ne $python) { $python.FullName } else { $null }
    }
    require_running_services = [bool]$RequireRunningServices
    checks = @($checks)
    result = if ($blockers.Count -eq 0) { "PASS_PREFLIGHT_ONLY" } else { "BLOCKED" }
    blocker_count = $blockers.Count
    blockers = @($blockers)
    limitations = @(
        "Does not connect to MT5 or PostgreSQL.",
        "Does not authenticate broker ownership or verify DEMO status.",
        "Does not validate certificate chains, revocation, private-key ACLs, or secret-manager custody.",
        "Does not execute the 21 acceptance tests or authorize release.",
        "A PASS_PREFLIGHT_ONLY result is not an end-to-end acceptance pass."
    )
}
$full = [IO.Path]::GetFullPath($EvidencePath)
$parent = Split-Path -Parent $full
if ($parent -and -not (Test-Path -LiteralPath $parent)) { New-Item -ItemType Directory -Path $parent -Force | Out-Null }
$record | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath $full -Encoding utf8
Write-Host ("Preflight result: {0}; checks: {1}; blockers: {2}" -f $record.result, $checks.Count, $blockers.Count)
Write-Host ("Redacted evidence: {0}" -f $full)
if ($blockers.Count -gt 0) { exit 1 }
exit 0
) "Expected numeric broker login is configured; value is not displayed."
$plainPassword = [Environment]::GetEnvironmentVariable("MT5_PASSWORD")
Add-Check "plaintext_password_absent" ([string]::IsNullOrEmpty($plainPassword)) "MT5_PASSWORD must not be set; use the CurrentUser DPAPI file."

$dpapiPath = [Environment]::GetEnvironmentVariable("MT5_PASSWORD_DPAPI_FILE")
if (-not [string]::IsNullOrWhiteSpace($dpapiPath)) {
    Add-Check "dpapi_secret_file_exists" (Test-Path -LiteralPath $dpapiPath -PathType Leaf) "DPAPI secret-file presence only; content is never read."
}
foreach ($name in @("MT5_BRIDGE_CLIENT_CERT", "MT5_BRIDGE_CLIENT_KEY", "MT5_BRIDGE_SERVER_CERT", "MT5_BRIDGE_SERVER_KEY")) {
    $path = [Environment]::GetEnvironmentVariable($name)
    if (-not [string]::IsNullOrWhiteSpace($path)) {
        Add-Check ("file:" + $name) (Test-Path -LiteralPath $path -PathType Leaf) "Configured file presence only; key/certificate contents are not read."
    }
}
$pin = [Environment]::GetEnvironmentVariable("MT5_BRIDGE_PRINCIPAL_SHA256")
Add-Check "worker_certificate_pin_format" ($pin -match '^[0-9a-fA-F]{64}$') "Expected 64-character SHA-256 fingerprint; value not displayed."

$record = [ordered]@{
    schema_version = 1
    check_type = "read_only_windows_deployment_preflight"
    captured_at_utc = [DateTimeOffset]::UtcNow.ToString("o")
    host = [ordered]@{
        windows_caption = [string]$os.Caption
        windows_build = [string]$os.BuildNumber
        architecture = [string]$os.OSArchitecture
    }
    artifacts = [ordered]@{
        terminal_path = if ($null -ne $terminal) { $terminal.FullName } else { $null }
        terminal_sha256 = $terminalHash
        daemon_sha256 = $daemonHash
        python_path = if ($null -ne $python) { $python.FullName } else { $null }
    }
    require_running_services = [bool]$RequireRunningServices
    checks = @($checks)
    result = if ($blockers.Count -eq 0) { "PASS_PREFLIGHT_ONLY" } else { "BLOCKED" }
    blocker_count = $blockers.Count
    blockers = @($blockers)
    limitations = @(
        "Does not connect to MT5 or PostgreSQL.",
        "Does not authenticate broker ownership or verify DEMO status.",
        "Does not validate certificate chains, revocation, private-key ACLs, or secret-manager custody.",
        "Does not execute the 21 acceptance tests or authorize release.",
        "A PASS_PREFLIGHT_ONLY result is not an end-to-end acceptance pass."
    )
}
$full = [IO.Path]::GetFullPath($EvidencePath)
$parent = Split-Path -Parent $full
if ($parent -and -not (Test-Path -LiteralPath $parent)) { New-Item -ItemType Directory -Path $parent -Force | Out-Null }
$record | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath $full -Encoding utf8
Write-Host ("Preflight result: {0}; checks: {1}; blockers: {2}" -f $record.result, $checks.Count, $blockers.Count)
Write-Host ("Redacted evidence: {0}" -f $full)
if ($blockers.Count -gt 0) { exit 1 }
exit 0
