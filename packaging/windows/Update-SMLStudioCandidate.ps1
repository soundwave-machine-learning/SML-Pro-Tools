<#
.SYNOPSIS
  Reversibly replace a portable SML Studio (SoundCraft) candidate folder with a new portable zip.

.DESCRIPTION
  Written for Hotfix Pass 01; works for any soundcraft-<version>-windows-x64-gnu-portable.zip
  made by packaging/windows/cross-build.sh.

  What it does, in order (any failure stops it and leaves the old folder in place):
    1. Checks the zip against its .sha256 file (or -ExpectedSha256).
    2. Checks the install folder holds soundcraft.exe and records the old build (hash, MANIFEST.json).
    3. Refuses to continue while soundcraft.exe or soundcraft-cli.exe from that folder is running.
       It never stops the program: close SML Studio yourself after saving your session.
    4. Unpacks the zip into a staging folder beside the install folder and checks every file
       against the zip's MANIFEST.json (SHA-256).
    5. Copies the per-user settings folder (%APPDATA%\SoundCraft, top-level files only: ui.json,
       shortcuts and the like) into the backup as well. The original settings are not changed.
    6. Renames the install folder to <folder>.backup-<timestamp> and moves the new build into
       place. Anything in the old folder that did not come from a package (for example sessions
       you saved there) is copied into the new folder too, and listed.
    7. Verifies the installed soundcraft.exe hash and writes update-<timestamp>.log in the backup.

  Sessions saved anywhere else, autosaves and preferences are not touched. Nothing is deleted:
  the old build stays complete in the backup folder until you remove it yourself.

  Roll back with:  .\Update-SMLStudioCandidate.ps1 -Rollback -InstallDir <folder> -BackupDir <backup>

.PARAMETER InstallDir
  The folder that contains the soundcraft.exe you run today (the extracted portable candidate).

.PARAMETER ZipPath
  The new portable zip. Default: the soundcraft-*-windows-x64-gnu-portable.zip next to this script.

.PARAMETER ExpectedSha256
  The zip's SHA-256. Default: read from <zip>.sha256 next to the zip.

.EXAMPLE
  .\Update-SMLStudioCandidate.ps1 -InstallDir "$env:USERPROFILE\Desktop\SML Studio" -WhatIf
  Shows what would happen without changing anything.

.NOTES
  Needs Windows PowerShell 5.1 or PowerShell 7. No administrator rights unless the install folder
  itself needs them. If Windows blocks the script: Unblock-File .\Update-SMLStudioCandidate.ps1
  (or run: powershell -ExecutionPolicy Bypass -File .\Update-SMLStudioCandidate.ps1 ...).
  This script was written in a Linux build container and has NOT been run on Windows yet; use
  -WhatIf first.
#>
[CmdletBinding(SupportsShouldProcess = $true)]
param(
    [Parameter(Mandatory = $true)][string]$InstallDir,
    [string]$ZipPath,
    [string]$ExpectedSha256,
    [switch]$Rollback,
    [string]$BackupDir
)
Set-StrictMode -Version 2.0
$ErrorActionPreference = 'Stop'

function Get-Sha256([string]$Path) { (Get-FileHash -LiteralPath $Path -Algorithm SHA256).Hash.ToLowerInvariant() }

function Assert-NotRunning([string]$Dir) {
    $sep = [System.IO.Path]::DirectorySeparatorChar
    $full = [System.IO.Path]::GetFullPath($Dir).TrimEnd($sep) + $sep
    $running = @(Get-Process -Name soundcraft, soundcraft-cli -ErrorAction SilentlyContinue | Where-Object {
            try { $_.Path -and $_.Path.StartsWith($full, [System.StringComparison]::OrdinalIgnoreCase) } catch { $false }
        })
    if ($running.Count -gt 0) {
        $list = ($running | ForEach-Object { "$($_.ProcessName) (PID $($_.Id))" }) -join ', '
        throw "SML Studio is running from $Dir ($list). Save your session, close SML Studio, then run this again. Nothing was changed."
    }
}

$InstallDir = [System.IO.Path]::GetFullPath($InstallDir).TrimEnd('\')
$stamp = Get-Date -Format 'yyyyMMdd-HHmmss'

if ($Rollback) {
    if (-not $BackupDir) { throw '-Rollback needs -BackupDir (the <folder>.backup-<timestamp> to restore).' }
    $BackupDir = [System.IO.Path]::GetFullPath($BackupDir).TrimEnd('\')
    if (-not (Test-Path -LiteralPath (Join-Path $BackupDir 'soundcraft.exe'))) { throw "No soundcraft.exe in $BackupDir" }
    Assert-NotRunning $InstallDir
    $replaced = "$InstallDir.replaced-$stamp"
    if ($PSCmdlet.ShouldProcess($InstallDir, "move the current build to $replaced and restore $BackupDir")) {
        if (Test-Path -LiteralPath $InstallDir) { Rename-Item -LiteralPath $InstallDir -NewName (Split-Path $replaced -Leaf) }
        Rename-Item -LiteralPath $BackupDir -NewName (Split-Path $InstallDir -Leaf)
        Write-Host "Restored $BackupDir to $InstallDir. The build that was there is kept in $replaced."
    }
    return
}

# 1. The package.
if (-not $ZipPath) {
    $ZipPath = @(Get-ChildItem -LiteralPath $PSScriptRoot -Filter 'soundcraft-*-windows-x64-gnu-portable.zip' | Sort-Object LastWriteTime -Descending | Select-Object -First 1 -ExpandProperty FullName)[0]
    if (-not $ZipPath) { throw "No soundcraft-*-windows-x64-gnu-portable.zip next to this script; pass -ZipPath." }
}
$ZipPath = [System.IO.Path]::GetFullPath($ZipPath)
if (-not $ExpectedSha256) {
    $sumFile = "$ZipPath.sha256"
    if (-not (Test-Path -LiteralPath $sumFile)) { throw "No $sumFile; pass -ExpectedSha256 (from the delivered checksum list)." }
    $ExpectedSha256 = ((Get-Content -LiteralPath $sumFile -TotalCount 1) -split '\s+')[0]
}
$zipHash = Get-Sha256 $ZipPath
if ($zipHash -ne $ExpectedSha256.ToLowerInvariant()) { throw "Checksum mismatch for $ZipPath`n expected $ExpectedSha256`n actual   $zipHash`nNothing was changed." }
Write-Host "Package OK: $ZipPath`n  sha256 $zipHash"

# 2. The current install.
$oldExe = Join-Path $InstallDir 'soundcraft.exe'
if (-not (Test-Path -LiteralPath $oldExe)) { throw "No soundcraft.exe in $InstallDir. Pass the folder you run SML Studio from." }
$oldHash = Get-Sha256 $oldExe
$oldCommit = 'unknown (no MANIFEST.json)'
$oldManifestPath = Join-Path $InstallDir 'MANIFEST.json'
$oldFiles = @()
if (Test-Path -LiteralPath $oldManifestPath) {
    $om = Get-Content -LiteralPath $oldManifestPath -Raw | ConvertFrom-Json
    $oldCommit = $om.source.commit
    $oldFiles = @($om.files.PSObject.Properties.Name)
}
Write-Host "Current build: $oldExe`n  sha256 $oldHash`n  commit $oldCommit"

# 3. Never replace a running program.
Assert-NotRunning $InstallDir

$parent = Split-Path $InstallDir -Parent
$leaf = Split-Path $InstallDir -Leaf
$staging = Join-Path $parent "$leaf.staging-$stamp"
$backup = Join-Path $parent "$leaf.backup-$stamp"
if (-not $PSCmdlet.ShouldProcess($InstallDir, "replace with $ZipPath (old build kept in $backup)")) {
    Write-Host "WhatIf: would unpack to $staging, verify, back up to $backup and install into $InstallDir."
    return
}

# 4. Unpack and verify before touching the install.
Expand-Archive -LiteralPath $ZipPath -DestinationPath $staging
$inner = @(Get-ChildItem -LiteralPath $staging -Directory)
if ($inner.Count -ne 1) { throw "Unexpected zip layout in $staging (expected one top folder). The install was not changed." }
$newRoot = $inner[0].FullName
$nm = Get-Content -LiteralPath (Join-Path $newRoot 'MANIFEST.json') -Raw | ConvertFrom-Json
foreach ($p in $nm.files.PSObject.Properties) {
    $f = Join-Path $newRoot $p.Name
    if (-not (Test-Path -LiteralPath $f)) { throw "Package file missing: $($p.Name). The install was not changed." }
    if ((Get-Sha256 $f) -ne $p.Value.sha256) { throw "Package file hash mismatch: $($p.Name). The install was not changed." }
}
Write-Host "Unpacked and verified $($nm.files.PSObject.Properties.Name.Count) files: version $($nm.version), commit $($nm.source.commit)"

# 5 + 6. Back up, then swap (rename on the same volume; rolled back on failure).
# Files that came with this or an earlier package (earlier zips had no MANIFEST.json and shipped
# these documents); everything else in the folder is treated as yours and carried over.
$packaged = @('soundcraft.exe', 'soundcraft-cli.exe', 'LICENSE-MIT', 'LICENSE-APACHE', 'README.txt', 'MANIFEST.json', 'docs',
    'USER_GUIDE.md', 'README-FIRST.txt', 'NOTICE', 'ATTRIBUTION.md', 'TARGET_MACHINE_GAUNTLET.md') + $oldFiles
$extras = @(Get-ChildItem -LiteralPath $InstallDir -Force | Where-Object { $packaged -notcontains $_.Name })
Rename-Item -LiteralPath $InstallDir -NewName (Split-Path $backup -Leaf)
try {
    Move-Item -LiteralPath $newRoot -Destination $InstallDir
    foreach ($x in $extras) { Copy-Item -LiteralPath (Join-Path $backup $x.Name) -Destination $InstallDir -Recurse }
    $newHash = Get-Sha256 (Join-Path $InstallDir 'soundcraft.exe')
    if ($newHash -ne $nm.files.'soundcraft.exe'.sha256) { throw "Installed soundcraft.exe hash $newHash does not match the package." }
}
catch {
    Write-Warning "Install failed: $($_.Exception.Message). Restoring the previous build."
    if (Test-Path -LiteralPath $InstallDir) { Rename-Item -LiteralPath $InstallDir -NewName "$leaf.failed-$stamp" }
    Rename-Item -LiteralPath $backup -NewName $leaf
    throw
}
Remove-Item -LiteralPath $staging -Recurse -Force

$settings = if ($env:APPDATA) { Join-Path $env:APPDATA 'SoundCraft' } else { $null }
$settingsCopy = 'none found'
if ($settings -and (Test-Path -LiteralPath $settings)) {
    $dest = Join-Path $backup 'settings-copy'
    New-Item -ItemType Directory -Path $dest | Out-Null
    Get-ChildItem -LiteralPath $settings -File | Copy-Item -Destination $dest
    $settingsCopy = "$settings (top-level files) copied to $dest; the originals are unchanged"
}

# 7. Record what happened.
$extrasText = if ($extras.Count) { (@($extras | ForEach-Object { $_.Name })) -join ', ' } else { 'nothing' }
$log = @(
    "SML Studio candidate update $stamp",
    "install folder: $InstallDir",
    "old build: soundcraft.exe sha256 $oldHash, commit $oldCommit -> kept in $backup",
    "new build: version $($nm.version), commit $($nm.source.commit), soundcraft.exe sha256 $newHash",
    "package: $ZipPath sha256 $zipHash",
    "copied from the old folder: $extrasText",
    "settings: $settingsCopy",
    "rollback: .\Update-SMLStudioCandidate.ps1 -Rollback -InstallDir `"$InstallDir`" -BackupDir `"$backup`""
)
$log | Set-Content -LiteralPath (Join-Path $backup "update-$stamp.log") -Encoding UTF8
$log | ForEach-Object { Write-Host $_ }
Write-Host "`nDone. Start SML Studio from $InstallDir; SML Studio > About shows the version, and Get-FileHash .\soundcraft.exe should print $newHash."
