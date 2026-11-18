[CmdletBinding()]
param(
    [string]$OutputPath = (Join-Path $env:ProgramData "QuantOS\\secrets\\mt5-password.dpapi")
)

$ErrorActionPreference = "Stop"

if ($env:OS -ne "Windows_NT") {
    throw "This helper must run on the Windows host under the dedicated MT5 worker service identity."
}

$protectedDataAvailable = $false
foreach ($assembly in @("System.Security", "System.Security.Cryptography.ProtectedData")) {
    try {
        Add-Type -AssemblyName $assembly -ErrorAction Stop
        $protectedDataAvailable = $true
    } catch {
        # Try the other assembly identity; the type name is checked below.
    }
}
if (-not $protectedDataAvailable -or
    -not ("System.Security.Cryptography.ProtectedData" -as [type])) {
    throw "The .NET ProtectedData type is unavailable on this Windows PowerShell/.NET runtime."
}

$identity = [System.Security.Principal.WindowsIdentity]::GetCurrent().Name
if ([string]::IsNullOrWhiteSpace($identity)) {
    throw "Could not resolve the current Windows service identity."
}

$secure = Read-Host "Enter the DEMO broker password for $identity" -AsSecureString
if ($secure.Length -eq 0) {
    throw "A non-empty DEMO password is required."
}

$bstr = [IntPtr]::Zero
$plainBytes = $null
$protectedBytes = $null
try {
    $bstr = [Runtime.InteropServices.Marshal]::SecureStringToBSTR($secure)
    $plain = [Runtime.InteropServices.Marshal]::PtrToStringBSTR($bstr)
    $plainBytes = [Text.Encoding]::UTF8.GetBytes($plain)
    $protectedBytes = [Security.Cryptography.ProtectedData]::Protect(
        $plainBytes,
        $null,
        [Security.Cryptography.DataProtectionScope]::CurrentUser
    )

    $directory = Split-Path -Parent $OutputPath
    New-Item -ItemType Directory -Force -Path $directory | Out-Null
    $resolved = [IO.Path]::GetFullPath($OutputPath)
    [IO.File]::WriteAllText($resolved, [Convert]::ToBase64String($protectedBytes), [Text.Encoding]::ASCII)

    # Remove inherited access and grant only the service identity plus local
    # recovery principals. DPAPI CurrentUser still requires the same identity
    # that created this blob to decrypt it.
    & icacls $resolved /inheritance:r /grant:r "$($identity):(F)" "SYSTEM:(F)" "BUILTIN\\Administrators:(F)" | Out-Null
    if ($LASTEXITCODE -ne 0) {
        Remove-Item -LiteralPath $resolved -Force -ErrorAction SilentlyContinue
        throw "Could not restrict ACLs on the protected secret file."
    }

    Write-Host "Protected broker secret written for Windows identity: $identity"
    Write-Host "Secret file: $resolved"
    Write-Host "Set MT5_PASSWORD_DPAPI_FILE to this path for the worker."
    Write-Host "The same Windows identity must run the worker; do not copy this file to another host or account."
}
finally {
    if ($null -ne $plainBytes) { [Array]::Clear($plainBytes, 0, $plainBytes.Length) }
    if ($null -ne $protectedBytes) { [Array]::Clear($protectedBytes, 0, $protectedBytes.Length) }
    if ($bstr -ne [IntPtr]::Zero) { [Runtime.InteropServices.Marshal]::ZeroFreeBSTR($bstr) }
    if ($null -ne $secure) { $secure.Dispose() }
}
