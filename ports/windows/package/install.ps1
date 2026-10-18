# Installs the Windows package (ports/windows/scripts/build.sh made this folder). UNVERIFIED ON WINDOWS.
# Run from an elevated PowerShell (Windows PowerShell 5.1 or PowerShell 7):
#   powershell -ExecutionPolicy Bypass -File .\install.ps1
# What it does:
#   - copies shanjie-host.exe, shanjie_tsf.dll and data\ to "%ProgramFiles%\ShanjiePorts\<version>" (a new folder per
#     version: a DLL or a mapped model file in use cannot be replaced, so nothing in use is overwritten);
#   - registers the text service (regsvr32: HKLM CLSID and the zh-TW profile);
#   - starts the host at every sign-in (HKLM Run) and stops a running older host (the next key starts the new one);
#   - adds the input method to the current user's zh-TW language (Settings > Time & language > Language).
# The earlier Windows port (shanjie-multiplatform) uses other identifiers and is left alone; uninstall it separately.
# User files are not touched: learning stays in %LOCALAPPDATA%\Shanjie (the same learning.tsv format), settings go to
# %LOCALAPPDATA%\Shanjie\settings.ini.
$ErrorActionPreference = 'Stop'
$here = Split-Path -Parent $MyInvocation.MyCommand.Path
$version = (Get-Content -LiteralPath (Join-Path $here 'VERSION') -Raw).Trim()
$principal = New-Object Security.Principal.WindowsPrincipal([Security.Principal.WindowsIdentity]::GetCurrent())
if (-not $principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) {
    throw 'Run this script from an elevated PowerShell (Run as administrator).'
}
$root = Join-Path $env:ProgramFiles 'ShanjiePorts'
$dest = Join-Path $root $version
$tip = '0404:{5F0C4A5E-9D3B-4F2E-8C61-2A7B3E9D4C10}{5F0C4A5E-9D3B-4F2E-8C61-2A7B3E9D4C11}'

Get-Process -Name 'shanjie-host' -ErrorAction SilentlyContinue | Stop-Process -Force -ErrorAction SilentlyContinue
if (Test-Path -LiteralPath $dest) { Remove-Item -LiteralPath $dest -Recurse -Force -ErrorAction SilentlyContinue }
New-Item -ItemType Directory -Force -Path $dest | Out-Null
Copy-Item -LiteralPath (Join-Path $here 'files\shanjie-host.exe') -Destination $dest
Copy-Item -LiteralPath (Join-Path $here 'files\shanjie_tsf.dll') -Destination $dest
Copy-Item -LiteralPath (Join-Path $here 'files\data') -Destination $dest -Recurse
Copy-Item -LiteralPath (Join-Path $here 'uninstall.ps1') -Destination $root -Force
Set-Content -LiteralPath (Join-Path $root 'current') -Value $version -Encoding ASCII

$dll = Join-Path $dest 'shanjie_tsf.dll'
$p = Start-Process -FilePath "$env:SystemRoot\System32\regsvr32.exe" -ArgumentList @('/s', "`"$dll`"") -Wait -PassThru
if ($p.ExitCode -ne 0) { throw "regsvr32 failed ($($p.ExitCode))" }

$run = 'HKLM:\Software\Microsoft\Windows\CurrentVersion\Run'
Set-ItemProperty -Path $run -Name 'ShanjiePortsHost' -Value "`"$(Join-Path $dest 'shanjie-host.exe')`""

# Older versions: removed when nothing holds them any more (a DLL still loaded in an app stays until that app quits).
Get-ChildItem -LiteralPath $root -Directory | Where-Object { $_.Name -ne $version } | ForEach-Object {
    Remove-Item -LiteralPath $_.FullName -Recurse -Force -ErrorAction SilentlyContinue
}

Import-Module International
$list = Get-WinUserLanguageList
$zh = $list | Where-Object { $_.LanguageTag -eq 'zh-Hant-TW' } | Select-Object -First 1
if (-not $zh) {
    $list.Add('zh-Hant-TW')
    $zh = $list | Where-Object { $_.LanguageTag -eq 'zh-Hant-TW' } | Select-Object -First 1
}
if (-not ($zh.InputMethodTips -contains $tip)) {
    $zh.InputMethodTips.Add($tip)
    Set-WinUserLanguageList -LanguageList $list -Force
}
Write-Host "Installed Shanjie $version to $dest. Choose it with Win+Space."
