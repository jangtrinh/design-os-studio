#Requires -Version 5.1
<#
.SYNOPSIS
    Installs design:os studio (GUI + CLI + MCP + Diff) for the current Windows user.
    No Administrator privileges required.
.DESCRIPTION
    1. Copies designos-studio.exe, designos.exe, designos-mcp.exe, designos-diff.exe
       into $env:LOCALAPPDATA\Programs\designos\bin
    2. Adds $env:LOCALAPPDATA\Programs\designos\bin to the User PATH
    3. Creates Start Menu and Desktop shortcuts for design:os studio
    4. Registers .designos and .fig file associations under HKCU\Software\Classes
#>
param(
    [string]$InstallRoot = "$env:LOCALAPPDATA\Programs\designos",
    [switch]$NoDesktopShortcut
)

$ErrorActionPreference = "Stop"
$SourceDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$BinDir = Join-Path $InstallRoot "bin"
$LicensesDir = Join-Path $InstallRoot "licenses"

New-Item -ItemType Directory -Force -Path $BinDir | Out-Null
New-Item -ItemType Directory -Force -Path $LicensesDir | Out-Null

$Binaries = @("designos-studio.exe", "designos.exe", "designos-mcp.exe", "designos-diff.exe")
foreach ($bin in $Binaries) {
    $src = Join-Path $SourceDir $bin
    if (-not (Test-Path $src)) {
        throw "Missing expected binary: $src"
    }
    Copy-Item -Force $src (Join-Path $BinDir $bin)
    # Also keep a copy in $InstallRoot for NSIS / direct launcher compatibility
    Copy-Item -Force $src (Join-Path $InstallRoot $bin)
}

foreach ($doc in @("README.md", "LICENSE", "designos.ico", "uninstall.ps1")) {
    $p = Join-Path $SourceDir $doc
    if (Test-Path $p) {
        Copy-Item -Force $p (Join-Path $InstallRoot $doc)
    }
}

$SrcLicenses = Join-Path $SourceDir "licenses"
if (Test-Path $SrcLicenses) {
    Copy-Item -Force (Join-Path $SrcLicenses "*") $LicensesDir
}

# Add $BinDir to User PATH if not already present
$UserPath = [Environment]::GetEnvironmentVariable("Path", "User")
if (-not $UserPath) { $UserPath = "" }
$PathEntries = $UserPath -split ";" | Where-Object { $_ -ne "" }
if ($PathEntries -notcontains $BinDir) {
    $NewPath = ($PathEntries + $BinDir) -join ";"
    [Environment]::SetEnvironmentVariable("Path", $NewPath, "User")
    $env:Path = "$BinDir;$env:Path"
    Write-Host "Added $BinDir to User PATH."
}

# Create Start Menu & Desktop shortcuts
$WshShell = New-Object -ComObject WScript.Shell
$StartMenuDir = Join-Path ([Environment]::GetFolderPath("Programs")) "designos"
New-Item -ItemType Directory -Force -Path $StartMenuDir | Out-Null

$StudioExe = Join-Path $BinDir "designos-studio.exe"
$StartShortcut = $WshShell.CreateShortcut((Join-Path $StartMenuDir "designos studio.lnk"))
$StartShortcut.TargetPath = $StudioExe
$StartShortcut.WorkingDirectory = $InstallRoot
$StartShortcut.Description = "design:os studio — vector & UI design studio"
$StartShortcut.Save()

if (-not $NoDesktopShortcut) {
    $DesktopDir = [Environment]::GetFolderPath("Desktop")
    $DesktopShortcut = $WshShell.CreateShortcut((Join-Path $DesktopDir "designos studio.lnk"))
    $DesktopShortcut.TargetPath = $StudioExe
    $DesktopShortcut.WorkingDirectory = $InstallRoot
    $DesktopShortcut.Description = "design:os studio — vector & UI design studio"
    $DesktopShortcut.Save()
}

# Register .designos and .fig file extensions in HKCU
New-Item -Force -Path "HKCU:\Software\Classes\.designos" | Set-ItemProperty -Name "(default)" -Value "designos.Document"
New-Item -Force -Path "HKCU:\Software\Classes\designos.Document" | Set-ItemProperty -Name "(default)" -Value "design:os studio Document"
New-Item -Force -Path "HKCU:\Software\Classes\designos.Document\DefaultIcon" | Set-ItemProperty -Name "(default)" -Value "`"$StudioExe`",0"
New-Item -Force -Path "HKCU:\Software\Classes\designos.Document\shell\open\command" | Set-ItemProperty -Name "(default)" -Value "`"$StudioExe`" `"%1`""

New-Item -Force -Path "HKCU:\Software\Classes\.fig" | Set-ItemProperty -Name "(default)" -Value "designos.FigmaDocument"
New-Item -Force -Path "HKCU:\Software\Classes\designos.FigmaDocument" | Set-ItemProperty -Name "(default)" -Value "Figma Design File (design:os studio)"
New-Item -Force -Path "HKCU:\Software\Classes\designos.FigmaDocument\DefaultIcon" | Set-ItemProperty -Name "(default)" -Value "`"$StudioExe`",0"
New-Item -Force -Path "HKCU:\Software\Classes\designos.FigmaDocument\shell\open\command" | Set-ItemProperty -Name "(default)" -Value "`"$StudioExe`" `"%1`""

Write-Host "design:os studio installed successfully to $InstallRoot"
Write-Host "CLI tools available: designos-studio, designos, designos-mcp, designos-diff"
