// tos64-sdprep — prepares an SD card for the TinyOS Pi 5 board session.
//
// The C# counterpart of docs/pi5-prepare-sd.ps1, for double-click use:
// the embedded app.manifest requests elevation (a disk wipe is an
// administrative action by OS design, in every language), the safety rails
// and the copy/hash verification are pure C#, and the one thing .NET has no
// managed API for — repartitioning — is driven through an audited diskpart
// script whose exact content is printed before it runs.
//
// Rails, in order: never disk 0 · USB/removable bus only · refuse a card
// carrying more than ~1 GiB of data · typed YES after a full summary.
// After formatting: kernel8.img and config.txt are copied and the copy's
// SHA-256 is verified against the build output; a mismatch deletes the bad
// copy rather than leaving a plausible-looking card, because the run record
// binds capture to image hash.

using System.Diagnostics;
using System.Management;
using System.Security.Cryptography;

// `LE-90`, first statements and nowhere else. Redirected stdout in .NET is
// buffered at 4 KiB and flushed on exit, so a tool that runs for minutes and is
// tailed from another window shows its banner and then nothing while the work
// it is narrating happens in silence. Found in tos64-netboot on 2026-08-06,
// where it hid an entire netboot server's log and let one stale instance
// masquerade as a healthy one (`LE-87`'s own cause, one level down).
//
// Applied here even where the tool exits quickly enough not to be bitten
// today. Whether a program runs long enough for this to matter is a property
// of its loops, and loops get added; a fix conditional on that is a fact
// recorded beside the thing that determines it rather than derived from it,
// which is the `LE-89`/`LE-91` family and the reason this is uniform.
Console.SetOut(new StreamWriter(Console.OpenStandardOutput()) { AutoFlush = true });
Console.SetError(new StreamWriter(Console.OpenStandardError()) { AutoFlush = true });


const long DataGuardBytes = 1L << 30; // ~1 GiB

int Fail(string message)
{
    Console.WriteLine();
    Console.WriteLine($"REFUSED: {message}");
    Pause();
    return 1;
}

void Pause()
{
    Console.WriteLine();
    Console.Write("Press any key to close...");
    try { Console.ReadKey(true); } catch (InvalidOperationException) { /* no console */ }
}

// Lists what a volume actually holds: every top-level entry (up to a display
// cap), then a bounded recursive tally, so approval is of visible contents
// rather than a matching label. Windows metadata folders are named, not hidden.
static List<string> DescribeContents(string root)
{
    var lines = new List<string>();
    const int TopLevelCap = 15;
    const int TallyCap = 5000;
    try
    {
        var entries = Directory.EnumerateFileSystemEntries(root).Take(TopLevelCap + 1).ToList();
        if (entries.Count == 0)
        {
            lines.Add("      contents: (empty)");
            return lines;
        }
        lines.Add("      contents (top level):");
        foreach (var entry in entries.Take(TopLevelCap))
        {
            bool isDir = Directory.Exists(entry);
            string name = Path.GetFileName(entry);
            if (isDir)
            {
                lines.Add($"        <DIR>  {name}");
            }
            else
            {
                long len = 0;
                try { len = new FileInfo(entry).Length; } catch (IOException) { }
                lines.Add($"        {len,12:N0}  {name}");
            }
        }
        if (entries.Count > TopLevelCap)
        {
            lines.Add("        ... (more top-level entries not shown)");
        }

        long fileCount = 0, byteCount = 0;
        bool capped = false;
        var pending = new Stack<string>();
        pending.Push(root);
        while (pending.Count > 0 && !capped)
        {
            string dir = pending.Pop();
            IEnumerable<string> children;
            try { children = Directory.EnumerateFileSystemEntries(dir); }
            catch (UnauthorizedAccessException) { continue; }
            catch (IOException) { continue; }
            foreach (var child in children)
            {
                if (Directory.Exists(child))
                {
                    pending.Push(child);
                }
                else
                {
                    fileCount++;
                    try { byteCount += new FileInfo(child).Length; } catch (IOException) { }
                    if (fileCount >= TallyCap) { capped = true; break; }
                }
            }
        }
        lines.Add($"      total: {(capped ? $"{TallyCap:N0}+ (count capped)" : fileCount.ToString("N0"))} file(s), " +
                  $"~{byteCount / (1024.0 * 1024):F1} MiB{(capped ? "+" : "")}");
    }
    catch (UnauthorizedAccessException)
    {
        lines.Add("      contents: (not readable — access denied)");
    }
    catch (IOException error)
    {
        lines.Add($"      contents: (not readable — {error.Message})");
    }
    return lines;
}

// --- locate the repository and the staged files ----------------------------

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
string kernelSrc = Path.Combine(stageDir, "kernel8.img");
string configSrc = Path.Combine(stageDir, "config.txt");
if (!File.Exists(kernelSrc) || !File.Exists(configSrc))
{
    return Fail($"missing staged files in {stageDir} — build first: cd os; cargo run -p xtask -- pi5 --fixture=boot");
}

string sourceHash = Convert.ToHexStringLower(SHA256.HashData(File.ReadAllBytes(kernelSrc)));

// --- which physical disks host the running OS? Those are untouchable -------
// Resolved by association (C: -> partition -> physical disk), never assumed
// to be disk 0: on some machines the OS lives elsewhere.

var systemDisks = new HashSet<uint>();
string systemLetter = Path.GetPathRoot(Environment.GetFolderPath(Environment.SpecialFolder.Windows))!
    .TrimEnd('\\');
using (var search = new ManagementObjectSearcher(
    $"ASSOCIATORS OF {{Win32_LogicalDisk.DeviceID='{systemLetter}'}} WHERE AssocClass=Win32_LogicalDiskToPartition"))
{
    foreach (var partition in search.Get())
    {
        using var drives = new ManagementObjectSearcher(
            $"ASSOCIATORS OF {{Win32_DiskPartition.DeviceID='{partition["DeviceID"]}'}} WHERE AssocClass=Win32_DiskDriveToDiskPartition");
        foreach (var drive in drives.Get())
        {
            systemDisks.Add((uint)drive["Index"]);
        }
    }
}

// --- find the SD card: exactly one removable USB/SD disk --------------------

var candidates = new List<(uint Index, string Model, ulong Size, string Bus)>();
using (var search = new ManagementObjectSearcher(
    "SELECT Index, Model, Size, InterfaceType, MediaType FROM Win32_DiskDrive"))
{
    foreach (var disk in search.Get())
    {
        uint index = (uint)disk["Index"];
        string bus = disk["InterfaceType"]?.ToString() ?? "";
        string media = disk["MediaType"]?.ToString() ?? "";
        bool removable = bus == "USB"
            || media.Contains("Removable", StringComparison.OrdinalIgnoreCase)
            || media.Contains("External", StringComparison.OrdinalIgnoreCase);
        if (index != 0 && !systemDisks.Contains(index) && removable)
        {
            string busShown = string.IsNullOrWhiteSpace(bus) ? media : bus;
            candidates.Add((index, disk["Model"]?.ToString() ?? "?", (ulong)(disk["Size"] ?? 0UL), busShown));
        }
    }
}

if (candidates.Count == 0)
{
    return Fail("no removable USB/SD disk found (the system disk and fixed internal disks are never eligible) — is the card inserted?");
}

// --- positive identification: the operator names the drive letter -----------
// Never erase on inference alone: even a single candidate could be the wrong
// empty USB stick if the card is not actually inserted. The letter typed here
// is resolved letter -> partition -> physical disk and must land on a
// candidate; every automatic rail still runs afterwards.

var letterToDisk = new Dictionary<string, uint>(StringComparer.OrdinalIgnoreCase);
Console.WriteLine();
Console.WriteLine("Removable disk(s) found (fixed internal disks and the system disk are excluded):");
foreach (var c in candidates)
{
    Console.WriteLine($"  disk {c.Index}: {c.Model}, {c.Size / (1024.0 * 1024 * 1024):F1} GiB, bus {c.Bus}");
    using var parts = new ManagementObjectSearcher(
        $"ASSOCIATORS OF {{Win32_DiskDrive.DeviceID='\\\\.\\PHYSICALDRIVE{c.Index}'}} WHERE AssocClass=Win32_DiskDriveToDiskPartition");
    foreach (var partition in parts.Get())
    {
        using var logical = new ManagementObjectSearcher(
            $"ASSOCIATORS OF {{Win32_DiskPartition.DeviceID='{partition["DeviceID"]}'}} WHERE AssocClass=Win32_LogicalDiskToPartition");
        foreach (var volume in logical.Get())
        {
            string letter = volume["DeviceID"]?.ToString() ?? "?";
            string volLabel = volume["VolumeName"]?.ToString() ?? "";
            string volFs = volume["FileSystem"]?.ToString() ?? "?";
            Console.WriteLine($"      {letter} '{volLabel}' ({volFs})");
            letterToDisk[letter.TrimEnd(':')] = c.Index;
        }
    }
}
if (letterToDisk.Count == 0)
{
    return Fail("the removable disk(s) mount no drive letter, so there is nothing you can positively identify — insert/re-insert the card so Windows assigns it a letter");
}
Console.WriteLine();
Console.Write("Type the DRIVE LETTER of your SD card exactly as you see it in Explorer (e.g. D): ");
string typedLetter = (Console.ReadLine() ?? "").Trim().TrimEnd(':');
if (!letterToDisk.TryGetValue(typedLetter, out uint chosenDisk))
{
    return Fail($"'{typedLetter}' is not a drive letter on any eligible removable disk; nothing was changed");
}

var (diskIndex, model, sizeBytes, busType) = candidates.First(c => c.Index == chosenDisk);

// --- walk the target's partitions and volumes: boot flags, OS markers, ------
// --- mounted letters/labels for display, and how much data it holds ---------

long usedBytes = 0;
var volumeLines = new List<string>();
using (var search = new ManagementObjectSearcher(
    $"ASSOCIATORS OF {{Win32_DiskDrive.DeviceID='\\\\.\\PHYSICALDRIVE{diskIndex}'}} WHERE AssocClass=Win32_DiskDriveToDiskPartition"))
{
    foreach (var partition in search.Get())
    {
        if (partition["BootPartition"] is bool bootFlag && bootFlag)
        {
            return Fail($"disk {diskIndex} has a boot-flagged partition ({partition["DeviceID"]}); " +
                        "this tool never touches a disk anything boots from");
        }
        using var logical = new ManagementObjectSearcher(
            $"ASSOCIATORS OF {{Win32_DiskPartition.DeviceID='{partition["DeviceID"]}'}} WHERE AssocClass=Win32_LogicalDiskToPartition");
        foreach (var volume in logical.Get())
        {
            string letter = volume["DeviceID"]?.ToString() ?? "?";
            string label = volume["VolumeName"]?.ToString() ?? "";
            string fs = volume["FileSystem"]?.ToString() ?? "?";
            ulong size = (ulong)(volume["Size"] ?? 0UL);
            ulong free = (ulong)(volume["FreeSpace"] ?? 0UL);
            usedBytes += (long)(size - free);
            volumeLines.Add(
                $"  {letter} '{label}' ({fs}), {size / (1024.0 * 1024 * 1024):F1} GiB, " +
                $"~{(size - free) / (1024.0 * 1024 * 1024):F1} GiB used");

            // Content manifest: what would actually be destroyed. A label is
            // a claim; the file listing is the fact the operator approves.
            volumeLines.AddRange(DescribeContents(letter + "\\"));

            // An operating system on any of its volumes disqualifies the
            // whole disk, regardless of every other signal.
            string root = letter + "\\";
            if (Directory.Exists(Path.Combine(root, "Windows"))
                || Directory.Exists(Path.Combine(root, "Users"))
                || File.Exists(Path.Combine(root, "pagefile.sys")))
            {
                return Fail($"disk {diskIndex} volume {letter} ('{label}') carries operating-system " +
                            "markers (\\Windows, \\Users or pagefile.sys); refusing outright");
            }
        }
    }
}
if (usedBytes > DataGuardBytes)
{
    return Fail($"disk {diskIndex} carries ~{usedBytes / (1024.0 * 1024 * 1024):F1} GiB of data; " +
                "this tool never overrides that guard — empty the card (or use the .ps1 with -Force) if you are certain");
}

// --- informed consent -------------------------------------------------------

string diskpartScript = string.Join(Environment.NewLine,
    $"select disk {diskIndex}",
    "clean",
    "convert mbr",
    "create partition primary size=2048",
    "format fs=fat32 label=TOS64BOOT quick",
    "assign",
    "exit");

Console.WriteLine();
Console.WriteLine($"About to ERASE disk {diskIndex}: {model}, {sizeBytes / (1024.0 * 1024 * 1024):F1} GiB, bus {busType}");
Console.WriteLine(volumeLines.Count > 0
    ? "Its currently mounted volume(s) — check the letter and label are your SD card:"
    : "It carries no mounted volumes.");
foreach (var line in volumeLines)
{
    Console.WriteLine(line);
}
Console.WriteLine($"Checks passed: not the system disk (that is disk {string.Join("+", systemDisks)},");
Console.WriteLine("hosting " + systemLetter + "\\Windows), no boot-flagged partition, no OS markers on");
Console.WriteLine($"any volume, removable bus, under 1 GiB of data (~{usedBytes / (1024.0 * 1024):F0} MiB).");
Console.WriteLine();
Console.WriteLine("Result: MBR + one 2 GiB FAT32 partition TOS64BOOT carrying:");
Console.WriteLine($"  kernel8.img  sha256 {sourceHash}");
// The build owns config.txt's contents (xtask `pi5::CONFIG_TXT`, pinned by
// test), so this echoes what is actually being copied rather than a subset
// remembered when this line was written -- it listed two of the four after
// `pciex4_reset=0` and `hdmi_force_hotplug=1` were added, which is the
// lying-by-omission shape LE-97 is about.
Console.WriteLine($"  config.txt   ({string.Join(", ", File.ReadAllLines(configSrc).Where(l => l.Length > 0))})");
Console.WriteLine();
Console.WriteLine("diskpart will run exactly this script:");
foreach (var line in diskpartScript.Split(Environment.NewLine))
{
    Console.WriteLine($"    {line}");
}
Console.WriteLine();
Console.Write("Type YES (uppercase) to proceed: ");
if (Console.ReadLine() != "YES")
{
    return Fail("aborted: nothing was changed");
}

// --- format via diskpart -----------------------------------------------------

string scriptPath = Path.Combine(Path.GetTempPath(), $"tos64-sdprep-{Environment.ProcessId}.txt");
File.WriteAllText(scriptPath, diskpartScript);
try
{
    var diskpart = Process.Start(new ProcessStartInfo
    {
        FileName = "diskpart.exe",
        Arguments = $"/s \"{scriptPath}\"",
        UseShellExecute = false,
        RedirectStandardOutput = true,
    })!;
    string output = diskpart.StandardOutput.ReadToEnd();
    diskpart.WaitForExit();
    if (diskpart.ExitCode != 0)
    {
        Console.WriteLine(output);
        return Fail($"diskpart exited {diskpart.ExitCode}; the card may be partially formatted — re-run");
    }
}
finally
{
    File.Delete(scriptPath);
}

// --- find the new volume -----------------------------------------------------

DriveInfo? card = null;
for (int attempt = 0; attempt < 20 && card is null; attempt++)
{
    Thread.Sleep(500);
    card = DriveInfo.GetDrives().FirstOrDefault(d =>
    {
        try { return d.IsReady && d.VolumeLabel == "TOS64BOOT"; }
        catch (IOException) { return false; }
        catch (UnauthorizedAccessException) { return false; }
    });
}
if (card is null)
{
    return Fail("formatted, but the TOS64BOOT volume never appeared — re-insert the card and re-run");
}

// --- stage and verify ---------------------------------------------------------

string kernelDst = Path.Combine(card.RootDirectory.FullName, "kernel8.img");
string configDst = Path.Combine(card.RootDirectory.FullName, "config.txt");
File.Copy(kernelSrc, kernelDst, overwrite: true);
File.Copy(configSrc, configDst, overwrite: true);

string copiedHash = Convert.ToHexStringLower(SHA256.HashData(File.ReadAllBytes(kernelDst)));
if (copiedHash != sourceHash)
{
    File.Delete(kernelDst);
    return Fail($"copy verification FAILED (card {copiedHash} != build {sourceHash}); bad copy deleted — re-run");
}
string[] cfg = File.ReadAllLines(configDst);
if (!cfg.Contains("os_check=0") || !cfg.Contains("kernel=kernel8.img"))
{
    return Fail("config.txt on the card is missing a required line — re-run");
}

Console.WriteLine();
Console.WriteLine($"DONE: {card.Name} is TOS64BOOT (FAT32), kernel8.img verified sha256 {copiedHash}");
Console.WriteLine("Safely eject the card, insert it into the Pi 5, and return to the runbook.");
Pause();
return 0;
