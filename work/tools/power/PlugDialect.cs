using System.Text.Json;

/// The four LAN-controllable plug dialects, as pure functions.
///
/// `LE-95` fixes exactly one property for the device on the Pi 5's supply: it
/// must be controllable over the LAN with NO vendor cloud account. That is a
/// containment requirement rather than a convenience one — a bench whose board
/// cannot be rebooted while somebody else's service is down is a new instrument
/// failure, and this project has had five in a row. It is not a requirement for
/// one specific plug, so four dialects are supported and the bench buys
/// whichever is in stock:
///
///   Tasmota      GET /cm?cmnd=Power%20On            {"POWER":"ON"}
///   Shelly gen1  GET /relay/0?turn=on               {"ison":true}
///   Shelly gen2  GET /rpc/Switch.Set?id=0&on=true   {"was_on":false}   <-- see below
///   ESPHome      POST /switch/<id>/turn_on          {"state":"ON"}
///
/// Every one of them is HTTP on the local segment with no account, and every
/// one of them is a pure string here, because a dialect that can only be read
/// with a plug on the desk cannot be reviewed before it switches mains.
public enum PlugKind
{
    Tasmota,
    ShellyGen1,
    ShellyGen2,
    Esphome,
}

/// What is being asked of the plug. `Read` is separate from the two commands on
/// purpose: it is the only action whose answer is allowed to name a state.
public enum PlugAction
{
    On,
    Off,
    Read,
}

/// What the plug says it is. `Unknown` is a first-class answer and is never
/// collapsed into either of the others — that collapse is the defect this whole
/// tool is shaped around.
public enum PlugState
{
    Unknown,
    On,
    Off,
}

/// One HTTP request, decided entirely before anything is sent.
internal sealed record PlugRequest(HttpMethod Method, string Url);

/// The plug's address, validated once at the edge.
internal sealed record PlugBase(string Url, string? Entity)
{
    /// A base URL is accepted only if the operator spelled a scheme. A bare
    /// `10.0.0.9` would become a relative URI and a bare `plug.local` would
    /// resolve wherever the resolver felt like; both are ways for this tool to
    /// switch a socket other than the one that was typed. Safety before
    /// convenience, in that order, always.
    internal static PlugBase? Parse(string? url, string? entity, PlugKind kind = PlugKind.Tasmota)
    {
        if (string.IsNullOrWhiteSpace(url)) return null;
        if (!Uri.TryCreate(url.Trim(), UriKind.Absolute, out var parsed)) return null;
        if (parsed.Scheme != Uri.UriSchemeHttp && parsed.Scheme != Uri.UriSchemeHttps) return null;

        // ESPHome names its entity; every other dialect addresses relay 0
        // implicitly. Guessing a name here would switch whatever happened to be
        // called that, which on this seam is somebody else's appliance.
        if (kind == PlugKind.Esphome && string.IsNullOrWhiteSpace(entity)) return null;

        return new PlugBase(url.Trim().TrimEnd('/'), entity?.Trim());
    }
}

internal static class PlugDialect
{
    /// The request for one action in one dialect. Total: every enum pair has a
    /// spelling, so a dialect added tomorrow fails to compile rather than
    /// falling through to a plausible default.
    internal static PlugRequest Request(PlugKind kind, PlugBase plug, PlugAction action)
    {
        var at = plug.Url;
        return (kind, action) switch
        {
            (PlugKind.Tasmota, PlugAction.On) => Get($"{at}/cm?cmnd=Power%20On"),
            (PlugKind.Tasmota, PlugAction.Off) => Get($"{at}/cm?cmnd=Power%20Off"),
            (PlugKind.Tasmota, PlugAction.Read) => Get($"{at}/cm?cmnd=Power"),

            (PlugKind.ShellyGen1, PlugAction.On) => Get($"{at}/relay/0?turn=on"),
            (PlugKind.ShellyGen1, PlugAction.Off) => Get($"{at}/relay/0?turn=off"),
            (PlugKind.ShellyGen1, PlugAction.Read) => Get($"{at}/relay/0"),

            (PlugKind.ShellyGen2, PlugAction.On) => Get($"{at}/rpc/Switch.Set?id=0&on=true"),
            (PlugKind.ShellyGen2, PlugAction.Off) => Get($"{at}/rpc/Switch.Set?id=0&on=false"),
            (PlugKind.ShellyGen2, PlugAction.Read) => Get($"{at}/rpc/Switch.GetStatus?id=0"),

            // ESPHome's actions are POSTs and its readback is a GET. Sending
            // the wrong verb produces a 405, which reads exactly like a plug
            // that is not answering — an instrument failure in a device
            // failure's costume, the shape this bench keeps buying.
            (PlugKind.Esphome, PlugAction.On) => Post($"{at}/switch/{plug.Entity}/turn_on"),
            (PlugKind.Esphome, PlugAction.Off) => Post($"{at}/switch/{plug.Entity}/turn_off"),
            (PlugKind.Esphome, PlugAction.Read) => Get($"{at}/switch/{plug.Entity}"),

            _ => throw new ArgumentOutOfRangeException(nameof(kind)),
        };
    }

    /// The state a READBACK body names, or `Unknown`.
    ///
    /// THE ONE THING THIS FUNCTION DOES NOT DO, and it is the reason it exists
    /// separately from the request builder: it never reads a command's own
    /// response as the new state. Shelly Gen2's `Switch.Set` answers
    /// `{"was_on":false}` — the PREVIOUS state — so a tool that accepted a
    /// command response as confirmation would report "off -> on" as done at the
    /// exact moment the relay did nothing. There is no `was_on` key below, on
    /// purpose. `LE-87`'s lesson (half a success reported as a success) applied
    /// before the defect rather than after it.
    ///
    /// Total and non-throwing. An empty 200, a truncated body, a newer firmware
    /// with a renamed field: all `Unknown`, because on a mains seam "I could not
    /// tell" and "it is off" must not be the same value.
    internal static PlugState ReadState(PlugKind kind, string body)
    {
        if (string.IsNullOrWhiteSpace(body)) return PlugState.Unknown;

        JsonElement root;
        try
        {
            using var document = JsonDocument.Parse(body);
            root = document.RootElement.Clone();
        }
        catch (JsonException)
        {
            return PlugState.Unknown;
        }
        if (root.ValueKind != JsonValueKind.Object) return PlugState.Unknown;

        return kind switch
        {
            // `POWER` for a single-relay device, `POWER1` for the first relay of
            // a multi-relay one. Tasmota answers whichever the firmware build
            // has; both mean relay one, which is the one this bench wires.
            PlugKind.Tasmota => Word(root, "POWER") ?? Word(root, "POWER1") ?? PlugState.Unknown,
            PlugKind.ShellyGen1 => Flag(root, "ison"),
            PlugKind.ShellyGen2 => Flag(root, "output"),
            PlugKind.Esphome => Word(root, "state") ?? Flag(root, "value"),
            _ => PlugState.Unknown,
        };
    }

    /// A boolean field, where anything that is not exactly `true`/`false` — a
    /// missing key, a null, a string "true" from a firmware that changed its
    /// mind — is `Unknown` rather than the falsy branch.
    private static PlugState Flag(JsonElement root, string name)
    {
        if (!root.TryGetProperty(name, out var value)) return PlugState.Unknown;
        return value.ValueKind switch
        {
            JsonValueKind.True => PlugState.On,
            JsonValueKind.False => PlugState.Off,
            _ => PlugState.Unknown,
        };
    }

    /// An `ON`/`OFF` string field. Returns null for "this key is not here" so
    /// the caller can try the next spelling; `Unknown` for "this key is here and
    /// says something I do not recognise", which must NOT fall through to a
    /// second key that might disagree.
    private static PlugState? Word(JsonElement root, string name)
    {
        if (!root.TryGetProperty(name, out var value)) return null;
        if (value.ValueKind != JsonValueKind.String) return PlugState.Unknown;
        return value.GetString()?.Trim().ToUpperInvariant() switch
        {
            "ON" => PlugState.On,
            "OFF" => PlugState.Off,
            _ => PlugState.Unknown,
        };
    }

    private static PlugRequest Get(string url) => new(HttpMethod.Get, url);
    private static PlugRequest Post(string url) => new(HttpMethod.Post, url);

    /// The `--dialect` spelling, or null. Named rather than parsed by
    /// `Enum.TryParse` so the accepted words are the documented ones and a
    /// typo is refused instead of matching a member the help never mentioned.
    internal static PlugKind? ParseKind(string? name) => name?.Trim().ToLowerInvariant() switch
    {
        "tasmota" => PlugKind.Tasmota,
        "shelly-gen1" or "shelly1" => PlugKind.ShellyGen1,
        "shelly-gen2" or "shelly2" => PlugKind.ShellyGen2,
        "esphome" => PlugKind.Esphome,
        _ => null,
    };
}
