// tos64-cardswap — switches the one physical SD card between its two roles:
// TOS64 experiment card and Pi OS ground-truth instrument (hand-2026-08-03
// 07A promoted the latter; the owner has exactly one card, so the roles
// share it). C# like every host tool in this folder (sdprep is the pattern).
//
// The trick that makes this safe: on a Pi 5 the boot firmware lives in
// EEPROM, so the FAT32 bootfs only decides *which kernel* runs. Swapping
// config.txt + kernel8.img swaps the OS; nothing else on the card is
// touched, and the Pi OS root filesystem is never mounted on Windows at
// all. The Pi OS originals are backed up on the card itself, so either
// direction is a plain restore.
//
//   tos64-cardswap tos64   back up Pi OS config.txt + kernel8.img into
//                          \pios-backup\ (only if no backup exists yet —
//                          a backup is never overwritten, so the originals
//                          can't be clobbered by running this twice), then
//                          copy the staged build from os\target\pi5\ and
//                          verify the on-card SHA-256 against the build.
//   tos64-cardswap pios    restore \pios-backup\ over the TOS64 files and
//                          verify the restore by hash.
//   tos64-cardswap status  say which role the card currently carries.
//
// No elevation, no diskpart, no destruction: every operation is a file
// copy on the bootfs volume with a hash check after it.

using System.Security.Cryptography;

const string BackupDirName = "pios-backup";
string[] swapFiles = ["config.txt", "kernel8.img"];

int Fail(string message)
{
    Console.WriteLine($"REFUSED: {message}");
    return 1;
}

static string HashOf(string path) =>
    Convert.ToHexStringLower(SHA256.HashData(File.ReadAllBytes(path)));

// --- locate the repository, the staged build, and the card -----------------

string? repoRoot = AppContext.BaseDirectory;
while (repoRoot is not null && !File.Exists(Path.Combine(repoRoot, "agent.md")))
{
    repoRoot = Path.GetDirectoryName(Path.TrimEndingDirectorySeparator(repoRoot));
}
if (repoRoot is null)
{
    return Fail("could not locate the TinyOS repository root (no agent.md above this exe)");
}
string stageDir = Path.Combine(repoRoot, "os", "target", "pi5");

// The card is whichever mounted volume is the Pi's boot partition: labeled
// bootfs (Pi OS imaging) or TOS64BOOT (sdprep). Anything else is not ours.
var cards = DriveInfo.GetDrives().Where(d =>
{
    try { return d.IsReady && d.VolumeLabel is "bootfs" or "TOS64BOOT"; }
    catch (IOException) { return false; }
    catch (UnauthorizedAccessException) { return false; }
}).ToList();
if (cards.Count != 1)
{
    return Fail(cards.Count == 0
        ? "no volume labeled 'bootfs' or 'TOS64BOOT' is mounted — is the card inserted?"
        : $"more than one candidate volume ({string.Join(", ", cards.Select(c => c.Name))}) — eject the one that is not the Pi card");
}
string root = cards[0].RootDirectory.FullName;
string backupDir = Path.Combine(root, BackupDirName);

// Sanity: a Pi boot partition carries the firmware's config. A volume that
// merely shares the label is refused before anything is written.
if (!File.Exists(Path.Combine(root, "config.txt")))
{
    return Fail($"{root} has no config.txt — not a Pi boot partition");
}

bool backupExists = swapFiles.All(f => File.Exists(Path.Combine(backupDir, f)));
string onCardKernelHash = File.Exists(Path.Combine(root, "kernel8.img"))
    ? HashOf(Path.Combine(root, "kernel8.img"))
    : "(no kernel8.img)";
string stagedKernelHash = File.Exists(Path.Combine(stageDir, "kernel8.img"))
    ? HashOf(Path.Combine(stageDir, "kernel8.img"))
    : "(not built)";

string mode = args.FirstOrDefault() ?? "status";
switch (mode)
{
    case "status":
        Console.WriteLine($"card:            {root} ('{cards[0].VolumeLabel}')");
        Console.WriteLine($"on-card kernel:  {onCardKernelHash}");
        Console.WriteLine($"staged build:    {stagedKernelHash}");
        Console.WriteLine($"pios backup:     {(backupExists ? "present" : "none")}");
        Console.WriteLine($"role:            {(onCardKernelHash == stagedKernelHash ? "TOS64 (matches staged build)" : backupExists ? "unknown kernel, backup present" : "Pi OS (untouched)")}");
        return 0;

    case "tos64":
    {
        foreach (var f in swapFiles)
        {
            if (!File.Exists(Path.Combine(stageDir, f)))
            {
                return Fail($"missing {f} in {stageDir} — build first: cd os; cargo run -p xtask -- pi5 --fixture=boot");
            }
        }
        if (!backupExists)
        {
            Directory.CreateDirectory(backupDir);
            foreach (var f in swapFiles)
            {
                string src = Path.Combine(root, f);
                string dst = Path.Combine(backupDir, f);
                File.Copy(src, dst);
                if (HashOf(dst) != HashOf(src))
                {
                    File.Delete(dst);
                    return Fail($"backup of {f} failed hash verification; bad copy deleted, card unchanged — re-run");
                }
                Console.WriteLine($"backed up {f} -> {BackupDirName}\\{f}");
            }
        }
        else
        {
            Console.WriteLine($"backup already present in {BackupDirName}\\ — kept as-is");
        }

        foreach (var f in swapFiles)
        {
            File.Copy(Path.Combine(stageDir, f), Path.Combine(root, f), overwrite: true);
        }
        string copied = HashOf(Path.Combine(root, "kernel8.img"));
        if (copied != stagedKernelHash)
        {
            return Fail($"on-card kernel {copied} != staged build {stagedKernelHash} after copy — re-run; Pi OS backup is intact");
        }
        Console.WriteLine($"DONE: card is TOS64 — kernel8.img verified sha256 {copied}");
        Console.WriteLine("Safely eject, insert into the Pi 5, monitor on before power.");
        return 0;
    }

    case "pios":
    {
        if (!backupExists)
        {
            return Fail($"no complete backup in {backupDir} — nothing to restore");
        }
        foreach (var f in swapFiles)
        {
            string src = Path.Combine(backupDir, f);
            string dst = Path.Combine(root, f);
            File.Copy(src, dst, overwrite: true);
            if (HashOf(dst) != HashOf(src))
            {
                return Fail($"restore of {f} failed hash verification — re-run; the backup itself is untouched");
            }
            Console.WriteLine($"restored {f}");
        }
        Console.WriteLine("DONE: card is Pi OS again (backup retained for the next swap)");
        return 0;
    }

    default:
        return Fail($"unknown mode '{mode}' — use: tos64 | pios | status");
}
