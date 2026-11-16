# Oddities

Things that look wrong and aren't, or that work differently from what a reader who knows bevy,
Unity or the SDK would expect. Several of these have been "fixed" by well-meaning agents more
than once. Check here before correcting something surprising.

## Rendering

- **The sky renders one 64th of its pixels per frame.** The atmosphere compute pass
  dispatches `resolution / 8 / 8` workgroups per axis across the six cubemap faces, so the
  whole sky refreshes progressively over 64 frames. The sky changes slowly and this keeps the
  pass cheap. The repeated mistake is to assume the pass renders the full cubemap each frame
  and "optimise" by running it less often, which only starves the progressive update and
  makes the sky lag. The pass must run every frame.
- **Our bevy is not stock bevy.** The fork page has the detail; the differences a reader will
  trip over are: the engine runs on web workers with a multi-threaded task pool on web; the
  transparent pass has a second attachment (the focus target) when depth of field is on, and
  custom transparent shaders must write it; render assets upload under a byte budget with
  priorities and serve the previous GPU copy while a re-upload is pending; extended materials
  render a fallback base until ready; pipeline compilation can be asynchronous with a handler;
  glTF URIs go through a host resolver; light textures, `Propagate<C>`, per-side border colours
  and large `children!` exist in 0.16 here though upstream added them later; and two
  performance commits (render-instance mirrors, previous-frame `ViewVisibility`) are throwaway
  adaptations of later upstream work.
- **bevy 0.16 ignores `Changed<MeshTag>`** in GPU mesh extraction. A tag that changes on its own
  stays stale in the mesh uniform. After writing the tag, call `set_changed` on the `Mesh3d`
  so the entity re-extracts. The particle system does this every frame. An entity whose
  `GlobalTransform` changes that frame re-extracts anyway and carries its tag along, so it
  skips the call, which would also specialize its pipeline again; nametags rely on this.
- **Nametags are drawn by the engine**, not by the system UI scene: `crates/avatar/src/nametag`
  gives each player and scene avatar in range a pooled tag entity with one mesh, drawn by one shared material
  in the main pass, facing the camera and scaled to keep its size on screen. A tag is a
  transparent mesh that tests depth, so it is hidden behind walls and still passes through
  tonemapping and bloom. A player's tag comes from their profile. A scene `AvatarShape` avatar
  shows its `name` (`NPC` when the scene sets none, no tag for an empty one), the last four
  characters of its `id` as the suffix (`#0000` for a shorter id) and the voice bars while
  `talking` is set; it appears once the avatar is built. The HUD's preview avatars and a scene
  entity that only restyles a player get none, and avatar modifier areas hide players' tags
  only.
- **The imposter store is multi-realm by design.** The fetch URL carries the realm's about
  URL, and each tile's CRC is derived from the scene set at bake time. A CRC mismatch after a
  publish falls back to an empty imposter on purpose; that is staleness handling, not silent
  breakage.

## Collision and movement

- **`cast_ray_nearest` for single-point queries, not `cast_avatar_nearest`.** The avatar cast
  sweeps the player's capsule with `skip_inside`, which discards any collider the capsule
  already overlaps, so a foot probe on flat ground returns nothing. Reserve the capsule cast
  for character motion.
- **Capsule-cast contact normals come from the capsule surface, not the world face.** A sweep
  grazing a step corner or running parallel past a wall reports a near-horizontal normal at
  zero distance, so a graze reads as unwalkable or as zero headroom.

## Scenes and the SDK

- **A scene can override a player's avatar.** Writing `PbAvatarShape` to a player's scene
  entity id replaces that player's rendered shape, body, wearables and triggered emote while
  they are in the scene, and the emote broadcasts over comms. There is no reserved-id guard
  and this is treated as a capability, not a hole. Don't add a guard without asking.
- **Bare base-emote names** (`"robot"`) in `AvatarShape.expressionTriggerId` must try the
  content map before parsing as an emote URN, because `EmoteUrn::new("models/foo.glb")`
  succeeds (zero colons gets the base-collection prefix), so URN-first swallows every
  scene-file emote.
- **Entity-targeted pointer results are stricter than Unity's.** Bevy writes an
  entity-targeted result only for a declared matching entry (event type, button, distance).
  Unity writes every button transition to a hovered entity with any `PointerEvents`, gated by
  the last entry's distance. A scene that registers only `PET_DOWN` and polls `PET_UP` on the
  entity works on Unity and not here. Unity's behaviour is considered wrong; the scene fix is
  to declare the entry.
- **`restricted_actions.proto` is a dead mirror.** The RestrictedActions interface is
  hand-wired JS plus ops, not protobuf-decoded; editing that proto here changes nothing.
- **Avatar-attach transforms are written back to the scene** every frame, Unity-style:
  rotation player-relative, position an unrotated world-axis delta. The SDK's
  `getWorldTransform` special-cases attach, and scenes in the wild invert Unity's quirk by hand,
  so bug-for-bug parity is the only convention that doesn't break them.
- **Scene UI pointer target survives a drag across other nodes.** A held node stays
  `Pressed` and never re-emits `HoverEnter`, so on exit of the current target the engine falls
  back to the highest-stamped node still hovered (`UiHoverTime`) rather than `None`.

## Unity, for comparison

- Unity alpha-blends MASK wearables after the cutoff; bevy stays spec-correct. See decisions.
- Unity never parses `KHR_materials_emissive_strength`; bevy honours it.
- Unity can leave a player's avatar hidden after a scene removes its `AvatarModifierArea`
  (a cleanup race latches a global flag). Not our bug, but it explains reports that "bevy
  shows the avatar and Unity doesn't".
