[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)]
    [string]$ExpectedServiceIdentity,
    [int]$ExpiryWarningDays = 30
)
$ErrorActionPreference = "Stop"
if ($ExpiryWarningDays -lt 1 -or $ExpiryWarningDays -gt 365) { throw "ExpiryWarningDays must be between 1 and 365." }
if ($env:OS -ne "Windows_NT") { throw "Certificate/ACL review must run on the target Windows MT5 host." }

function Require-EnvPath([string]$Name) {
    $value = [Environment]::GetEnvironmentVariable($Name)
    if ([string]::IsNullOrWhiteSpace($value) -or $value.Trim() -ne $value) { throw "Required environment variable is missing or invalid: $Name" }
    if (-not (Test-Path -LiteralPath $value -PathType Leaf)) { throw "Configured certificate/key file does not exist: $Name" }
    return (Resolve-Path -LiteralPath $value).Path
}
function Test-PrivateKeyAcl([string]$Label, [string]$Path) {
    $acl = Get-Acl -LiteralPath $Path
    $broad = @("Everyone", "NT AUTHORITY\Authenticated Users", "BUILTIN\Users", "$env:COMPUTERNAME\Users", "CREATOR OWNER")
    $approved = @($ExpectedServiceIdentity, "NT AUTHORITY\SYSTEM", "BUILTIN\Administrators")
    $failures = [System.Collections.Generic.List[string]]::new()
    Write-Host "ACL $Label:"
    foreach ($rule in $acl.Access) {
        $identity = [string]$rule.IdentityReference
        Write-Host ("  {0} | {1} | {2} | inherited={3}" -f $identity, $rule.AccessControlType, $rule.FileSystemRights, $rule.IsInherited)
        $rights = $rule.FileSystemRights
        $hasRead = (($rights -band [System.Security.AccessControl.FileSystemRights]::ReadData) -ne 0) -or
                   (($rights -band [System.Security.AccessControl.FileSystemRights]::Read) -ne 0) -or
                   (($rights -band [System.Security.AccessControl.FileSystemRights]::ReadAndExecute) -ne 0) -or
                   (($rights -band [System.Security.AccessControl.FileSystemRights]::FullControl) -ne 0)
        if ($rule.AccessControlType -eq [System.Security.AccessControl.AccessControlType]::Allow -and $hasRead) {
            if ($broad -contains $identity) { $failures.Add("$Label grants read access to broad principal '$identity'") }
            elseif (-not ($approved -contains $identity)) { Write-Warning "$Label grants read access to unreviewed identity '$identity'; verify necessity." }
        }
    }
    $serviceRead = $acl.Access | Where-Object {
        $_.AccessControlType -eq [System.Security.AccessControl.AccessControlType]::Allow -and
        [string]$_.IdentityReference -eq $ExpectedServiceIdentity -and
        ((($_.FileSystemRights -band [System.Security.AccessControl.FileSystemRights]::ReadData) -ne 0) -or
         (($_.FileSystemRights -band [System.Security.AccessControl.FileSystemRights]::Read) -ne 0) -or
         (($_.FileSystemRights -band [System.Security.AccessControl.FileSystemRights]::FullControl) -ne 0))
    }
    if (-not $serviceRead) { $failures.Add("$Label does not grant expected service identity '$ExpectedServiceIdentity' read access") }
    if ($failures.Count) { $failures | ForEach-Object { Write-Error $_ }; return $false }
    return $true
}

$clientCert = Require-EnvPath "MT5_BRIDGE_CLIENT_CERT"
$clientKey = Require-EnvPath "MT5_BRIDGE_CLIENT_KEY"
$serverCert = Require-EnvPath "MT5_BRIDGE_SERVER_CERT"
$serverKey = Require-EnvPath "MT5_BRIDGE_SERVER_KEY"
$pin = [Environment]::GetEnvironmentVariable("MT5_BRIDGE_PRINCIPAL_SHA256")
if ([string]::IsNullOrWhiteSpace($pin) -or $pin -notmatch '^[0-9a-fA-F]{64}$') { throw "MT5_BRIDGE_PRINCIPAL_SHA256 must be a 64-character hex fingerprint." }

$pythonProbe = @'
import hashlib, json, os, ssl, sys
from datetime import datetime, timezone
def cert(path):
    decoded = ssl._ssl._test_decode_cert(path)
    before = datetime.strptime(decoded["notBefore"], "%b %d %H:%M:%S %Y %Z").replace(tzinfo=timezone.utc)
    after = datetime.strptime(decoded["notAfter"], "%b %d %H:%M:%S %Y %Z").replace(tzinfo=timezone.utc)
    with open(path, "r", encoding="ascii") as stream:
        der = ssl.PEM_cert_to_DER_cert(stream.read())
    return {"subject": decoded.get("subject", []), "issuer": decoded.get("issuer", []),
            "not_before_utc": before.isoformat(), "not_after_utc": after.isoformat(),
            "sha256_der": hashlib.sha256(der).hexdigest()}
try:
    client = cert(os.environ["REVIEW_CLIENT_CERT"])
    server = cert(os.environ["REVIEW_SERVER_CERT"])
    for cert_path, key_path in ((os.environ["REVIEW_CLIENT_CERT"], os.environ["REVIEW_CLIENT_KEY"]),
                                (os.environ["REVIEW_SERVER_CERT"], os.environ["REVIEW_SERVER_KEY"])):
        context = ssl.SSLContext(ssl.PROTOCOL_TLS_CLIENT)
        context.load_cert_chain(certfile=cert_path, keyfile=key_path)
    print(json.dumps({"client": client, "server": server}, separators=(",", ":")))
except Exception as exc:
    print("Certificate parsing or certificate/private-key pairing failed (" + type(exc).__name__ + ").", file=sys.stderr)
    raise SystemExit(1)
'@
$env:REVIEW_CLIENT_CERT = $clientCert
$env:REVIEW_CLIENT_KEY = $clientKey
$env:REVIEW_SERVER_CERT = $serverCert
$env:REVIEW_SERVER_KEY = $serverKey
try {
    $metadataJson = & python -c $pythonProbe
    if ($LASTEXITCODE -ne 0) { throw "Certificate inspection failed. No key material was printed." }
    $metadata = $metadataJson | ConvertFrom-Json
} finally {
    Remove-Item Env:REVIEW_CLIENT_CERT, Env:REVIEW_CLIENT_KEY, Env:REVIEW_SERVER_CERT, Env:REVIEW_SERVER_KEY -ErrorAction SilentlyContinue
}
if ([string]$metadata.client.sha256_der -ne $pin.ToLowerInvariant()) { throw "Client certificate fingerprint does not match MT5_BRIDGE_PRINCIPAL_SHA256." }

$now = [DateTimeOffset]::UtcNow
$threshold = $now.AddDays($ExpiryWarningDays)
$expired = $false
foreach ($entry in @(@{name="client"; cert=$metadata.client}, @{name="server"; cert=$metadata.server})) {
    $before = [DateTimeOffset]::Parse([string]$entry.cert.not_before_utc)
    $after = [DateTimeOffset]::Parse([string]$entry.cert.not_after_utc)
    Write-Host ("CERT {0}: subject={1}; issuer={2}; not_before_utc={3}; not_after_utc={4}; sha256_der={5}" -f $entry.name, ($entry.cert.subject | ConvertTo-Json -Compress -Depth 5), ($entry.cert.issuer | ConvertTo-Json -Compress -Depth 5), $before.ToString("o"), $after.ToString("o"), $entry.cert.sha256_der)
    if ($before -gt $now) { throw "$($entry.name) certificate is not yet valid." }
    if ($after -le $now) { Write-Error "$($entry.name) certificate is expired."; $expired = $true }
    elseif ($after -le $threshold) { Write-Warning "$($entry.name) certificate expires within $ExpiryWarningDays days." }
}
$clientAclOk = Test-PrivateKeyAcl "client private key" $clientKey
$serverAclOk = Test-PrivateKeyAcl "server private key" $serverKey
if ($expired -or -not $clientAclOk -or -not $serverAclOk) { throw "Certificate lifecycle/ACL review FAILED. Do not proceed to venue execution." }
Write-Host "PASS: certificate/key pairs load, client fingerprint matches pin, validity dates inspected, and no broad-principal key-read ACL was found."
Write-Host "MANUAL REQUIRED: record CA trust/revocation method, rotation owner, overlap, rollback, and tested rotation evidence. This script cannot prove revocation status or that rotation/rollback has been exercised."
