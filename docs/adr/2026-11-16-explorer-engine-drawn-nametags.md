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
- **One tag per player, as the player's child.** A player with a profile carries
  `NametagContent` and `NametagVoice`, and has one tag entity as a child for as long as it has
  that content. Despawning the player despawns the tag, so a disconnect needs no polling and
  there is no pool: a tag is a handful of components, and reusing one saved nothing, since its
  name was shaped and its mesh uploaded again for the next player.
- **A scene avatar is named from its `AvatarShape`.** Once the avatar is built it carries the
  same `NametagContent`, marked `SceneAvatarNametag`: its `name` (an unset or blank
  name asks for no tag), `#` and the last four characters of its `id` as the suffix (`#0000` for
  a shorter id), never the verified or official badge, and the voice bars while `talking` is
  set. As in a player's name, what follows a `#` in the name is dropped; a name with nothing
  before one reads `NPC`. The tag goes while the avatar is rebuilt for a new outfit, and comes
  back once the new body is built. A scene entity that only restyles a player gets none, nor,
  from their name, do the HUD's preview avatars, which a system scene makes. Avatar modifier
  areas act on players, so they do not hide these tags; a scene hides its avatar's tag by hiding
  the avatar.
- **A system scene puts a tag on its own avatar by call.** `SystemApi::SetAvatarShapeNametag`
  (the bridge's `setAvatarShapeNametag`) names an entity, read in the system scene's own context,
  and a tag or none; of several words on an entity in one engine update, the last counts. The
  word doesn't name its scene, so it is taken only while there is one system scene. A tag
  asked before the engine has the entity waits until that scene names it, and is dropped if the
  entity dies or the scene goes. The engine keeps the tag on the entity as `SystemSceneNametag`
  until it is taken away or the avatar's `AvatarShape` is removed, and makes it the avatar's
  `NametagContent` once the avatar is built: the name; for a claimed NAME the verified badge and
  no suffix, else `#` and the end of the id; the given colour or the engine's for the name; and a
  size, a multiple of a player's from 0.25 to 4, by which the tag's mesh is scaled. The tag shows
  for the camera that draws the avatar: it skips the range from the player's camera and inherits
  the avatar's render layers. The in-world Shop's NAMEs pages put the NAME on their preview avatar
  this way.
- **A scene's plate is part of the avatar's tag.** `AvatarNametag` on a scene entity asks for a
  plate over the name of the avatar the entity stands for: the local player, another player, or
  an avatar the scene made, once its body is built. The avatar carries a `NametagPlate` beside
  its `NametagContent`, and has its one tag for as long as it has either; the plate is more
  quads in the tag's mesh, so it is placed, scaled, faded and hidden with the name. It floats
  over the name's box and stays there while a bubble opens under it. An avatar with no name
  (a scene avatar with a blank one) shows the plate alone, standing on the anchor. Only the
  scene on the parcel the local player stands on plates avatars, and it plates another player
  only while they stand in it too; a portable experience plates no one. While any scene asks
  for a plate, the plates are worked out every frame from what the local player's scene holds,
  so a plate goes with its component, its entity, its scene or the player who leaves; the other
  players' places are read only when that scene plates one of them. A plate is for the player
  it was written for: an entity id freed by a player who left comes back as a new generation.
  While a mention's ring shows, the plate stands clear of it. The label is one line, cut with an ellipsis at the plate's widest; colours the scene
  leaves out are the tag's own, and a border left out takes the fill's.
- **The anchor on the CPU, the rest in the vertex shader.** After the avatar's bones are final,
  the tag's translation is set to the player's name-tag anchor: the highest of the head, toes and
  hips, plus the height of what the avatar wears above its head bone, measured in the head bone's
  bind space when the avatar is built. The anchor is relative to the player, so transform
  propagation places the tag from the player's transform of the same frame. The vertex shader
  turns the tag to the camera plane, scales it by distance under a perspective camera (not under
  an orthographic one, such as a HUD preview's) so that it keeps its size on screen,
  and fades it over the second half of its 2 m to 40 m range. Nothing on an idle tag changes from
  frame to frame except, when its player moves, its place in the world. Its bounds are a cube
  around the anchor that covers the mesh at the far end of the range, where it is largest,
  sized against a scene's scale of its avatar. The anchor is worked out in the avatar's own
  units, so a scaled, turned or tilted avatar keeps its tag straight over its head.
- **A name that should not show is hidden, not removed.** An area that hides nametags hides the
  name and its bubble, and leaves a scene's plate standing in the name's place. Out of range,
  hidden by an avatar modifier area that hides avatars, beyond the avatar cap, blocked either way or banned from the scene's room, the
  local player in first person (with a scene's camera override applied), not visible (its own
  visibility or a parent's), scaled to nothing, or with nametags off:
  the tag's `Visibility` is hidden and the tag is left as it is. A tag out of view is culled by
  the frustum like any mesh. A hidden tag is not rebuilt; it is brought up to date when it shows
  again.
- **Meshes rebuild on change only.** A tag's mesh is rebuilt, in place under one asset id, when
  what it was built from differs from what it should show: the content, the voice badge, the
  glyph atlas' generation, or how far its bubble has opened. A bubble opening or closing
  therefore rebuilds it each frame for about 0.4 s, and once more at its final size.
- **Bubbles come from the filtered chat stream.** `pipe_chats_to_scene` writes a
  `ChatBubbleRequest` where it forwards a nearby, community or direct message, after block and ban
  filtering. The `ChatBubble` is the sender's, not the tag's: a tag hidden for a moment shows the
  rest of the message when it is back, and a message whose five seconds ran out unseen is
  dropped. A blank message opens no bubble. The setting `ChatBubbleVisibility` chooses all
  messages, nearby only, or none.
- **A translation reaches the bubble through a typed call.** The HUD translates messages, so the
  bridge tells the engine the new words with `SystemApi::SetChatBubbleText`, which replaces the
  text of the bubble still showing that message and keeps its time. Words that arrive while the
  bubble is closing do not open it again.
- **Tags are tonemapped with the frame.** The material writes its colours as they are specified
  and the main pass tonemaps them with everything else in the frame.
- **What a peer sends is bounded and checked before it is drawn.** A bubble shows the first 100
  characters of a message on wrapped lines, with line breaks read as spaces; a community's name
  shows its first 28 characters and a player's name its first 32. A scene's plate shows the
  first 128 characters of its label, and its colour channels are kept between 0 and 1. An unclaimed name shows the
  last four characters of its address in lower case, and anything after a `#` in the name itself
  is dropped. The verified seal needs a claimed name on a profile that says it has a connected
  wallet. Both flags are the peer's word: nothing confirms them against the catalyst yet.
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
- **A dedicated pass after tonemapping**: it would give exactly the specified colours and draw
  tags over every other transparent object, but needs its own render node reading prepass depth
  and its own handling of anti-aliasing and depth of field.
- **Inverting the tonemapper in the material**: the approximate inverse brought the sampled
  colours no closer to the specified ones, and an exact inverse of the tonemapper would write
  white at an HDR value that bloom turns into a halo.
- **A pool of tags handed to the players in range**: it kept only a mesh handle whose contents
  were replaced anyway, and cost a relationship, a release observer and the loss of a bubble
  whenever its sender's tag was released.
- **Placing, scaling and fading each tag on the CPU**: three writes per tag per frame, and a tag
  one frame behind a player who moved after their transform was last propagated.
- **Keeping the scene and raising its tick or texture size**: neither removes the single point of
  failure, and the texture atlas is bounded by the device's texture size.
- **Smoothing the anchor's height**: the previous quarter-second hold kept a tag above an avatar's
  highest recent pose, and lagged behind it when the avatar sat or crouched.
- **Keeping a message past its five seconds for a sender whose tag was hidden**: it would appear
  late over a player who has just come into range.
- **A console command as the bridge's way to replace a bubble's text**: untyped, and it would
  freeze a debug command's name and arguments into a contract. `/chat_bubble_text` stays for
  tests only.

## Consequences

- Nametags no longer depend on the bridge scene loading or running, and a departed player's tag is
  freed in the frame the player despawns.
- Changing how a tag looks needs an engine build; a scene deploy no longer changes it.
- Tags pass through tonemapping, bloom and anti-aliasing with the rest of the 3D pass. Their
  colours come out darker than specified: sampled from a frame of the web build (Chrome on an
  Apple M1 Pro, 2026-11-16), the `#161518` fill is about `#040506`, the `#43404A` border about
  `#16171D` and the `#FCFCFC` message text `#C9C9C9`, a contrast of about 12:1. A build with the
  approximate inverse sampled `#020202` and `#C9C9C9` for the same fill and text.
- Tags sort among the other transparent objects by distance, so hair, glass or particles nearer
  the camera draw over a tag.
- Each tag is one draw and one mesh; the count follows the avatar cap (20 by default, up to 100).
- A blocked or scene-banned player's name is hidden. Their avatar still shows.
- A scene item attached to the name-tag attach point now follows the avatar's bones every frame.
- A player's name colour is derived from their name by one hash, shared with the HUD
  (`library/explorer-core/engine/nameColor.ts`, same test vectors). The loading ghost and the
  point-at marker use it too.
- `SetCommunityChannels` carries each community's name beside its id, so a community bubble can
  show it. The HUD sends the set again when its ids change, so a community renamed meanwhile
  keeps its old name on bubbles until then.
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

- Fill `OfficialWallets` with the profile data the dcl-one services send: the social service
  already marks official wallets on community messages.
- Confirm a claimed name against the catalyst before drawing the verified seal.
- A fallback font for CJK names.
- Measure a crowd of twenty or more players before and after, and on a desktop build; decide
  from it whether one draw per tag needs batching.
