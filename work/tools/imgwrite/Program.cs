// tos64-imgwrite — writes a raw OS image (.img or .img.xz) to a removable
// disk. C# like every host tool here (sdprep is the pattern); the embedded
// app.manifest requests elevation because a raw disk write is an
// administrative action by OS design.
//
// Rails, in order (sdprep's discipline): never disk 0 · the target disk
// number is explicit on the command line · the disk must be present and
// smaller than 512 GiB (no accidental data drives) · full summary then a
// typed YES before the first destructive byte. Volumes on the target are
// locked and dismounted before writing so the filesystem stack cannot race
// the raw writes.
//
// Usage: tos64-imgwrite <image.img[.xz]> <physical-disk-number>

using System.Runtime.InteropServices;
using Microsoft.Win32.SafeHandles;
using SharpCompress.Compressors.Xz;

const long MaxTargetBytes = 512L * 1024 * 1024 * 1024;
const int SectorAlign = 512;
const int ChunkBytes = 4 * 1024 * 1024;

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
    try { Console.ReadKey(true); } catch (InvalidOperationException) { }
}

if (args.Length != 2) return Fail("usage: tos64-imgwrite <image.img[.xz]> <physical-disk-number>");
var imagePath = args[0];
if (!File.Exists(imagePath)) return Fail($"image not found: {imagePath}");
if (!int.TryParse(args[1], out var diskNumber)) return Fail($"not a disk number: {args[1]}");
if (diskNumber == 0) return Fail("disk 0 is never a target, in any language");

var drivePath = $@"\\.\PhysicalDrive{diskNumber}";
using var disk = Native.OpenForWrite(drivePath);
if (disk.IsInvalid) return Fail($"cannot open {drivePath} ({Marshal.GetLastPInvokeError()}) — is the card present? elevated?");

long diskBytes = Native.DiskLength(disk);
if (diskBytes <= 0) return Fail($"cannot size {drivePath}");
if (diskBytes > MaxTargetBytes) return Fail($"{drivePath} is {diskBytes / (1L << 30)} GiB — too large to be the SD card; refusing");

// Every mounted volume on the target must be locked and dismounted, or the
// filesystem stack silently overwrites our sectors with cached metadata.
var letters = Native.VolumeLettersOnDisk(diskNumber);
var lockedVolumes = new List<SafeFileHandle>();
foreach (var letter in letters)
{
    var vol = Native.OpenForWrite($@"\\.\{letter}:");
    if (vol.IsInvalid) return Fail($"cannot open volume {letter}: for locking");
    if (!Native.Lock(vol)) return Fail($"cannot lock volume {letter}: — is something using the card?");
    if (!Native.Dismount(vol)) return Fail($"cannot dismount volume {letter}:");
    lockedVolumes.Add(vol);
}

long imageBytes = new FileInfo(imagePath).Length;
bool xz = imagePath.EndsWith(".xz", StringComparison.OrdinalIgnoreCase);
Console.WriteLine("tos64-imgwrite — this will DESTROY everything on the target disk.");
Console.WriteLine($"  image : {imagePath} ({imageBytes:N0} bytes{(xz ? ", xz-compressed" : "")})");
Console.WriteLine($"  target: {drivePath} ({diskBytes:N0} bytes){(letters.Count > 0 ? $", volumes {string.Join(", ", letters.Select(l => l + ":"))} (locked)" : "")}");
Console.WriteLine();
Console.Write("Type YES to write: ");
if (Console.ReadLine()?.Trim() != "YES") return Fail("not confirmed");

using Stream file = File.OpenRead(imagePath);
using Stream source = xz ? new XZStream(file) : file;

var buffer = new byte[ChunkBytes];
long written = 0;
var started = DateTime.UtcNow;
while (true)
{
    int filled = 0;
    while (filled < buffer.Length)
    {
        int read = source.Read(buffer, filled, buffer.Length - filled);
        if (read == 0) break;
        filled += read;
    }
    if (filled == 0) break;
    // Raw device writes must be sector-aligned; the final fragment is padded
    // with zeros, which is dead space past the image's last partition.
    int aligned = (filled + SectorAlign - 1) / SectorAlign * SectorAlign;
    Array.Clear(buffer, filled, aligned - filled);
    if (written + aligned > diskBytes) return Fail($"image larger than the disk at offset {written:N0}");
    if (!Native.Write(disk, buffer, aligned)) return Fail($"write failed at offset {written:N0} ({Marshal.GetLastPInvokeError()})");
    written += aligned;
    if (written % (256L * 1024 * 1024) < ChunkBytes)
        Console.WriteLine($"  {written / (1 << 20):N0} MiB written ({(int)(DateTime.UtcNow - started).TotalSeconds}s)");
}
Native.Flush(disk);
foreach (var vol in lockedVolumes) vol.Dispose();

Console.WriteLine();
Console.WriteLine($"DONE: {written:N0} bytes written in {(int)(DateTime.UtcNow - started).TotalSeconds}s. Eject the card and boot the board.");
Pause();
return 0;

static class Native
{
    [DllImport("kernel32.dll", SetLastError = true, CharSet = CharSet.Unicode)]
    static extern SafeFileHandle CreateFile(string name, uint access, uint share, IntPtr security, uint disposition, uint flags, IntPtr template);

    [DllImport("kernel32.dll", SetLastError = true)]
    static extern bool DeviceIoControl(SafeFileHandle device, uint code, IntPtr inBuf, int inLen, out long outBuf, int outLen, out int returned, IntPtr overlapped);

    [DllImport("kernel32.dll", SetLastError = true)]
    static extern bool DeviceIoControl(SafeFileHandle device, uint code, IntPtr inBuf, int inLen, IntPtr outBuf, int outLen, out int returned, IntPtr overlapped);

    [DllImport("kernel32.dll", SetLastError = true)]
    static extern bool WriteFile(SafeFileHandle handle, byte[] buffer, int bytes, out int written, IntPtr overlapped);

    [DllImport("kernel32.dll", SetLastError = true)]
    static extern bool FlushFileBuffers(SafeFileHandle handle);

    const uint GenericRead = 0x80000000, GenericWrite = 0x40000000;
    const uint ShareReadWrite = 0x3, OpenExisting = 3;
    const uint IoctlDiskGetLengthInfo = 0x0007405C;
    const uint FsctlLockVolume = 0x00090018, FsctlDismountVolume = 0x00090020;

    public static SafeFileHandle OpenForWrite(string path) =>
        CreateFile(path, GenericRead | GenericWrite, ShareReadWrite, IntPtr.Zero, OpenExisting, 0, IntPtr.Zero);

    public static long DiskLength(SafeFileHandle disk) =>
        DeviceIoControl(disk, IoctlDiskGetLengthInfo, IntPtr.Zero, 0, out long length, sizeof(long), out _, IntPtr.Zero) ? length : -1;

    public static bool Lock(SafeFileHandle volume) =>
        DeviceIoControl(volume, FsctlLockVolume, IntPtr.Zero, 0, IntPtr.Zero, 0, out _, IntPtr.Zero);

    public static bool Dismount(SafeFileHandle volume) =>
        DeviceIoControl(volume, FsctlDismountVolume, IntPtr.Zero, 0, IntPtr.Zero, 0, out _, IntPtr.Zero);

    public static bool Write(SafeFileHandle disk, byte[] buffer, int count) =>
        WriteFile(disk, buffer, count, out int written, IntPtr.Zero) && written == count;

    public static bool Flush(SafeFileHandle disk) => FlushFileBuffers(disk);

    // Maps drive letters to the physical disk they sit on, via the volume
    // extents ioctl — no WMI, no guessing by label.
    public static List<char> VolumeLettersOnDisk(int diskNumber)
    {
        var letters = new List<char>();
        foreach (var drive in DriveInfo.GetDrives())
        {
            if (drive.DriveType != DriveType.Removable && drive.DriveType != DriveType.Fixed) continue;
            char letter = drive.Name[0];
            using var vol = CreateFile($@"\\.\{letter}:", GenericRead, ShareReadWrite, IntPtr.Zero, OpenExisting, 0, IntPtr.Zero);
            if (vol.IsInvalid) continue;
            var buf = Marshal.AllocHGlobal(1024);
            try
            {
                const uint IoctlVolumeGetExtents = 0x00560000;
                if (DeviceIoControl(vol, IoctlVolumeGetExtents, IntPtr.Zero, 0, buf, 1024, out _, IntPtr.Zero))
                {
                    int extentCount = Marshal.ReadInt32(buf);
                    for (int i = 0; i < extentCount; i++)
                        if (Marshal.ReadInt32(buf, 8 + i * 24) == diskNumber) { letters.Add(letter); break; }
                }
            }
            finally { Marshal.FreeHGlobal(buf); }
        }
        return letters;
    }
}
