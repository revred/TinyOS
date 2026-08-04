// tos64-linkwatch — the second instrument for the TOS64 boot (hand-2026-08-03
// decision table, LE-68), C# like every host tool in this folder (sdprep is
// the pattern, tos64-serialwatch is the sibling).
//
// Watches every wired Ethernet interface on the laptop and logs each
// operational-status or link-speed transition with a timestamp. The question
// it answers: does the peer-to-peer link train while the TOS64 card boots?
// Up + a speed during our boot window = the PHY release sequence works and
// LE-68 closes. Flat = our release sequence is wrong. Either way the answer
// is in the log, not in whether anyone happened to be looking at
// Get-NetAdapter at the right second.
//
// Run:  dotnet run  — or the published exe. Optional arg: --log-dir=<path>
//       (default: logs\ beside the executable). Stop: the pid is in
//       <log-dir>\watch.pid.

using System.Net.NetworkInformation;

var logDir = args
    .Select(a => a.StartsWith("--log-dir=") ? a["--log-dir=".Length..] : null)
    .FirstOrDefault(v => v != null)
    ?? Path.Combine(AppContext.BaseDirectory, "logs");
Directory.CreateDirectory(logDir);
var logPath = Path.Combine(logDir, "watch.log");
File.WriteAllText(Path.Combine(logDir, "watch.pid"), Environment.ProcessId.ToString());

void Log(string message)
{
    var line = $"{DateTime.Now:yyyy-MM-dd HH:mm:ss.fff}  {message}";
    File.AppendAllText(logPath, line + Environment.NewLine);
    Console.WriteLine(line);
}

// Wired Ethernet only, and only real NICs: Windows reports every WFP filter
// and WAN miniport as an Ethernet interface, and that noise would bury the
// one transition that matters.
static bool IsPseudo(NetworkInterface n) =>
    n.Description.Contains("Filter")
    || n.Description.Contains("WAN Miniport")
    || n.Description.Contains("Bluetooth")
    || n.Description.Contains("Kernel Debug")
    || n.Description.Contains("QoS Packet Scheduler");

static Dictionary<string, (OperationalStatus Status, long Speed)> Snapshot() =>
    NetworkInterface.GetAllNetworkInterfaces()
        .Where(n => (n.NetworkInterfaceType is NetworkInterfaceType.Ethernet
                                            or NetworkInterfaceType.GigabitEthernet)
                    && !IsPseudo(n))
        .ToDictionary(n => $"{n.Name} ({n.Description})",
                      n => (n.OperationalStatus, n.Speed));

static string Describe((OperationalStatus Status, long Speed) s) =>
    s.Status == OperationalStatus.Up ? $"UP at {s.Speed / 1_000_000} Mbps" : s.Status.ToString();

var baseline = Snapshot();
Log($"watch armed (pid {Environment.ProcessId}) — polling wired Ethernet every 250 ms");
if (baseline.Count == 0)
    Log("no wired Ethernet interfaces present — is the USB NIC plugged in?");
foreach (var (name, state) in baseline)
    Log($"baseline: {name} — {Describe(state)}");

while (true)
{
    Thread.Sleep(250);
    var now = Snapshot();

    foreach (var (name, state) in now)
    {
        if (!baseline.TryGetValue(name, out var was))
            Log($"interface appeared: {name} — {Describe(state)}");
        else if (was != state)
            Log($"TRANSITION: {name} — {Describe(was)} -> {Describe(state)}");
    }
    foreach (var name in baseline.Keys.Except(now.Keys))
        Log($"interface vanished: {name}");

    baseline = now;
}
