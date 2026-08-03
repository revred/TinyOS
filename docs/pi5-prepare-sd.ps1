<#
.SYNOPSIS
  Prepares an SD card for the TinyOS Pi 5 board session (runbook steps 1-2):
  formats the card FAT32/MBR and stages kernel8.img + config.txt, verified.

.DESCRIPTION
  Owned by docs/pi5-board-session-runbook.md. Run from an ELEVATED PowerShell:

    powershell -ExecutionPolicy Bypass -File docs\pi5-prepare-sd.ps1 -DiskNumber 1

  Safety rails, in order:
    - refuses disk 0 outright (the system disk);
    - refuses any disk that is not removable/USB/SD-attached;
    - refuses a disk carrying more than 1 GiB of data unless -Force;
    - shows exactly what it will erase and requires you to type YES.

  The card is DESTRUCTIVELY repartitioned: MBR table, one 2 GiB FAT32
  partition labelled TOS64BOOT (the Pi 5 EEPROM bootloader reads the first
  FAT partition; exFAT and the card's factory format are unreadable to it;
  Windows cannot FAT32-format >32 GiB volumes, hence the small partition).

  It then copies kernel8.img and config.txt from os\target\pi5\ (which must
  have been built by `cargo run -p xtask -- pi5 --fixture=boot` at the commit
  you intend to capture with) and verifies the copied kernel8.img's SHA-256
  against the build output byte for byte. A mismatched copy is deleted, not
  tolerated: the run record binds capture to image hash.
#>
param(
    [Parameter(Mandatory = $true)]
    [int]$DiskNumber,

    # Skip the data-present guard (never the disk-0 or non-removable guards).
    [switch]$Force
)

$ErrorActionPreference = 'Stop'
$repoRoot = Split-Path -Parent $PSScriptRoot
$stageDir = Join-Path $repoRoot 'os\target\pi5'
$kernel   = Join-Path $stageDir 'kernel8.img'
$config   = Join-Path $stageDir 'config.txt'

# --- rail 0: the staged files must exist before we erase anything ----------
foreach ($f in @($kernel, $config)) {
    if (-not (Test-Path $f)) {
        throw "missing $f - build first: cd os; cargo run -p xtask -- pi5 --fixture=boot"
    }
}
$sourceHash = (Get-FileHash -Algorithm SHA256 $kernel).Hash.ToLower()

# --- rail 1: never the system disk -----------------------------------------
if ($DiskNumber -eq 0) { throw 'refusing disk 0: that is the system disk' }

# --- rail 2: must be a removable/SD/USB disk --------------------------------
$disk = Get-Disk -Number $DiskNumber
$busOk = $disk.BusType -in @('USB', 'SD', 'MMC')
if (-not $busOk) {
    throw "refusing disk $($DiskNumber): bus type '$($disk.BusType)' is not USB/SD/MMC"
}

# --- rail 3: refuse a card that carries data, unless -Force -----------------
$usedBytes = 0
Get-Partition -DiskNumber $DiskNumber -ErrorAction SilentlyContinue | ForEach-Object {
    $v = $_ | Get-Volume -ErrorAction SilentlyContinue
    if ($v) { $usedBytes += ($v.Size - $v.SizeRemaining) }
}
if (($usedBytes -gt 1GB) -and (-not $Force)) {
    throw ("refusing: disk {0} carries ~{1:N1} GiB of data; re-run with -Force only if " -f $DiskNumber, ($usedBytes / 1GB)) +
          'you are certain nothing on it matters'
}

# --- the informed-consent gate ----------------------------------------------
Write-Host ''
Write-Host ('About to ERASE disk {0}: {1}, {2:N1} GiB, bus {3}' -f `
    $DiskNumber, $disk.FriendlyName, ($disk.Size / 1GB), $disk.BusType)
Write-Host ('Result: MBR + one 2 GiB FAT32 partition TOS64BOOT carrying:')
Write-Host ("  kernel8.img  sha256 $sourceHash")
Write-Host ('  config.txt   (os_check=0, kernel=kernel8.img)')
$answer = Read-Host 'Type YES to proceed'
if ($answer -cne 'YES') { throw 'aborted: nothing was changed' }

# --- format ------------------------------------------------------------------
Clear-Disk -Number $DiskNumber -RemoveData -Confirm:$false
Initialize-Disk -Number $DiskNumber -PartitionStyle MBR -ErrorAction SilentlyContinue
$part = New-Partition -DiskNumber $DiskNumber -Size 2GB -AssignDriveLetter
Start-Sleep -Seconds 2
$vol = $part | Format-Volume -FileSystem FAT32 -NewFileSystemLabel 'TOS64BOOT' -Confirm:$false
$drive = "$($vol.DriveLetter):"

# --- stage and verify ---------------------------------------------------------
Copy-Item $kernel (Join-Path $drive 'kernel8.img')
Copy-Item $config (Join-Path $drive 'config.txt')
$copiedHash = (Get-FileHash -Algorithm SHA256 (Join-Path $drive 'kernel8.img')).Hash.ToLower()
if ($copiedHash -ne $sourceHash) {
    Remove-Item (Join-Path $drive 'kernel8.img')
    throw "copy verification FAILED (card $copiedHash != build $sourceHash); bad copy deleted - re-run"
}
$cfgLines = Get-Content (Join-Path $drive 'config.txt')
if (($cfgLines -notcontains 'os_check=0') -or ($cfgLines -notcontains 'kernel=kernel8.img')) {
    throw 'config.txt on the card is missing a required line - re-run'
}

Write-Host ''
Write-Host "DONE: $drive is TOS64BOOT (FAT32), kernel8.img verified sha256 $copiedHash"
Write-Host 'Safely eject the card, insert it into the Pi 5, and return to the runbook.'
