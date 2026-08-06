/// The four plug dialects, as pure request-building and response-reading.
///
/// `LE-95` fixes ONE property for the device: it must be controllable over the
/// LAN with no vendor cloud account. That rules a great deal in rather than out
/// — Tasmota, ESPHome and both Shelly HTTP generations all qualify — so the
/// tool supports the four and the bench can buy whichever is in stock. Choosing
/// a dialect at run time is only safe if each dialect is a pure function that
/// can be read without a plug on the desk, which is what this file asserts.
public sealed class PlugDialectTests
{
    // ---- request building -------------------------------------------------

    [Theory]
    [InlineData(PlugAction.On, "http://10.0.0.9/cm?cmnd=Power%20On")]
    [InlineData(PlugAction.Off, "http://10.0.0.9/cm?cmnd=Power%20Off")]
    [InlineData(PlugAction.Read, "http://10.0.0.9/cm?cmnd=Power")]
    public void Tasmota_speaks_cm_cmnd(PlugAction action, string expected)
    {
        var request = PlugDialect.Request(PlugKind.Tasmota, Base("http://10.0.0.9"), action);
        Assert.Equal(expected, request.Url);
        Assert.Equal(HttpMethod.Get, request.Method);
    }

    [Theory]
    [InlineData(PlugAction.On, "http://10.0.0.9/relay/0?turn=on")]
    [InlineData(PlugAction.Off, "http://10.0.0.9/relay/0?turn=off")]
    [InlineData(PlugAction.Read, "http://10.0.0.9/relay/0")]
    public void Shelly_gen1_speaks_relay(PlugAction action, string expected)
    {
        var request = PlugDialect.Request(PlugKind.ShellyGen1, Base("http://10.0.0.9"), action);
        Assert.Equal(expected, request.Url);
    }

    [Theory]
    [InlineData(PlugAction.On, "http://10.0.0.9/rpc/Switch.Set?id=0&on=true")]
    [InlineData(PlugAction.Off, "http://10.0.0.9/rpc/Switch.Set?id=0&on=false")]
    [InlineData(PlugAction.Read, "http://10.0.0.9/rpc/Switch.GetStatus?id=0")]
    public void Shelly_gen2_speaks_rpc(PlugAction action, string expected)
    {
        var request = PlugDialect.Request(PlugKind.ShellyGen2, Base("http://10.0.0.9"), action);
        Assert.Equal(expected, request.Url);
    }

    [Theory]
    [InlineData(PlugAction.On, "http://10.0.0.9/switch/board/turn_on")]
    [InlineData(PlugAction.Off, "http://10.0.0.9/switch/board/turn_off")]
    public void Esphome_actions_are_posts(PlugAction action, string expected)
    {
        var request = PlugDialect.Request(PlugKind.Esphome, Base("http://10.0.0.9", "board"), action);
        Assert.Equal(expected, request.Url);
        Assert.Equal(HttpMethod.Post, request.Method);
    }

    /// ESPHome's readback is a GET where its actions are POSTs, and getting
    /// that backwards produces a 405 that reads exactly like a plug that is not
    /// answering — an instrument failure wearing the costume of a device
    /// failure, which this bench has now had six of.
    [Fact]
    public void Esphome_readback_is_a_get()
    {
        var request = PlugDialect.Request(PlugKind.Esphome, Base("http://10.0.0.9", "board"), PlugAction.Read);
        Assert.Equal("http://10.0.0.9/switch/board", request.Url);
        Assert.Equal(HttpMethod.Get, request.Method);
    }

    [Fact]
    public void A_trailing_slash_on_the_base_does_not_double()
    {
        var request = PlugDialect.Request(PlugKind.Tasmota, Base("http://10.0.0.9/"), PlugAction.Read);
        Assert.Equal("http://10.0.0.9/cm?cmnd=Power", request.Url);
    }

    /// A bare host is refused rather than assumed to be `http://`. This tool
    /// drives mains power, and a base URL that silently becomes something other
    /// than what the operator typed is the first step of switching the wrong
    /// socket. Safety before convenience, in that order, always.
    [Theory]
    [InlineData("10.0.0.9")]
    [InlineData("plug.local")]
    [InlineData("ftp://10.0.0.9")]
    [InlineData("")]
    public void A_base_without_an_http_scheme_is_refused(string given)
    {
        Assert.Null(PlugBase.Parse(given, null));
    }

    [Fact]
    public void Esphome_without_an_entity_id_is_refused()
    {
        // Every other dialect addresses relay 0 implicitly; ESPHome names its
        // entity, and guessing `relay` would switch whatever happened to be
        // called that.
        Assert.Null(PlugBase.Parse("http://10.0.0.9", null, PlugKind.Esphome));
        Assert.NotNull(PlugBase.Parse("http://10.0.0.9", "board", PlugKind.Esphome));
    }

    // ---- reading a state back --------------------------------------------

    [Theory]
    [InlineData(PlugKind.Tasmota, "{\"POWER\":\"ON\"}", PlugState.On)]
    [InlineData(PlugKind.Tasmota, "{\"POWER\":\"OFF\"}", PlugState.Off)]
    [InlineData(PlugKind.Tasmota, "{\"POWER1\":\"ON\"}", PlugState.On)]
    [InlineData(PlugKind.ShellyGen1, "{\"ison\":true,\"has_timer\":false}", PlugState.On)]
    [InlineData(PlugKind.ShellyGen1, "{\"ison\":false}", PlugState.Off)]
    [InlineData(PlugKind.ShellyGen2, "{\"id\":0,\"output\":true,\"apower\":3.1}", PlugState.On)]
    [InlineData(PlugKind.ShellyGen2, "{\"id\":0,\"output\":false}", PlugState.Off)]
    [InlineData(PlugKind.Esphome, "{\"id\":\"switch-board\",\"state\":\"ON\",\"value\":true}", PlugState.On)]
    [InlineData(PlugKind.Esphome, "{\"id\":\"switch-board\",\"state\":\"OFF\",\"value\":false}", PlugState.Off)]
    public void A_readback_body_names_a_state(PlugKind kind, string body, PlugState expected)
    {
        Assert.Equal(expected, PlugDialect.ReadState(kind, body));
    }

    /// THE TRAP, and it is the reason `ReadState` is only ever applied to a
    /// readback. Shelly Gen2's `Switch.Set` answers `{"was_on":false}` — the
    /// PREVIOUS state, not the new one. A tool that treats a command's own
    /// response as confirmation reports "off -> on" as done at the exact moment
    /// the relay did nothing, which is `LE-87`'s half-a-success signature on a
    /// mains path. There is no `was_on` key in `ReadState`, deliberately.
    [Fact]
    public void A_set_response_never_names_the_new_state()
    {
        Assert.Equal(PlugState.Unknown, PlugDialect.ReadState(PlugKind.ShellyGen2, "{\"was_on\":false}"));
        Assert.Equal(PlugState.Unknown, PlugDialect.ReadState(PlugKind.ShellyGen2, "{\"was_on\":true}"));
    }

    [Theory]
    [InlineData("")]
    [InlineData("   ")]
    [InlineData("not json at all")]
    [InlineData("{")]
    [InlineData("{\"POWER\":\"MAYBE\"}")]
    [InlineData("{\"unrelated\":1}")]
    [InlineData("[]")]
    [InlineData("null")]
    public void Anything_unreadable_is_Unknown_and_never_a_throw(string body)
    {
        foreach (var kind in Enum.GetValues<PlugKind>())
        {
            Assert.Equal(PlugState.Unknown, PlugDialect.ReadState(kind, body));
        }
    }

    /// An empty 200 is the platform semantic this bench has been bitten by in
    /// the other direction (`Ok(0)` on a stream seam). Named here because a
    /// plug that answers 200 with nothing is indistinguishable from a working
    /// one to any code that only checks the status line.
    [Fact]
    public void A_dialect_never_infers_state_from_a_status_code()
    {
        Assert.Equal(PlugState.Unknown, PlugDialect.ReadState(PlugKind.Tasmota, ""));
    }

    private static PlugBase Base(string url, string? entity = null) =>
        PlugBase.Parse(url, entity, entity is null ? PlugKind.Tasmota : PlugKind.Esphome)!;
}
