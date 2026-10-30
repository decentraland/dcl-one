# Recipes

How to add the things that get added most often. Each one names the template to copy; copy it
whole rather than assembling from pieces.

## A new SDK component

Register it in `update_world` with `add_crdt_lww_component::<PbFoo, Foo>` (or
`add_crdt_go_component` for grow-only sets). The registration adds the `UpdateWorld` system
that converts each delta into your component via `TryFrom<PbFoo>`; you don't add a system to
the scene loop by hand. Systems that react to the component go in `SceneSets::PostLoop`. The
proto type comes from `dcl_component`; never redeclare it.

## A new engine-to-system-scene stream

Template: the proximity stream, `GetProximityStream` in
`crates/scene_runner/src/update_scene/pointer_results.rs` and
`react-web/bridge-scene/src/domains/proximity.ts`, added whole in #764. Not the chat stream,
which drags in player, wallet and marker filtering you don't want. Smallest plumbing-only
reference: `getBlockUpdateStream` (#844). The file set:

1. `crates/system_api_types/src/lib.rs`: the payload struct, `#[serde(rename_all =
   "camelCase")]`, `#[derive(ts_rs::TS)] #[ts(export)]`, `#[ts(type = "number")]` on 64-bit
   fields. It must be a **new, distinct struct**: the receiver is stored in scene state keyed
   by Rust type, so two streams sharing a payload type fight over one slot.
2. `crates/system_bridge/src/lib.rs`: the `SystemApi::GetFooStream(RpcStreamSender<Foo>)`
   variant.
3. The producing system: `EventReader<SystemApi>` collects senders, retains the open ones,
   early-outs when empty, diffs against previous state, and sends fire-and-forget.
   `pipe_connectivity_events_to_scene` in `crates/social/src/lib.rs` is the cleanest example.
4. `crates/dcl_js/src/system_api.rs`: `op_get_foo_stream` and `op_read_foo_stream`.
5. `crates/dcl_deno/src/js/op_wrappers/system_api.rs`: the two native op wrappers, added to
   the super-user ops list.
6. `crates/dcl_wasm/src/inner/op_wrappers/system_api.rs`: the two web wrappers. No list to
   edit; the sandbox worker scans every `op_*` export. Only a wasm build compiles these.
7. `crates/dcl_js/src/modules/SystemApi.js`: `getFooStream` as an async generator over the
   two ops.
8. `react-web/bridge-scene/src/bevy-api.ts`: the method on `BevyApiInterface`, importing the
   generated type; a relay domain under `bridge-scene/src/domains/`; a message kind in
   `react-web/src/engine/protocol.ts`; a case in `useEngineSession.ts`; the mock bridge.

Run `just ts-bindings` before touching the TypeScript: the generated types under
`react-web/src/engine/generated/` are gitignored, and CI regenerates them before every TS
build, so a Rust struct change that breaks the page fails `tsc` there.

## The ts-rs boundary

`crates/system_api_types` holds the plain serde structs that cross to the HUD and the system
scene; `scripts/gen-ts-bindings.sh` is the single source of truth for generating their TS.
Rules that follow from the transport:

- `skip_serializing_if` is unsafe on these types. Natively they cross the scene-process IPC
  as positional msgpack, so a skipped field shortens the array and decode fails.
- 64-bit integers get `#[ts(type = "number")]`; `Option` becomes `| null`.
- `serde_repr` enums get no ts derive (the wire is numeric, ts-rs would emit a string union);
  use a field-level `ts(type = "number")` instead.

## A new engine parameter

The shared clap `LaunchOptions` in `crates/system_api_types/src/launch_options.rs` is the one
definition: native flags are the kebab-case web names, with no aliases. The web-param table in
`web_params.rs` is derived from the clap metadata and exported to the page with the ts
bindings, so a new option appears in both places from one edit. On web the page reads it with
`webParam(name)` and the engine receives it through `engine_run(options)`, which rejects
unknown fields. Whether a param is trusted is the front end's concern (`lib/launchGate.ts`),
not the engine's. A **web-only** switch is not a `LaunchOptions` field at all: it is a page
param handed to a wasm export, like `?guest=`.

## A new console command

`app.add_console_command::<FooCommand, _>(foo_system)` with a `bevy_console::Command` struct.
`add_preview_console_command` registers it only in preview mode; outside preview it is absent
from `/help` and rejected. Console commands land in `ConsoleSet::Commands`, just before
`SceneSets::Init`, so their effects are visible to the scene loop in the same frame.
