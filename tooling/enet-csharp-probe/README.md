# ENet-CSharp interop probe

The ENet-CSharp side of the vendored rusty_enet's native interop test,
`test::native_csharp_all_lanes_and_controlled_sender_throttle` in
[`library/vendor/rusty_enet/src/test.rs`](../../library/vendor/rusty_enet/src/test.rs). It shows that
the Rust ENet that Pulse and the native Bevy Explorer share
([ADR](../../docs/adr/2026-10-28-shared-pulse-enet.md)) talks to ENet-CSharp as Unity Explorer ships
it.

The test is `#[ignore]`d, and [`tooling/dev-regressions.json`](../dev-regressions.json) keeps it ignored
in the crate's default run, which the Development workflow uses. The
[ENet-CSharp interop workflow](../../.github/workflows/enet-csharp-interop.yml) builds this probe and runs
the test on Linux for every change to the vendored crate, the probe or the workflow.

## What it checks

The test starts a Rust host on a local UDP port and runs `dotnet NativeProbe.dll <port> <trace file>`.
The probe connects as Unity Explorer's Pulse transport does: one peer and the three lanes of
`Pulse.Transport.ENetChannel` with their packet flags.

1. The probe sends one packet per lane: 4096 bytes on reliable lane 0, 32 on lane 1 (sequenced,
   `Unthrottled`) and 32 on lane 2 (`Unsequenced`). The host checks each one's lane, first byte and length.
2. The host sets its send throttle to 0. It sends `[99]` as an ordinary unreliable packet on lane 0 and
   `[100]`..`[107]` on lane 1 with the per-packet throttle bypass.
3. It restores the throttle, sends a full-size and a one-byte packet on each lane and disconnects
   with reason 16.

The probe passes only if lane 0 received the 4096-byte and one-byte packets without `[99]`, lane 1 received
`[100]`..`[107]` and then its two packets in order, lane 2 received its two packets, and the disconnect
carried reason 16. It writes each step to the trace file, which the test prints, and exits 0 only on a pass.

## Pinned inputs

[`NativeProbe.csproj`](NativeProbe.csproj) holds the pins. The build downloads each file once into
`obj/pinned/<sha256>/` and stops if its SHA-256 differs.

| File | Source |
| --- | --- |
| `ENet.cs`, `ENetChannel.cs` | The ENet-CSharp 2.4.9 wrapper ([SoftwareGuy/ENet-CSharp](https://github.com/SoftwareGuy/ENet-CSharp)) and the Pulse lanes, from the `com.decentraland.pulse.transport` package at decentraland/Pulse `994b07ee16`, the commit unity-explorer's `Packages/packages-lock.json` pins |
| `libenet.so` (linux-x64), `enet.dll` (win-x64) | unity-explorer `Explorer/Assets/Plugins/ENet/runtimes` at `4c85b8603e` (main, 2026-11-19) |

Update the pins when unity-explorer moves its `com.decentraland.pulse.transport` lock or changes its
`Plugins/ENet` libraries: change `UnityExplorerRevision`, `PulseRevision` and the `Sha256` values, then
run the test. Only linux-x64 and win-x64 are pinned; the build stops on other machines.

## Run it locally

It needs the .NET 10 SDK and `dotnet` on `PATH`. From the repository root (bash):

```sh
dotnet build tooling/enet-csharp-probe/NativeProbe.csproj -c Release -o "$PWD/.cache/enet-csharp-probe"
PULSE_NATIVE_PROBE="$PWD/.cache/enet-csharp-probe/NativeProbe.dll" \
PULSE_NATIVE_TRACE="$PWD/.cache/enet-csharp-probe/trace.txt" \
  cargo +1.99.1 test --locked --manifest-path library/vendor/rusty_enet/Cargo.toml --lib -- \
  --exact test::native_csharp_all_lanes_and_controlled_sender_throttle --include-ignored --nocapture
```

## Origin

The test came with the shared ENet import. Its probe was built outside the repository, and its source was
never committed. This probe was written to the test's contract and pinned to what Unity Explorer ships.
