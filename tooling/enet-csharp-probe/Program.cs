// The ENet-CSharp side of the vendored rusty_enet's native interop test,
// test::native_csharp_all_lanes_and_controlled_sender_throttle in library/vendor/rusty_enet/src/test.rs:
//
//   dotnet NativeProbe.dll <port> <trace file>
//
// It connects to the Rust host on 127.0.0.1:<port> as Unity Explorer's Pulse transport does (one peer, the
// three lanes of Pulse.Transport.ENetChannel with their packet flags) and sends one packet per lane: 4096
// bytes of 10 on reliable lane 0, 32 bytes of 11 on sequenced lane 1 and 32 bytes of 12 on unsequenced
// lane 2. Once the host holds all three, it sets its send throttle to 0 and sends [99] as an ordinary
// unreliable packet on lane 0, which the throttle must drop, and [100]..[107] on lane 1 with the per-packet
// throttle bypass, which must arrive. It then restores the throttle, sends a full-size and a one-byte packet
// on each lane and disconnects with reason 16. The probe checks that exactly those packets arrived, writes
// what it saw to the trace file (the test prints it) and exits 0 only when everything matched.

using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.IO;
using System.Linq;
using ENet;
using Pulse.Transport;

internal static class Program
{
    // The Rust test gives up after 15 s; the probe stops first so the test still prints its trace.
    private const int DEADLINE_MS = 10_000;
    private const uint DISCONNECT_REASON = 16;

    private static readonly ENetChannel[] LANES =
    {
        ENetChannel.RELIABLE, ENetChannel.UNRELIABLE_SEQUENCED, ENetChannel.UNRELIABLE_UNSEQUENCED,
    };

    // What the probe sends: the host checks the first byte (10 + lane) and the length of each.
    private static readonly byte[][] SENT = { Fill(10, 4096), Fill(11, 32), Fill(12, 32) };

    // What the host sends, per lane in send order. Lane 0 never carries [99]: the zero throttle drops it.
    private static readonly string[][] EXPECTED =
    {
        new[] { Describe(Fill(10, 4096)), Describe(Fill(110, 1)) },
        Enumerable.Range(100, 8).Select(id => Describe(Fill((byte)id, 1)))
                  .Concat(new[] { Describe(Fill(11, 32)), Describe(Fill(111, 1)) })
                  .ToArray(),
        new[] { Describe(Fill(12, 32)), Describe(Fill(112, 1)) },
    };

    private static int Main(string[] args)
    {
        if (args.Length != 2 || !ushort.TryParse(args[0], out ushort port))
        {
            Console.Error.WriteLine("usage: dotnet NativeProbe.dll <port> <trace file>");
            return 2;
        }

        var trace = new List<string>();
        int status;

        try { status = Run(port, trace) ? 0 : 1; }
        catch (Exception e)
        {
            trace.Add($"FAIL: {e}");
            status = 1;
        }

        File.WriteAllLines(args[1], trace);
        Console.WriteLine($"NativeProbe: {(status == 0 ? "pass" : "fail")}, trace in {args[1]}");
        return status;
    }

    private static bool Run(ushort port, List<string> trace)
    {
        // Initialize refuses a native library whose version differs from the wrapper's.
        if (!Library.Initialize())
        {
            trace.Add($"FAIL: the native library is not ENet-CSharp {VersionText(Library.version)}");
            return false;
        }

        trace.Add($"ENet-CSharp {VersionText(Library.version)} initialized");

        try { return Exchange(port, trace); }
        finally { Library.Deinitialize(); }
    }

    private static bool Exchange(ushort port, List<string> trace)
    {
        using var host = new Host();
        var address = new Address();
        address.SetHost("127.0.0.1");
        address.Port = port;

        // As Unity Explorer's ENetTransport.ConnectAsync: one peer, one channel per lane.
        host.Create(peerLimit: 1, channelLimit: ENetChannel.COUNT);
        Peer server = host.Connect(address, channelLimit: ENetChannel.COUNT);

        List<string>[] received = LANES.Select(_ => new List<string>()).ToArray();
        uint? disconnectReason = null;
        var clock = Stopwatch.StartNew();

        while (disconnectReason == null)
        {
            if (clock.ElapsedMilliseconds > DEADLINE_MS)
            {
                trace.Add($"FAIL: the host did not disconnect within {DEADLINE_MS} ms");
                break;
            }

            // Drain the queued events before waiting on the socket again, as the transport does.
            int serviced = host.CheckEvents(out Event netEvent);

            if (serviced == 0)
                serviced = host.Service(15, out netEvent);

            if (serviced < 0)
            {
                trace.Add("FAIL: servicing the host failed");
                return false;
            }

            if (serviced == 0)
                continue;

            switch (netEvent.Type)
            {
                case EventType.Connect:
                    trace.Add("connected");

                    for (var lane = 0; lane < LANES.Length; lane++)
                    {
                        var packet = default(Packet);
                        packet.Create(new Span<byte>(SENT[lane]), LANES[lane].PacketMode);
                        int result = server.Send(LANES[lane].ChannelId, ref packet);
                        trace.Add($"sent lane={LANES[lane].ChannelId} flags={LANES[lane].PacketMode} payload={Describe(SENT[lane])} result={result}");

                        if (result < 0)
                            return false;
                    }

                    host.Flush();
                    break;

                case EventType.Receive:
                {
                    using Packet packet = netEvent.Packet;
                    var payload = new byte[packet.Length];
                    packet.CopyTo(payload);
                    trace.Add($"received lane={netEvent.ChannelID} payload={Describe(payload)}");

                    if (netEvent.ChannelID >= received.Length)
                    {
                        trace.Add($"FAIL: lane {netEvent.ChannelID} is outside the {ENetChannel.COUNT} negotiated lanes");
                        return false;
                    }

                    received[netEvent.ChannelID].Add(Describe(payload));
                    break;
                }

                case EventType.Disconnect:
                    disconnectReason = netEvent.Data;
                    trace.Add($"disconnected reason={netEvent.Data}");
                    break;

                case EventType.Timeout:
                    trace.Add("FAIL: the connection timed out");
                    return false;
            }
        }

        bool pass = disconnectReason == DISCONNECT_REASON;

        if (disconnectReason is uint reason && reason != DISCONNECT_REASON)
            trace.Add($"FAIL: disconnect reason {reason}, expected {DISCONNECT_REASON}");

        for (var lane = 0; lane < LANES.Length; lane++)
        {
            // Unsequenced packets have no delivery order; the other two lanes keep the send order.
            bool match = LANES[lane].PacketMode.HasFlag(PacketFlags.Unsequenced)
                ? received[lane].OrderBy(p => p, StringComparer.Ordinal).SequenceEqual(EXPECTED[lane].OrderBy(p => p, StringComparer.Ordinal))
                : received[lane].SequenceEqual(EXPECTED[lane]);

            trace.Add($"lane {lane} {(match ? "ok" : "FAIL")}: expected [{string.Join(", ", EXPECTED[lane])}], received [{string.Join(", ", received[lane])}]");
            pass &= match;
        }

        trace.Add(pass ? "PASS" : "FAIL");
        return pass;
    }

    private static byte[] Fill(byte value, int length) =>
        Enumerable.Repeat(value, length).ToArray();

    // "11x32" for 32 bytes of 11; other payloads in hexadecimal.
    private static string Describe(byte[] payload) =>
        payload.Length > 0 && payload.All(b => b == payload[0]) ? $"{payload[0]}x{payload.Length}" : $"0x{Convert.ToHexString(payload)}";

    private static string VersionText(uint version) =>
        $"{version >> 16}.{(version >> 8) & 0xff}.{version & 0xff}";
}
