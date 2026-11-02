# Glossary

- **Realm**: a content-and-comms endpoint the client connects to, described by its `/about`.
  Genesis City, each World, and a local preview server are all realms.
- **Catalyst**: the Decentraland content server network; serves entities and their files by
  hash. **Worlds content server**: the same for named Worlds (`foo.dcl.eth`).
- **Zone / org**: the dev and production environments (`decentraland.zone`,
  `decentraland.org`). Separate wallets, separate rollouts.
- **Scene**: a deployed entity (JS bundle plus content map) that runs in its own isolate or
  worker and owns a set of parcels. **Portable** (portable experience): a scene with no
  parcels, attached to the player.
- **Scene entity vs bevy entity**: the scene addresses entities by its own small integer ids;
  the engine spawns a bevy entity per scene entity under the scene root and maps between them.
- **CRDT**: the scene-to-engine state protocol. Each SDK component is either **LWW**
  (last-write-wins, one value per entity) or **GO** (grow-only set, append-only).
  `CrdtStateComponent` on the scene root holds the pending deltas per component type.
- **`Pb*` types**: the protobuf-generated SDK component types from the `protocol` repo. The
  engine converts them into its own components.
- **System scene / super-user scene**: the one scene the engine trusts, launched with
  `--system-scene`, holding the `SystemApi` sender. On web this is the **bridge scene**, which
  sits between the HUD page and the engine and signs requests.
- **`SystemApi`**: the privileged message channel from the system scene to the engine.
  **`RpcCall`**: the ordinary scene-to-engine request channel every scene has.
- **Restricted actions**: scene requests that may act on the player or the client (move,
  teleport, open URL), subject to the permissions table.
- **Sidecar**: the `dcl_deno_ipc` process that runs scene JS on native. **Sandbox worker**:
  the per-scene web worker. **Relay**: the engine-side half of the web scene channel in
  `dcl_wasm`, copying messages between memories.
- **Shell page / app page**: on web, the top-level document that holds the sign-in key, and
  the isolated iframe that runs the HUD and engine.
- **HUD**: the user interface. `react-web` is the React implementation, rendered as DOM on
  web and through CEF on native. Sites' `/play` mounts an adapted copy of it,
  `apps/ui3/src/play`. **System UI**: the older in-engine bevy UI in `system_ui` and
  `ui_core`.
- **CEF**: Chromium Embedded Framework, which renders the HUD into a texture on native.
- **Imposter** (boimp): a baked billboard stand-in for a distant scene, rendered instead of
  the scene's geometry.
- **Collectible**: a wearable or emote. **Base body / body shape**: the avatar's base mesh.
- **Comms**: the multiplayer transport. **Archipelago** groups players into islands;
  **livekit** carries voice and realtime data; **Pulse** is the authoritative-server
  transport over WebTransport.
- **Headless / server mode / orchestrated**: the engine running scenes with no renderer as
  an authoritative scene server, possibly many scenes under an orchestrator.
- **Signed fetch**: an HTTP request carrying an auth chain signed by the player's key, so a
  scene backend can trust who is asking.
- **Guest**: a session with a throwaway wallet and no login.
- **SDK / js-sdk-toolchain**: the scene authoring library and CLI. **Creator Hub**: the
  desktop editor that embeds this client. **Unity explorer**: the reference client, used for
  behaviour comparisons.
