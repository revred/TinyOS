// tos64-serialwatch — the armed watch (06A next-action 1, STORY-P1-09-05
// criterion 4), C# like every host tool in this folder (sdprep is the
// pattern).
//
// Waits for a serial adapter to (re)enumerate, then sweeps twelve candidate
// baud rates listening for the board's once-per-second TOS64-BEAT/1
// heartbeat. The heartbeat made the listen untimed; this tool makes it
// unattended: plug the adapter in whenever, the sweep starts by itself,
// everything is logged and every byte captured.
//
// Sweep design: the park loop heartbeats every second forever, so a ~6 s
// dwell per baud sees ~6 lines if the clock transcription is right at that
// rate. A wrong UART clock is a *rational* factor off, so the candidates are
// 115200 scaled by the plausible clock ratios plus the classic strap rates.
// Bytes that arrive but never decode as TOS64 are themselves the finding 05A
// named: bytes at a findable rate instead of silence.
//
// Run:  dotnet run  — or the published exe. Optional arg: --log-dir=<path>
//       (default: logs\ beside the executable). Stop: the pid is in
//       <log-dir>\watch.pid.

using System.IO.Ports;
using System.Text;

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


var logDir = args
    .Select(a => a.StartsWith("--log-dir=") ? a["--log-dir=".Length..] : null)
    .FirstOrDefault(v => v != null)
    ?? Path.Combine(AppContext.BaseDirectory, "logs");
Directory.CreateDirectory(logDir);
var logPath = Path.Combine(logDir, "watch.log");
File.WriteAllText(Path.Combine(logDir, "watch.pid"), Environment.ProcessId.ToString());

// Twelve candidates, most-likely first. 115200 is the pinned rate
// (docs/pi5-board-session-runbook.md); the rest cover 2x/4x/8x clock ratios
// both directions, the 250k/500k/1M crystal-transcription family, and the
// legacy strap rates.
int[] bauds = [115200, 230400, 57600, 460800, 921600, 38400, 76800, 250000, 500000, 1000000, 19200, 9600];
const int DwellSeconds = 6;

void Log(string message)
{
    var line = $"{DateTime.Now:yyyy-MM-dd HH:mm:ss}  {message}";
    File.AppendAllText(logPath, line + Environment.NewLine);
    Console.WriteLine(line);
}

// Reads the port for one dwell window; returns every byte heard. A timeout
// is a quiet moment, not a failure; anything else ends the dwell.
static byte[] Dwell(SerialPort port, int seconds, Action<string> log)
{
    var heard = new List<byte>();
    var chunk = new byte[512];
    var deadline = DateTime.UtcNow.AddSeconds(seconds);
    while (DateTime.UtcNow < deadline)
    {
        try
        {
            int read = port.BaseStream.Read(chunk, 0, chunk.Length);
            if (read > 0) heard.AddRange(chunk[..read]);
        }
        catch (TimeoutException) { /* a quiet half-second at this baud */ }
        catch (Exception e)
        {
            log($"read error at {port.BaudRate} baud: {e.Message}");
            break;
        }
    }
    return [.. heard];
}

static string Printable(byte[] bytes) =>
    new(bytes.Select(b => b is >= 32 and <= 126 ? (char)b : b == (byte)'\n' ? '\n' : '.').ToArray());

// Sweeps every candidate baud in a loop until the adapter disappears or a
// TOS64 line decodes; on decode, stays at that baud capturing forever —
// the first contact also carries the LINK verdict and the fb= field, so
// nothing after it may be dropped.
void SweepPort(string portName)
{
    Log($"sweep starting on {portName} — twelve bauds, {DwellSeconds}s dwell each");
    while (true)
    {
        foreach (var baud in bauds)
        {
            using var port = new SerialPort(portName, baud, Parity.None, 8, StopBits.One);
            port.ReadTimeout = 500;
            try { port.Open(); }
            catch (Exception e)
            {
                Log($"cannot open {portName} at {baud} ({e.Message}) — adapter gone? Back to the arrival watch.");
                return;
            }

            var heard = Dwell(port, DwellSeconds, Log);
            if (heard.Length == 0) continue;

            var stamp = DateTime.Now.ToString("yyyyMMdd-HHmmss");
            var capturePath = Path.Combine(logDir, $"capture-{baud}-{stamp}.bin");
            File.WriteAllBytes(capturePath, heard);
            Log($"{heard.Length} bytes at {baud} baud -> {capturePath}");

            var ascii = Printable(heard);
            if (!ascii.Contains("TOS64-"))
            {
                Log($"bytes at {baud} did not decode as TOS64 — a wrong-clock candidate; sweep continues");
                continue;
            }

            Log($"DECODED at {baud} baud — TOS64 protocol lines present. Staying on this baud.");
            foreach (var line in ascii.Split('\n').Where(l => l.Contains("TOS64-")).Take(8))
                Log($"  > {line}");

            var livePath = Path.Combine(logDir, $"capture-{baud}-{stamp}-live.bin");
            var chunk = new byte[512];
            while (true)
            {
                try
                {
                    int read = port.BaseStream.Read(chunk, 0, chunk.Length);
                    if (read > 0)
                    {
                        using var live = File.Open(livePath, FileMode.Append);
                        live.Write(chunk, 0, read);
                    }
                }
                catch (TimeoutException) { }
                catch (Exception e)
                {
                    Log($"live capture ended ({e.Message})");
                    return;
                }
            }
        }
        Log($"full sweep silent on {portName} — looping (board powered? adapter loopback never run — TEST-P1-07-01-A clause 1)");
    }
}

Log($"watch armed (pid {Environment.ProcessId}) — waiting for a serial port to enumerate");
var baseline = SerialPort.GetPortNames();
if (baseline.Length > 0)
    Log($"ports already present at arm time (ignored until re-enumeration): {string.Join(", ", baseline)}");
// COM5 is the port every prior capture attempt used; if it is already
// present, sweep it immediately rather than demanding a replug.
if (baseline.Contains("COM5")) SweepPort("COM5");
while (true)
{
    Thread.Sleep(2000);
    var now = SerialPort.GetPortNames();
    foreach (var fresh in now.Except(baseline))
    {
        Log($"port enumerated: {fresh}");
        SweepPort(fresh);
    }
    baseline = now;
}
