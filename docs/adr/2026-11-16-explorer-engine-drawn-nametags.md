# ADR: The Explorer draws nametags and chat bubbles in the engine

- Status: Accepted
- Date: 2026-11-16

## Context

A player's nametag shows their name, badges and voice state above their avatar, and grows into a
chat bubble when they send a message.

The bridge scene (`rendering/bevy-explorer/source/react-web/bridge-scene`) drew them. Each tag was
a React-ECS UI tree rendered into a 640×440 texture on a lit, billboarded 2.4 m plane attached to
the avatar's name-tag attach point. The scene learned of players, profiles, voice activity, mutes
and chat through its API streams and wrote transforms back each tick.

That placed three limits on the tags:

- **Fidelity.** A fixed-size texture without mipmaps is blurry close up and shimmers far away, and a
  lit material takes the scene's lighting and fog. The plane had a fixed world size, because a
  stranded tag scaled by distance could fill the screen.
- **Timing.** Fade, position and bubble animation moved at the scene's tick, which falls below the
  render rate when the frame budget is tight; the fade was recomputed about three times a second.
- **Lifetime.** The scene polled the player list and hid a tag after three missed polls. It could
  not remove attached entities reliably, so hidden tags were parked below the world. One exception
  in the scene, or the scene failing to load, removed every nametag.

The engine already holds everything a tag shows: the player entity and its profile, the avatar's
bones after IK, the active speakers, the mute list and the filtered chat stream.

## Decision

The engine draws nametags and chat bubbles for players, and nametags for the avatars a scene
makes with `AvatarShape`. `NametagPlugin` (`crates/avatar/src/nametag/`) owns them; the bridge
scene draws none.

- **One mesh and one draw call per tag, in the 3D transparent pass.** Every tag uses the same
  unlit material (`NametagMaterial`, `nametag.wgsl`): alpha blended, depth tested, no depth write,
  no fog. Triangles of one draw paint in index order, so the background, border, icons and text
  need no depth bias. A per-vertex kind selects the fragment path: SDF glyph, colour bitmap
  (emoji), tinted icon, rounded rectangle, or voice bar. Voice bars animate in the shader from the
  global time, so speaking never rebuilds a mesh.
- **Depth tested.** A tag is hidden by walls and by other avatars, like any object in the world.
- **Text shares the SDF glyph pipeline of `TextShape`.** The atlas, rasteriser and layout in
  `scene_runner::update_world::text_shape` are public, and nametags use their own atlas key
  (`AtlasKey::Engine("nametag")`), so scene text cannot evict name glyphs. A tag's
  `AtlasGlyphUse` component keeps its glyphs counted as in use. The coverage maths is one WGSL
  module (`sdf_coverage.wgsl`) imported by both shaders. Tags build text spans directly, so a name
  or message cannot inject markup.
- **Placement on the CPU, every frame.** After the avatar's bones are final, each shown tag is
  placed at the highest of the head, toes and hips, plus the height of what the avatar wears above
  its head bone, measured in the head bone's bind space when the avatar is built. It faces the
  camera plane and is scaled by distance so that it keeps its size on screen. It is shown from 2 m
  to 40 m and fades over the second half of that range. Opacity travels in the tag's `MeshTag`,
  so a fade changes neither the mesh nor the material.
- **State is on entities.** A player carries `NametagContent` and `NametagVoice`. A shown tag is
  its own entity, related one-to-one to its player (`NametagOf` / `ShownNametag`), and carries the
  `ChatBubble` it is showing. Despawning the player removes the relationship, and an observer
  frees the tag, so a disconnect needs no polling.
- **A scene avatar is named from its `AvatarShape`.** Once the avatar is built it carries the
  same `NametagContent`, marked `SceneAvatarNametag`: its `name` (`NPC` when unset; an empty
  name asks for no tag), `#` and the last four characters of its `id` as the suffix (`#0000` for
  a shorter id), never the verified or official badge, and the voice bars while `talking` is
  set. A scene entity that only restyles a player gets none, nor do the HUD's preview avatars,
  which a system scene makes. Avatar modifier areas act on players, so they do not hide these
  tags.
- **Tags are pooled.** A tag is released when its player is out of range, behind the camera,
  hidden by an avatar modifier area, beyond the avatar cap, the local player in first person, or
  when nametags are off. Released tags keep their mesh handle and wait hidden; at most 32 wait,
  and the rest are despawned.
- **Meshes rebuild on change only.** A tag's mesh is rebuilt when its content changes, while its
  bubble animates, or when the glyph atlas is repacked. An idle tag costs one transform write a
  frame.
- **Bubbles come from the filtered chat stream.** `pipe_chats_to_scene` writes a
  `ChatBubbleRequest` where it forwards a nearby, community or direct message, after block and ban
  filtering. A bubble is shown only if its sender has a tag at that moment; nothing is queued. The
  setting `ChatBubbleVisibility` chooses all messages, nearby only, or none.
- **What a peer sends is bounded and checked before it is drawn.** A bubble shows the first 100
  characters of a message on wrapped lines, with line breaks read as spaces; a community's name
  shows its first 28 characters and a player's name its first 32. An unclaimed name shows the
  last four characters of its address, and anything after a `#` in the name itself is dropped.
  The verified seal needs a claimed name on a profile with a connected wallet: a guest's profile
  is only the guest's word.
- **The foundation badge has an input and no source.** `OfficialWallets` lists the wallets that
  show it. Nothing fills it yet.
- **Observable through the console.** `/nametag_state` reports every shown tag without message
  text, `/nametag_bubble` raises a test bubble, and the runtime debug panel has a *Nametags*
  section. The end-to-end driver reads the same commands.

## Rejected

- **A texture per tag** (what the scene did, moved into the engine): one camera and one texture
  per tag, and text that is only sharp at one distance.
- **A `bevy_ui` overlay positioned from the world**: it draws after the 3D pass without the depth
  buffer, so tags would show through walls.
- **A DOM overlay in the HUD**: positions would arrive over the bridge at the scene's tick, and on
  desktop the HUD is a texture composited over the frame, with no depth either.
- **A dedicated pass after tonemapping**: it would give exact colours, but needs its own render
  node reading prepass depth and its own handling of anti-aliasing and depth of field. The
  material approximately inverts the tonemapper instead. This is the fallback if the colours are
  measurably off.
- **Keeping the scene and raising its tick or texture size**: neither removes the single point of
  failure, and the texture atlas is bounded by the device's texture size.
- **Smoothing the anchor's height**: the previous quarter-second hold kept a tag above an avatar's
  highest recent pose, and lagged behind it when the avatar sat or crouched.
- **Queueing bubbles for players without a tag**: a message would appear seconds late over a
  player who has just come into range.

## Consequences

- Nametags no longer depend on the bridge scene loading or running, and a departed player's tag is
  freed in the frame the player despawns.
- Changing how a tag looks needs an engine build; a scene deploy no longer changes it.
- Tags pass through tonemapping, bloom and anti-aliasing with the rest of the 3D pass. Their
  colours are close to the specified ones, not exact.
- A scene item attached to the name-tag attach point now follows the avatar's bones every frame.
- A player's name colour is derived from their name by one hash, shared with the HUD
  (`library/explorer-core/engine/nameColor.ts`, same test vectors). The loading ghost and the
  point-at marker use it too.
- `SetCommunityChannels` carries each community's name beside its id, so a community bubble can
  show it.
- Names in scripts the nametag font and the emoji font do not cover show missing glyphs.

**Measured.** Web builds of this change and of its base (`main` `6d490cdf9`), both without
wasm-opt, in Chrome 157 on an Apple M1 Pro, driven through the console by the end-to-end driver
on 2026-11-16. Each run signs in as a guest at Genesis Plaza (0,-1) on the production realm, lets
the world settle for 90 s, then samples `/performance_state` once a second for 60 s and the CPU
time of the browser's processes over the same window. The builds alternate. Five or six other
players were present, and 4 to 7 nametags were shown.

| | base (4 runs) | this change (5 runs) |
|---|---|---|
| frame rate, mean of the runs | 32.0 fps | 33.5 fps |
| frame rate, range of the runs | 31.1 to 33.0 | 32.9 to 34.1 |
| frame time, median of the runs' medians | 33.2 ms | 30.9 ms |
| frames over 50 ms in the worst 1,000 frames, range | 26 to 81 | 28 to 78 |
| browser CPU, share of one core, mean | 188 % | 191 % |
| engine memory, mean | 811 MiB | 784 MiB |

The frame rate is about 1.5 fps (5 %) higher at the same CPU use. Slow frames and memory are
within the spread between runs. The crowd was small, so this does not show how either build
scales with many players, and GPU timestamps were not available in the browser. Five further
runs (three of the base, two of this change) are left out: the plaza's scene did not load in
them, which halves the work in the frame.

## Follow-ups

- Fill `OfficialWallets` from feature flags once the Explorer has them.
- The SDK's nametag plate (`AvatarNametag`).
- A fallback font for CJK names.
- Measure a crowd of twenty or more players before and after, and on a desktop build.
- Sample the tag's fill colour against its specified value on the web build.
