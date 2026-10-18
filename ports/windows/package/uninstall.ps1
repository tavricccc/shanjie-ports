# Removes what install.ps1 installed. UNVERIFIED ON WINDOWS. Run from an elevated PowerShell:
#   powershell -ExecutionPolicy Bypass -File "%ProgramFiles%\ShanjiePorts\uninstall.ps1" [-DeleteUserData]
# User files (%LOCALAPPDATA%\Shanjie: learning and settings) are kept unless -DeleteUserData is given; then they are
# moved to %LOCALAPPDATA%\Shanjie.deleted_files\<time> rather than erased.
param([switch]$DeleteUserData)
$ErrorActionPreference = 'Stop'
$root = Join-Path $env:ProgramFiles 'ShanjiePorts'
$tip = '0404:{5F0C4A5E-9D3B-4F2E-8C61-2A7B3E9D4C10}{5F0C4A5E-9D3B-4F2E-8C61-2A7B3E9D4C11}'

Import-Module International
$list = Get-WinUserLanguageList
foreach ($language in $list) {
    if ($language.InputMethodTips -contains $tip) { [void]$language.InputMethodTips.Remove($tip) }
}
Set-WinUserLanguageList -LanguageList $list -Force

$current = Join-Path $root 'current'
if (Test-Path -LiteralPath $current) {
    $dll = Join-Path (Join-Path $root (Get-Content -LiteralPath $current -Raw).Trim()) 'shanjie_tsf.dll'
    if (Test-Path -LiteralPath $dll) {
        Start-Process -FilePath "$env:SystemRoot\System32\regsvr32.exe" -ArgumentList @('/s', '/u', "`"$dll`"") -Wait | Out-Null
    }
}
Remove-ItemProperty -Path 'HKLM:\Software\Microsoft\Windows\CurrentVersion\Run' -Name 'ShanjiePortsHost' -ErrorAction SilentlyContinue
Get-Process -Name 'shanjie-host' -ErrorAction SilentlyContinue | Stop-Process -Force -ErrorAction SilentlyContinue
Remove-Item -LiteralPath $root -Recurse -Force -ErrorAction SilentlyContinue
if (Test-Path -LiteralPath $root) {
    Write-Host "Some files are still in use by running apps; $root is removed after they quit (or at the next sign-in run of this script)."
}
if ($DeleteUserData) {
    $data = Join-Path $env:LOCALAPPDATA 'Shanjie'
    if (Test-Path -LiteralPath $data) {
        $aside = Join-Path $env:LOCALAPPDATA ('Shanjie.deleted_files\' + (Get-Date -Format 'yyyyMMdd-HHmmss'))
        New-Item -ItemType Directory -Force -Path (Split-Path -Parent $aside) | Out-Null
        Move-Item -LiteralPath $data -Destination $aside
    }
}
Write-Host 'Removed Shanjie.'
