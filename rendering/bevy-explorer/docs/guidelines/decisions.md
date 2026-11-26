# Decisions

Things chosen on purpose in a way that looks like an oversight, or that deliberately differ from
the Unity client. Check here before proposing a change in one of these areas. Each entry says
what was decided and why; the PR or issue has the detail. Add to this page whenever a review
ends in "we don't do that", so the next person doesn't have to rediscover it.

## Deliberate divergences from Unity

- **Toon MASK wearables.** Unity alpha-blends after the cutoff, so thin masked lines look
  fine there and fat and black here. Bevy stays glTF-spec-correct (MASK is binary). We don't
  match.
- **Toon avatars are fogged.** Like everything else; Unity's toon computes a fog factor its
  fragment never applies.
- **Toon avatars take only the sun.** No point or spot light reaches them, as in Unity, whose
  toon additional-light loop is commented out.
- **The toon ambient floor is the sky envmap's irradiance.** It stands in for Unity's
  spherical harmonics, read up and down, and without the world's dielectric factor, as Unity reads
  `ShadeSH9` raw. Avatars in shade sit about 4% above a white world surface; that matches Unity
  and is not to be removed.
- **Without the sky envmap, toon avatars take the view's flat ambient.** Such a view (the
  Backpack preview, a photo booth) has no Trilight ambient. The Backpack is lit flat; a booth
  carries a one-light-unit key light, or its avatars would sit at the night floor.
- **Face features are lit by the ambient alone.** As Unity's facial shader: no sun and no bands,
  so they are brighter than the body at noon and darker at night. A view without the sky envmap
  takes the noon horizon colour, since its flat ambient would light them twentyfold. They keep
  the body's outline, which Unity's face shader does not draw.
- **The toon outline is a screen-space line, not an inverted hull.** It keeps Unity's colour,
  width falloff and 8 m cutoff. Kept differences: the cutoff is per mesh, not per avatar; an
  overlap shallower than the edge threshold gets no line, so a curved surface draws none on
  itself; creases inside the silhouette darken, which the look relies on; an orthographic
  view (the Backpack) skips the cutoff, in place of Unity's preview flag; an alpha-blended
  mesh draws no line, since the line reads the depth and normal prepass and a blended mesh is not
  in it (it would trace the edges of the scene behind); and a single-sided mesh seen from its
  back draws nothing, where Unity's hull, made of back faces, fills its shape with the outline
  colour (the Epic Arrival Shield's particles, seen from the front). As with Unity's hull, the
  line never covers a mesh's emission, so a glowing mesh glows to its edge.
- **A hovered avatar glows through the shared highlight pass.** Unity's colour, opacity, width
  and 0.1 s fade, through the pass scene entities use, so the glow shows through what stands in
  front and ignores the camera dither and the ghost reveal. Kept differences: every opaque or
  alpha-tested mesh under the player glows, an emote's props included, where Unity lists the
  wearables only; an avatar shown with `ShowOnlyWearables` glows like any other.
- **Day and night (#1467).** The time of the scene the player stands in holds the sky: over
  the running clock, over a time set from the HUD and over `/time`, which still move the clock
  and show once no scene sets the time. The HUD's Skybox menu is locked meanwhile and says
  why. The "Allow overriding scene controls" setting (off by default) unlocks it: the scene's
  time still takes the sky when it arrives, a time the player stops the clock at after that
  wins, and the scene's time is back when it arrives again (on return, in another scene, or
  when the scene changes it) or when the clock runs again, the menu's Auto included. No
  override is kept for a scene. `~system/Runtime.getWorldTime` tells every scene, next to the
  time, whether a scene holds it, the clock's speed and whether the clock sets the sky, so
  that the menu and a scene that shows the time of day read one source. Two exceptions.
  `/time --force` beats a
  scene's time whatever the setting, until the next `/time` that sets a time or a speed: the
  scene editor lights a stopped scene with it. A portable's time comes after the scene's, wins
  only over a running clock and never locks the menu: a portable follows the player
  everywhere, so the player's own stopped time beats it.
  `transitionMode` in scene.json is ignored. HUD and `/time` skips take the nearest
  direction; SDK `SkyboxTime` keeps its own mode. `/time` reports the effective time.
- **Scene environment components.** Skybox cloud speed accepts finite negative values for
  reverse rotation, as the public protocol specifies. Unity's environment profile at
  `c78b1f0c59545b0d3b1ff43e109fa63997267850` clamps negative speed to zero. Missing overrides
  retain this renderer's time-of-day atmosphere, lighting and quality settings; component
  compatibility does not imply identical default pixels across renderers. A sky image changes
  the visible sky while the authored color bands continue to drive ambient lighting: an
  authored zenith, horizon or nadir replaces the sky, equator or ground colour of the Trilight
  ambient, and the sky preset keeps the bands a scene leaves out. A scene's fog density is in the
  sky preset's units, so it goes through the same conversion to bevy's falloff, and it takes the
  place of the `/fog` density while the scene is active.
- **Scene colour gradients are linear (#1293).** Sun, sky, fog and cloud gradients are read as
  linear colours, as `gradient.proto` specifies for `ColorKey.color`, with values above 1
  allowed. A Sun key above 1 is scaled back to 1 for the hue, and the light is multiplied by its
  largest channel. Unity's environment profile at `0a8bafca9e62ced7f34ac182158b11edd0c842a4`
  puts the Sun gradient straight into the light colour, which it decodes as sRGB: the same key
  gives a darker, more saturated hue there, and a key of (2, 2, 2) lights the scene about 4.6
  times rather than 2. We follow the protocol.
- **A root light source replaces what it sets.** Point and spot light intensities are scaled to
  the sky preset's light unit (the default 16000 is 8 units). A `LightSource` on a scene's root
  entity replaces the sun's colour and, when it gives an intensity, the sun's intensity as lux,
  unscaled: the Unity client creates no light for a light source without a type, so there is no
  unit to match, and scenes already set it in lux here. Without an intensity the sky preset's
  light stays: the lamps' default of 16000 is not applied to the sun, where it is over four
  times the preset's noon.
- **The light sets with the sun.** From sunset until the swing to the moon, and from the swing
  back until sunrise, the directional light stays on the sun's arc below the horizon while the
  intensity curve fades it, as in Unity. It grazes walls from under the horizon and floors get
  no direct light; at most a tenth of a light unit points upward. It is not clamped to the
  horizon.
- **Emissive strength.** Unity never parses `KHR_materials_emissive_strength`; bevy honours
  it, spec-correctly, on scene glTFs. Content authored against Unity can glow (the Nekko cat at
  65x). Dropping the extension is not the fix for scenes; the long-term answer is still open. A
  glTF emissive factor of 1 draws five times as bright as a lit white surface, and the strength
  multiplies that. Wearables are the exception: an avatar drops the strength, as Unity does, and
  draws a factor of 1 at 12.5 times a lit white surface, so a wearable glows as its creator saw
  it in Unity.
- **A Material that names a glTF keeps the glTF's rules.** An SDK unlit material is opaque
  unless it has an alpha texture, whose brightness scales the alpha, and an SDK PBR emissive
  colour is drawn as it is, as in Unity. A glTF's own unlit material cuts out or blends through
  its texture's alpha, and its emissive factor is tinted by its base colour texture. The
  material the engine writes to a scene for a glTF has to draw the same once the scene sends it
  back with one field changed, and the SDK fields cannot say either thing. So a Material with a
  `gltf` reference reads `alpha_test`, the diffuse alpha and the emissive by the glTF's rules;
  one without it, which includes `GltfNodeModifiers`, reads them by the SDK's. The imposter
  bake applies the same alpha-texture and emissive rules, so a scene keeps its look when its
  imposter takes over.
  A normal map follows the texture, not the reference: an SDK `bumpTexture` has its green
  channel pointing down the image and a glTF's normal map up, so the glTF's own normal map
  keeps its orientation when it comes back, and a bump texture the scene puts in its place is
  read as the SDK's.
- **Bloom is off or on, and scatters at 0.5.** Unity's quality presets name 0.3 (Low, where
  bloom is off), 0.5 (Medium) and 0.6 (High). At 0.3 the glow around a bright surface here is
  about half as strong as Unity draws it, and 0.5 matches its captures (`BLOOM_SCATTER`). The
  setting has one level: a saved config that names an earlier one (Low or High) reads as On.
- **Alpha test and blend premultiplies the whole base colour.** The alpha multiplies the base
  colour before lighting, and a metal's reflectance is its base colour, so a transparent metal's
  reflections thin with it where Unity keeps them. Keeping them needs the diffuse and specular
  terms split inside the lighting functions.
- **Sky preset curves are linear.** The fog colour, light colour, light intensity and Trilight
  ambient keys are interpolated linearly; Unity's curve tangents are not carried.
- **The sky below the horizon is the horizon colour.** The sky is Unity's lookup by day
  phase and elevation (`stylized_sky.wgsl`), and as in Unity the elevation clamps at the
  horizon, so the sky holds the horizon colour from a few degrees below the horizon down; the
  horizon noise can lift directions just under it (direction y above about -0.1), as in Unity.
  The sky does not fade to black there. A scene's authored sky colours blend over that colour,
  not over black. The reflection map is unaffected: it reads directions under the horizon from
  their mirror image above it unless a scene authors a nadir. One difference from Unity: the
  horizon noise is sampled at its full resolution: the image has no mip chain, and the cube's
  compute shader has no screen derivatives to choose a mip from.
- **The screen draws the sky per pixel; the cube serves the rest.** As Unity's skybox, the
  sky material (`StylizedSkyMaterial`) evaluates the sky for every pixel, so the cloud strips
  and the disc keep their detail; it replaces bevy_atmosphere's skybox material, which shows the
  cube, on every skybox. The compute model draws the same sky into the 1024 cube, one texel in
  64 a frame, for the reflections, the ambient and the stars' cloud cover: its refresh smears
  scrolling cloud and its texels soften the disc's edge, which those uses do not show. The
  strips ship at 1024 by 128, a quarter of their authored 4096 by 512 (about 2.8
  texels a degree around the sky, against 11): the wasm build resizes any image over 1024 to fit
  (`reduce_image_sizes`), so larger files would only add to the download and to startup, and
  the clouds look right at that size on web. Native uses the same files; it is not released, so
  its sharper strips are not worth a second set.
- **The stylized sky is the sky for every scene, `PBSkybox` ones included.** Unity switches a
  sky that a scene controls (any `PBSun`, `PBSkybox` or fog) to its Legacy look; here the
  scene's values override the StylizedV1 lookup instead, and a scene with none sees the preset.
  Each `PBSkybox` field maps as follows:
  - `zenith`, `horizon`, `nadir`, `rim` replace the lookup's colour in their band, with
    pole, horizon and rim weights; the rim follows the horizon when unset, and an authored nadir
    colours the lower sky.
  - `clouds.color` is the lit colour of the three cloud strips. The shadowed colour is that
    colour times one number: the luminance (Rec. 709) of the preset's shadow over that of its
    lit colour at that phase, at most 1. The strips keep the preset's contrast and the authored
    hue, and a shadow is never brighter than the lit colour; the preset's own shadow tint
    (blue at sunrise) is not carried over.
  - `clouds.opacity` scales every strip.
  - `clouds.speed`, in radians per second around the sky, replaces the three preset speeds:
    one speed turns every strip together, backwards when negative, and the strips carry on from
    where they were.
  - `stars.brightness` scales the stars (see "Stars are added over the sky").
  - `PBSun.visible = false` hides the sun and the moon; the stars stay.
  - `skybox_texture` shows the panorama instead, with no clouds or stars.

  Unity's environment profile at `0a8bafca9e62ced7f34ac182158b11edd0c842a4` writes the cloud
  values to the old cloud cubemap's material, which the strips do not read. The strips are
  fixed art sampled at their full resolution: they have no mip chain. That loses nothing at
  1024 wide: the screen magnifies the strips, even about 70 degrees up, as far as the overhead
  strip is seen (about 8 texels a degree of arc there). The horizon noise is still minified,
  about 8 times in V, with no mips; Unity samples it with gradients and mips, so it aliases a
  little more here. The growth flow the preset switches off is not
  ported. `/cloud <cover> [speed]` is the user's own setting on top: cover multiplies every
  strip's opacity (the scene's opacity times it), speed scales the strips' scroll and the horizon
  noise's drift, so `/cloud 0 0` clears the strips and holds them and the horizon noise in a
  fixed pose (the shader time is 0 and every scroll 0). A non-finite value is refused, and cover
  is held to 0 to 1. The shader time wraps at 500 s, one repeat of the horizon noise at the
  preset's speed. A scene's values still apply while it is active; the star twinkle ignores the
  speed.
- **A scene's sun direction draws the sun's whole disc.** Unity switches a sky that a scene
  controls (any `PBSun`, `PBSkybox` or fog) to its Legacy look, which computes no celestial
  path. Here a scene-controlled sky stays on StylizedV1. A scene that fixes the sun's direction
  gets the sun's disc, the sun's colour and the halo at that direction at every hour: no moon
  crescent, where Unity's Legacy look draws a sun-coloured crescent there at midnight, and no
  Legacy sun opacity curve. The disc still dips out through the two windows in which the light
  swings between the sun's arc and the moon's.
  A scene's `PBSun.color` tints the disc: the authored hue at the brightness the disc has
  without it (the sun's ramp at the phase, or the moon's along its rise to set), times the
  authored colour's brightest channel when that is above 1. Unity feeds the colour raw, but the
  preset's disc is HDR (about 10 at noon), so a raw white would dim the sun about tenfold; a
  black colour gives a black disc.
- **Stars are added over the sky, not drawn under it.** The stars are Unity's Stars v2 and
  shooting stars (`SkyLut_Stars`), with the preset's values. Unity draws them inside its sky
  shader, so the clouds and the disc cover them. Here they stay a pass of their own at screen
  resolution, added over the frame wherever nothing was drawn and faded by the sky cube's
  alpha, its cloud cover: the disc and its halo do not hide a star behind them. The dim patches
  read the horizon noise with Unity's seam-safe derivatives, but the image has no mip chain
  here, so every sample is its top level. A scene's `PBSkybox` star brightness scales the
  preset's stars from the protocol's default of 4.62; below that default it thins the shooting
  stars by the same factor, down to none at 0, and above it their rate stays the preset's. A
  scene that shows a panorama gets neither. Unity gives a scene-controlled sky its Legacy look,
  which has no Stars v2.
- **Render scale compositor.** At scale 1 the world camera draws to the window and carries the
  UI, at no extra cost. Below it, the world renders to an image that a 2D camera composites onto
  the window with FSR EASU; bevy's CAS at 0.92 stands in for RCAS; the UI rides the compositor
  at full resolution; `/screenshot` captures the window. The primary camera always keeps the
  depth prepass (the jump marker reads it); the normal prepass, and both on texture cameras, only
  while the avatar outline is on or the camera has SSAO (native only).
- **Render Resolution stops at 100%.** Unity's Resolution Scale moves in the same 10% steps
  but reaches 120%. The compositor only upscales, so the range is 50–100%. A saved value
  between steps or outside that range moves to the nearest step in it once, when the saved
  settings are migrated, so the slider shows what is rendered.
- **Sun shadows keep bevy's default bias.** Unity's `DepthBias` (1.75 or 1.56 texels, times
  URP's soft kernel radius 2.5) is a depth bias, not a normal bias, and carried over as bevy's
  normal bias it offset each lookup about half a metre on Low at 60 m, dropping thin and contact
  shadows. On the 1024 map, acne showed below a normal bias of 0.5 on Low and about 1 on High
  (Genesis Plaza, low sun), so both qualities keep bevy's 0.02 m depth and 1.8 texel normal
  bias.
- **Sun shadow map is 2048, and Low's one cascade stops at 40 m.** Unity's main shadow
  resolution is 1024. High's four cascades each cover a slice of the shadow distance: at 100 m
  and 200 m, 2048 per cascade looked the same as 4096 and the frame took about 10 ms less (web,
  Apple M1, Genesis Plaza); 1024 saved 2 ms more and softened the contact shadows. Low's single
  cascade spreads its map over the whole shadow distance, and the bias is in texels. From 20 m
  to 60 m, 2048 looked the same as 4096 with the sun at 10:00 and at 15:00, and at 20 m took
  about 6 ms less. At 100 m with the sun at 15:00, the shadows of a crowd 5 to 10 m from the
  camera were mostly gone on 2048 and all gone on 1024. With a low sun, past about 75 m the
  avatar's shadow was a smudge at the feet on 2048 and gone on 1024. 4096 kept them, but no
  preset reaches that far on Low, and a size that follows the setting reallocates the map as
  the distance crosses the line. So the map is 2048 for every setting and is never resized,
  and Low's cascade stops at 40 m (`SUN_LOW_MAX_DISTANCE`) whatever the shadow distance; past
  it the far shadow floor (next entry) applies, and High reaches further. Spot-light shadows
  and the lobby stage light share the sun's shadow texture array, so they are drawn at the
  same size.
- **Sun shadows hold a floor past the shadow distance.** URP fades the shadow out completely at
  the shadow distance. We fade over the same squared-distance curve, but towards a partial
  shadow (`CASCADE_FAR_SHADOW` in `shadow_softening.wgsl`) that holds past the distance, so the
  worst-case change between near and far is smaller. Fading to no shadow looked worse about
  half the time.
- **Low shadows keep hardware 2x2 filtering.** Unity's soft shadows filter every quality. Low is
  the Medium preset's shadow quality and the filter applies per view, spot shadows included, so
  Low keeps bevy's `Hardware2x2` (one PCF sample) instead of `Gaussian` (nine) to stay cheap on
  weak GPUs and web. High uses `Gaussian`.
- **Jump marker reads the depth prepass.** Like Unity's projector, the blob (`BlobShadow.mat`)
  is projected onto the surface below, so it follows slopes, steps and ledges; a flat quad
  cut into rising ground. A depth bias does not fix that: it offsets the quad's depth test,
  so a bias that clears a metre of slope also draws it over anything that close, and the
  blob stays flat rather than lying on the ground. Bevy's own decals do not fit either:
  clustered decals are true box projectors but need bindless textures, so they do not run
  on WebGPU or macOS, and `ForwardDecal` is a flat quad with a depth fade, so it has the
  same slope limits in a softer form. The projection reads the primary camera's
  depth prepass, which the camera therefore keeps; without one the marker draws nothing.
  Keeping it costs nothing new: the camera already renders it on native and web, and it
  saves the main pass shading hidden opaque pixels. It also enables later work: decals,
  soft edges where transparent surfaces meet geometry, and GPU occlusion culling.
  The box runs from 0.1 m under the feet, so the blob never darkens the avatar, to 1 m
  below the ground under the player's centre, so ground falling away by up to 45 degrees
  across the footprint still catches it. The ramps follow the shadergraph, except that
  grounded means within 0.2 m of the ground, as for the animation, and the opacity rises
  over the next 0.2 m instead of the last metre: with the projector's 0.5 fade factor,
  Unity's ramp left the blob nearly invisible before the player landed.
- **Scene and download priority without distance buckets.** unity-explorer ranks scenes and
  asset loads by distance buckets and a bucket-indexed tick-rate table. We keep the aging
  scheduler and measure raw squared distance from the player to the nearest point of a scene's
  nearest parcel (not its corner); a scene behind the rendering camera counts four times
  farther in the tick queue (no once-a-second cap, no frozen far scenes). Request slots go to
  the head of the best-ranked queue (one per scene, wearable or emote hash, and one for requests
  with no hash): the player's own wearables and the emotes they play or have equipped, then the
  player's scenes,
  then the other avatars' wearables and emotes, then the
  other scenes, the last two by parcel ring, in front before behind, then raw distance; then
  requests no scene or avatar owns (no hash, or a hash nobody holds), oldest first; last, the
  downloads of a scene that has left.
  Scene roots and avatars register their hashes through component hooks; the publisher writes
  the values in place. See the
  [ADR](../../../../docs/adr/2026-11-02-explorer-nearest-first-priority.md).
  The same key orders scene spawns, which come in loading waves of at most six
  ([ADR](../../../../docs/adr/2026-11-09-explorer-scene-streaming.md)).
- **Voice chat (private calls and community streams)** keeps the call statuses and guards, the
  3 × 2000 ms re-dial budget, the 1 s hang-up-before-join delay and the server's 60 s ring.
  - The `Microphone` key is `T`, Unity's `Talk` key. The chat opens with `Enter`, so the chat
    panel has no default key. The `Microphone` key drives the call mic while a call is connected
    and the nearby mic otherwise, as Unity's `T` does. Nearby, a press opens the mic and its
    release closes it. In a call, a press shorter than 0.5 s leaves the mic open and a longer one
    is push-to-talk. A press while the mic is open closes it on release. A community listener's
    `T` does nothing, since a listener can't speak.
  - Don't start a call's mic from a stored setting. It goes up on the call's first connect for
    whoever may speak (both sides of a private call, a stream's speakers). Don't turn it back on
    after a re-dial: what the user chose since then stands.
  - Don't keep a muted mic's track published. Muting unpublishes it, so no audio leaves a muted
    user. In a stream `MuteSpeakerFromCommunityVoiceChat` is still sent, so every client shows the
    same state (a focus pause is the exception, below).
  - Don't play calls at the nearby-voice volume. Calls and streams follow `Voice Chat Volume` and
    nearby players `Voice Volume`, both scaled by `Master`. Capture uses the system's default
    microphone; a device picker is next work.
  - Don't loop the call tune in the audio layer, which has no loop: the one-shot clip is
    re-triggered every 3.056 s (its length) while ringing.
  - Don't auto-subscribe the voice room. Like every room it joins with `auto_subscribe: false`,
    and the voice module subscribes to each microphone track itself.
  - Don't also play someone the user shares a voice room with (a call or a stream) through
    nearby voice: their spatial track is unsubscribed while they share it.
  - Don't skip the re-dial when the user is alone in the room. A stream's creator before anyone
    joins, or a caller whose partner is still connecting, would lose the call to a network blip.
    A call that has really ended has its room deleted, and that disconnect ends the call without
    a re-dial.
  - Don't count the user's own call room for the community chat's "Live Voice Stream" banner: it
    shows the backend's participant count for the stream (1 for a stream known only from its
    start update, until the next read). Don't let it catch clicks while the chat is unfocused: it
    is hidden, not just transparent.
  - Don't wait for an update about the user's own stream. Expected: once the user's stream starts
    it shows as live (rail icon, chat banner, sidebar badge, scene banner) with current counts.
    Actual: the social service leaves the starter out of the stream's STARTED update, and neither
    update carries counts; ENDED reaches everyone, the ender included. So the bridge re-reads the
    live list when the user's own stream is entered or left.
  - Don't let a ringing private call carry on next to a stream: joining or starting a stream
    while one rings refuses it first, and the stream starts a second later.
  - Don't leave a stream running with nobody in it: one whose start succeeds after its creator
    left (or moved to another stream) is ended with `EndCommunityVoiceChat`.
  - Don't hold a hung-up private call in an ending state. One whose id is known ends at once,
    and `EndPrivateVoiceChat` is sent behind it. With the social link down the request waits for
    the reconnect (up to 30 s of backoff), and an ending state would keep the nearby mic off and
    refuse new calls all that time. That request is not retried if it fails. A call whose start
    is still in flight at sign-out is not ended by the client either: its reply comes back after
    the account's link is gone, so the end goes out on the next account's link, where the server
    refuses it (only a party ends a call), or on none. The server ends both anyway: leaving the
    room makes the gatekeeper delete a private room, and a closed social socket ends that user's
    pending call (`social-service-ea` `logic/subscriptions/lifecycle.ts`, mirrored by the
    catalyst).
  - Don't ring a call again on reconnect. A call left ringing when the social link dropped is
    rejected on reconnect (one the user placed is ended), and the reconnect's own report of it is
    ignored: the server tells only the caller about a reject, so a second ring would never stop.
  - Don't show the "stream in this scene" banner to guests: they can't join.
- **A scene can turn nearby voice off.** `featureToggles.nearbyVoiceChat` in `scene.json` is on
  when absent, not a string, empty or `enabled` (any case); any other string turns nearby voice off for a player
  standing in that parcel scene. An empty parcel and a portable experience never do.
  - The setting is the scene's request for the `DisableVoice` permission, asked once per visit.
    The permission allows by default; a player who refuses it for the scene, the realm or
    everywhere keeps nearby voice there, and the use shows in the HUD's permissions popover
    like any other. Nothing is turned off while the player has not answered. A question the
    player leaves the scene without answering is withdrawn, and records no use. A teleport
    that lands in the same scene is the same visit.
  - Don't leave the mic open: entering closes it, the `Microphone` key and the HUD cannot open it,
    and leaving does not reopen it. Each refused open counts in `MicState.refusals`, and the HUD
    toasts "Nearby voice chat unavailable in this scene." when the count goes up.
  - Don't play nearby players or show their speaking badge there. Their tracks stay subscribed, so
    hearing returns on the way out.
  - Don't touch calls and community streams: their mic, audio, badges and mic on/off sounds are
    unaffected.
  - Don't offer Speak there: the nearby voice popover still opens, with Speak disabled and the same
    hint, so hearing and volume stay adjustable; the minimap header lists "Nearby voice disabled".
  - A scene room that refuses the player as banned (a platform, world or scene ban, as
    `SceneAdmission` reads the refusal) turns nearby voice off the same way. The room asks again
    every 15 s, since an unban reaches only the players inside it; an admission, or a new scene,
    realm or wallet, lifts it without reopening the mic. A recheck with no answer (a network
    error, a 503) keeps the refusal. The interval bounds both the gatekeeper requests from a
    player who stays banned and the wait before an unban is noticed. Platform bans recheck here
    too: the platform ban-status poll (every 300 s) reads the ban but never admits the room
    again. unity-explorer suppresses on any 401 or 403 from the scene room and lifts it on the
    room's next connect or disconnect; its scene room asks again every 5 s while refused.
- **The mic pauses while the Explorer lacks focus** (the primary window natively,
  `document.hasFocus()` on the web) while the Mute Mic in Background setting (Sounds, on by
  default) is on, so it never captures while the player is in another app or tab: it stops
  capturing and publishing for nearby voice and calls alike, and resumes when focus returns.
  The mic setting does not change, and neither pause nor resume plays the mic on/off sound.
  The HUD shows the paused mic off: the call panel at once, the nearby mic toggle at its next
  mic state poll (up to 1 s).
  - In a community stream no `MuteSpeakerFromCommunityVoiceChat` is sent for the pause, so
    other clients keep showing the paused speaker unmuted while no audio arrives; the mute
    entry above holds only for a mute the player chooses.
  - With the setting off, the mic stays live without focus, for nearby voice and calls alike.
  - unity-explorer's setting of the same name pauses only the nearby mic.
- **Blocked and scene-banned players are not heard nearby.** A player blocked in either
  direction, or banned by the connected scene room, carries `BlockedOrBanned`, and their nearby
  voice track is unsubscribed while it lasts; unity-explorer keeps the track and creates no audio
  source. Calls and community streams still carry them. Scene bans apply without the
  `alfa-banned-users-from-scene` flag unity-explorer reads, as in Nearby chat.
- **Saved mutes for nearby voice** follow unity-explorer's: the profile menu's Hush and Hear, the
  list kept by the social service (`/v1/mutes`), a mute that holds for the session when saving it
  fails, a saved list that only adds to what the user did before it arrived, and a muted player
  silenced but still subscribed, with the hushed badge in place of the speaking bars. These differ:
  - The list belongs to the account. A change of account drops it and reads the new account's;
    unity-explorer reads it once at start and keeps it for whoever signs in next.
  - `nearby_voice_user_mute` is sent for the user's Hush or Hear only; unity-explorer also sends
    it for every mute read from the saved list.
  - Muting oneself is refused, as the service refuses it; the menu never offers it.
  - Changes are saved one at a time, in the order the user made them, and only once the saved
    list has been read: the service applies each request on its own, so a Hush and a Hear sent
    together could land in either order, and a row removed while the list is read page by page
    makes the next page skip one. unity-explorer sends each change as it is made, cancels the
    one in flight when the next is made, and does not wait for the list.
  - A saved list that cannot be read is read again from the first page after 2, 4 and 8 seconds;
    after that it stays unread until the account changes. unity-explorer repeats a single page
    request up to twice on a transient network error and does not read the list again.
  - Hush and Hear are on the profile card's default and friend menus, for a blocked user too, as
    unity-explorer's user menu has them; the request, blocked-list and community-member menus
    do not carry them.
  - Don't apply it to calls or community streams: as in unity-explorer it is nearby voice only.
- **Profiles from the centralized catalyst.** unity-explorer switches between the
  asset-bundle registry and a catalyst with `USE_CENTRALIZED_PROFILES` and reads a remote
  player's catalyst from their LiveKit metadata. We read the base-domain catalyst's lambdas:
  `POST /lambdas/profiles` for full profiles and `POST /lambdas/profiles/metadata` for the compact
  tier, which falls back to full where the route is missing, as unity-explorer forces full on a
  catalyst. No flag, and no catalyst named by another player. For now, when that catalyst is on
  `decentraland.org` or `decentraland.zone`, whose catalysts lack the compact route, compact reads
  go to that deployment's asset-bundle registry instead. This path is to be removed once those
  catalysts serve the route. See the
  [ADR](../../../../docs/adr/2026-11-04-explorer-profiles-from-the-catalyst.md).
- **Overlapping profile saves (#1453).** The engine applies editor patches in arrival order.
  Each upload carries the edits accepted before it starts, and those callers share its result.
  Later edits build on the last confirmed profile. The local avatar previews them immediately,
  while peers and the player's Passport read the confirmed version. [Profile ADR](../../../../docs/adr/2026-11-04-explorer-profiles-from-the-catalyst.md)
- **Own profile versions never repeat.** unity-explorer's `SelfProfile` numbers an edit one above
  the profile it holds, so after a failed publish puts the old profile back, the next edit reuses
  the failed version. A failed deploy may still have reached the server, and a peer that read it
  skips any other profile under that number, so our next edit goes above every version this session
  gave the account (`CurrentUserProfile::bump_version`).
- **Community chat (#223).** A community packet is taken only from the server's message
  router, which checks membership and stamps the author; unity-explorer also accepts one
  straight from a peer under the peer's own identity. Community history is stored like a DM's
  and trimmed to 200 messages and 1000 records per community; unity-explorer keeps it in memory
  and persists DMs only. A community tab's input is blocked while the private chat room is not
  joined or the router is not in it, as a DM's is for an unreachable partner. Leaving a
  community (or being removed) deletes its stored history and settings, and the communities an
  account had chats for are remembered per account so a leave while logged out does the same.
  The notification ping has a default in the Chat settings tab and a per-conversation override
  with a "Default" choice; a flood of lines on one conversation is one sound (400 ms). The
  member-of list is read up to 10 pages of 100 (unity-explorer reads one), and one pair of clips
  serves focused and unfocused channels alike, played by the bridge scene from its own content.
- **Blocked users in chat history (#296).** Live chat and reactions from anyone blocked in
  either direction are dropped on every channel. Community history still stores them and
  leaves them out of each read while the block lasts, so an unblock brings them back;
  unity-explorer stores no community history. A new DM or DM reaction from them is not stored,
  since a DM history is never trimmed. A DM thread stored before the block is read back whole:
  hiding it would reveal that the other side blocked you, who is otherwise shown as offline.
- **Situational reactions.** An emoji a player reacts with travels as unity-explorer's rfc4
  `Reaction` and is sent, limited and played one at a time as unity-explorer does, with these
  differences. Only the indices that name an emoji are taken (unity-explorer takes any under 4096
  and draws the blank tiles), and an emoji with no index is not sent or shown. A message's copy
  by the other room is told by its stamp and its index together, and the stamps sent only move
  on: the sender's uptime as a float stops telling two messages of one send apart after a few
  hours. What a sender may send is counted on the sender's own entity, so a player who leaves or
  is blocked takes their waiting reactions with them; unity-explorer plays them out. A sender's
  reactions play 0.2 s apart whichever messages they came in; unity-explorer starts the gap
  afresh for a message that finds none of theirs waiting, which lets a sender of one-reaction
  messages play twenty a second.
- **Community creation and editing.** The form follows unity-explorer's (fields, limits, texts,
  the content check's two refusal screens, every HTTP 400 read as the check's answer), with
  these differences. CANCEL on the refusal screen asks "Discard this community?" first, as the
  form's own CANCEL does; unity-explorer closes at once. A refused edit says "Your Community
  Couldn't Be Updated"; unity-explorer says "Created" for edits too. A failed delete shows its
  message on the community card, like the card's other actions, instead of as a toast. A
  picture saved by an edit is kept by the HUD for the session, as unity-explorer's image cache
  does, since its URL does not change and the CDN (CloudFront on .org) ignores a version query.
  The places picker links the LANDs and Worlds the user owns or operates, as unity-explorer's does.
  An edit sends `placeIds` only when the linked places changed, and the service keeps them when the
  field is absent; unity-explorer sends them on every save. When the community's linked places
  could not be read, the picker and its remove buttons are disabled, so a save cannot replace them
  with a partial list; unity-explorer leaves them editable. Linked places the places service does
  not return (a disabled one, say) are kept on every save without being shown, and the community's
  places are read a page at a time; unity-explorer reads one page and would unlink the rest. As in
  unity-explorer, the picker also lists LANDs the user only operates, which the service refuses to
  link: the save fails with the generic error. That a moderator can unlink only the places they own
  is a HUD rule, as in unity-explorer; the service checks ownership only of places being added.
  On the card's Places tab, owners and moderators get unity-explorer's ADD PLACE tile (it opens the
  edit form) and delete on the places they own, asked first. A failed delete shows its message on
  the card, like the card's other actions, instead of as a toast. unity-explorer's variant of the
  PLACES tab button for owners and moderators is not ported.
  On the card's Members tab, a row opens the member's passport and carries unity-explorer's
  friendship buttons; its ⋮ opens unity-explorer's member menu: View Profile, Chat, Call, the
  owner's and moderators' actions (Demote or Promote Moderator, Make Community Owner, Remove
  Member, Ban) with unity-explorer's rules and confirmations, then Block and Report, then Invite
  to Community. Gift sits after Call, greyed for a blocked member (see the gifting entries below). Chat is
  always offered in-world: unity-explorer greys it
  only while its chat area is down. unity-explorer's row shows no presence, so neither does
  this one; "N Mutual Friends" is not shown because no members API returns the count. A failed
  action shows its message on the card instead of as a toast, and the tab lists the first 100
  members without unity-explorer's paging.
  The Communities page's browse grid is unity-explorer's: 20 communities a page, the next read as
  the grid's end comes into view, the name searched on the server a second after typing stops (at
  once on Enter), and each card's Public or Private, Join, Request to Join or Request Sent (Cancel
  Request on hover, on the card too), its description on hover, and the first three of the user's
  friends in it. A sent or cancelled request shows at once, before the community is read again,
  and a failed join or request from the grid is toasted. Whether a join request is
  pending comes from the user's own join requests, read when the grid's first page loads and when a
  closed community's card opens, and kept from the request's answer, as unity-explorer's. The
  card always shows its member count; unity-explorer shows it only behind its members-counter
  flag. Live streams show as a card's LIVE chip and the grid's Streaming filter, not as
  unity-explorer's separate Voice Streaming section (for an invited card, see Community invites
  below). A failed read shows unity-explorer's error toast
  and leaves the grid empty, with no count, where unity-explorer leaves its spinner running. A
  card shows no spinner while its join or request runs. My Communities is every membership, read
  apart from the grid; when that read fails the list stays as it was and the page asks again the
  next time it opens, without unity-explorer's "error loading My Communities" toast.
- **Community invites and join requests.** As unity-explorer: the browser's Invites & Requests
  opens Invites, Requests Received (only for a user who owns or moderates a community; one group
  per community, each request with ACCEPT, REJECT and the member menu without owner actions) and
  Requests Sent, each titled with its count, and "No pending invites or requests." when all are
  empty. Its counter is the invites plus the requests received, and Requests Received's title
  counts the same totals. Unlike unity-explorer, opening the page reads only the counter (each
  read's total, one row asked for); the section is read in full when it opens, and again after
  any answer or cancel and when an invite or request notification arrives while the page is open
  (the counter alone until the section has been opened). A section that cannot be read says so,
  with RETRY. Only private communities are read for join requests, a
  community's first 100 (the service's page cap); one whose read fails is left out, and the list
  fails only when none could be read. A card the user is invited to offers Accept and Reject
  (asked first) instead of Join in the grid, and ACCEPT INVITE and REJECT INVITE on the community
  card. The card's Members tab gives owners and moderators MEMBERS, BANNED (with UNBAN), JOIN
  REQUESTS (private communities, with its counter) and INVITED (with CANCEL). Invite to Community,
  in the user menus, the member menus and the passport but not the voice panel's, lists the
  communities the user may invite the person to, is hidden when there are none, and is read again
  after an invite. It opens beside its row, on the left or upward where there is no room, and from
  the keyboard (a keyboard click on the row focuses its first community, Tab moves through them);
  Cancel closes it before the menu around it. Clicking an invite, a request received or a request accepted notification opens the
  community's card, and every opening of a card reads its community again, so an invite or an
  accepted request that arrived since shows. An accepted request also reads My Communities, the
  grid and that open card again. Call in the member menu closes the card first, as Chat does.
  Answers and an unban are toasted when they fail. A request takes one answer at a time, released
  when that answer settles, and a request answered here never comes back from an older read.
  Differences: a Members section that cannot be read says so, with RETRY (unity-explorer keeps
  what it showed and tries again on the next read, so a first read that fails looks empty); each section reads its first 100 rows without paging, as the Members list does;
  the section shows a spinner, not unity-explorer's skeleton cards, while it loads; and a row's
  buttons are disabled, with no spinner, while its answer runs.
- **Community announcements.** As unity-explorer: the card's ANNOUNCEMENTS tab reads 10 a page,
  the next as the list's end comes into view, newest first. Owners and moderators get the composer
  first ("Any Announcement to share with your Community?", 500 characters counted as UTF-16 units,
  so most emoji take two, with a counter shown once there is text, the chat's emoji panel and `:` suggestions, a POST button held with a spinner
  while it posts), then a separator. The owner deletes any announcement and an author their own,
  asked first ("Are you sure you want to delete this Announcement?", DELETE and CANCEL); a
  moderator cannot delete another member's. Anyone who sees the list likes and unlikes, one
  request at a time for each announcement, and the count changes once the service answers. The
  author shows a verified mark or their #tag, and the official mark when the post comes marked
  `isOfficial` by the social API. A post or a delete reads the first page again. Every failure is
  toasted with unity-explorer's four texts. A member who sees no announcements reads "No
  Announcements here yet!". A private community's announcements are not read for someone who
  isn't a member (the service refuses them); their tab stays empty. Differences: POST stays off
  for a draft of only whitespace, and the text is posted without leading and trailing whitespace
  (unity-explorer enables POST for any text and sends it as typed; the service refuses a post that
  starts with whitespace); a failed post stops the button's spinner (unity-explorer
  leaves it spinning); and the official mark comes with the post, not from the client: this
  node's social API marks each post (`isOfficial`, a field the Foundation's API doesn't have)
  when its author is in the explorer feature flags' `alfa-official-wallets` list (`wallets`
  variant, first CSV line), which unity-explorer reads itself. Against the Foundation's social
  API, which sends no such field, no post shows the official mark.
- **One answer to "banned or blocked?".** The `ban_block` crate is a facade: it reads the
  social client's block sets and comms' scene admission and scene-room lists through one
  `BanBlock` system parameter and owns only the platform-ban check, where unity-explorer keeps
  `IUserBlockingCache`, `ApplicationBlocklistGuard` and `RoomMetadataCurrentScene` apart. A
  blocked pair is blocked in either direction, as there. The last block sets stand across a
  social reconnect until the next snapshot (unity-explorer also resets only on the next
  `GetBlockingStatus`). The platform ban is read from the gatekeeper's public, unsigned
  `GET /users/{address}/bans` (unity-explorer signs the same request, which the endpoint does
  not need), on sign-in, on a realm change, on `/ban_block_refresh` and, as a periodic chance to
  notice a ban while the client idles, every 300 s (unity-explorer checks at start-up only; no
  push reaches an idle client here, and the period promises no freshness, as a failed check
  fails open, as there, and the state says so instead of guessing; a failed check keeps a ban
  already known, past its `expiresAt` too, since the local clock lifts no ban; once a known ban's
  `expiresAt` passes it is checked again once, after any `Retry-After`); a ban whose record the
  gatekeeper withholds (a device match) is still a ban. The endpoint matches the address only,
  so a device ban shows only as a platform-scoped refusal of the scene adapter: that refusal
  marks the platform ban. The endpoint's "not banned" does not lift it while the refusal stands:
  it asks the gatekeeper for that scene's room again, and the gatekeeper's admission lifts it, so
  a ban that expired or was lifted ends without a scene or realm change. Until then the notice
  shows the refusal's ban, which carries no reason or period. The local user's scene
  ban is the refusal of the
  scene adapter, read from its status and body (the gatekeeper's 403 for a platform or scene
  ban, the worlds server's 401 for a platform or world ban), where unity-explorer treats any 401
  or 403 as forbidden access. The player is not moved out of a scene that refused them, as
  there. Other players' scene bans and
  the admins come from the room's metadata and count only while the room is connected, as there.
- **Scene message trust is enforced in the engine.** Room metadata rejects banned senders
  across the string bus, binary bus and topics. New rooms without metadata grant ordinary peer
  permissions. Preview and local scene development trust peers, as Unity Explorer does for
  local scene development, so shared VideoPlayer sources sync without room admins; bans still
  outrank that trust. Metadata arrives as full snapshots. Missing or non-array lists retain
  their last good values defensively; explicit arrays apply their valid addresses and warn about skipped entries; an empty array clears
  its list. An entirely unusable non-empty ban array retains known bans; an entirely unusable
  admin array clears admin trust.
  Binary SDK CRDT batches and state responses exclude VideoControlState and the combined physics
  force/impulse components in both directions; incoming VideoPlayer edits require a trusted
  development realm, a room admin or the authoritative server. This protects the video source;
  peers can still move, hide or delete its entity. An untrusted state response omits VideoPlayer, so late joiners need a trusted sender to
  supply it. Filtering covers SDK envelopes 1 and 3. Auth-server SDK envelopes are
  opaque: those clients accept CRDT only from the authoritative server, but local-only component
  filtering for that generation is unverified. Malformed recognized CRDT packets are dropped.
  Outgoing binary payloads are limited to 13,269 bytes, reserving a sender-length byte and 42
  address bytes in Unity Explorer's 13 KiB receiver frame. Incoming binary payloads retain the
  30,000-byte bus limit for older peers. Strings retain 30,000 bytes; topics retain 13 KiB.
  Rejecting banned senders' scene messages is a deliberate divergence from Unity Explorer's
  scene pipe, which gates component trust through admin status and does not check its ban list.
- **Scene bans retain their admission owner.** A recognized scene/world refusal or local ban
  in room metadata unloads the matching parcel scene, without moving the player or unloading
  portable experiences. Refusals persist per scene across parcel changes within the wallet and
  realm session; only fresh admission for that scene clears its refusal. The shared room credential
  lifecycle rechecks admission with a minimum fifteen-second ban retry delay and honors longer
  server delays. A transient failure cannot clear a known ban. Successful fresh admission
  restores the scene while the player stays on or returns to its parcel. Leaving and returning
  preserves the retry deadline, including a longer server delay. A removed scene-room participant
  requests fresh admission; other room types retain their existing denial behavior. Preview,
  local and headless scene lifecycles are exempt from local-player unloading. Teleport arrival
  onto a refused parcel completes without waiting for the deliberately absent scene root.
  Unloading is a deliberate divergence from Unity Explorer's minimap ban mark: it prevents
  refused scene scripts from running while the player is nearby. A later platform refusal is
  reported independently of the retained scene/world ban, so device bans reach platform status
  even while the user stays on a scene that already refused them.
- **Realm entry refused during a platform ban.** The realm change reads `PlatformBanStatus`
  (`common::platform_ban`) and, while a platform ban is known, refuses every request ("You are
  banned from Decentraland"), so the lobby's pick, a link held for the
  terms and an in-world realm change all stay where they are and nothing loads or connects. A ban
  learnt while a change waits on world access (a world's password prompt) stops it with the same
  answer; one learnt after the realm's own fetch has started does not stop that change. The
  HUD reads the same ban from one stream: before the world it covers the lobby, welcome page,
  picker or loader with a blocking notice that a sign-out or a "not banned" answer closes, and a
  ban answered during the first entry keeps it open over the world (B1); a refused in-world
  travel shows the same notice, dismissible, and the player stays put (B5). A ban learnt after the
  first world entry opens nothing until a travel is refused, and an open notice words the period
  again each minute. Differences from
  unity-explorer, which checks once in its login flow and ends on a terminal blocked screen with
  Exit and Support:
  - Entering does not wait for the first check: an unknown or failed check lets the player in,
    and a ban answered later refuses realm changes from then on. unity-explorer awaits the request.
    The check starts at sign-in, before the lobby shows, so a ban is normally known before the
    pick; a launch straight to a link's destination can enter first and the gatekeeper's own
    refusal of the scene adapter then applies.
  - In-world realm changes are refused too, which unity-explorer does not check.
  - Sign Out instead of Exit: the web has no application to exit.
  - The notice words the period as unity-explorer does (hours under a day, else days, rounded
    up) and adds the ban's custom message.
- **Pulse only.** unity-explorer falls back to LiveKit-only after five failed start-up attempts,
  through dual-transport proxies and LiveKit broadcast targeting. Here Pulse is the only
  avatar-state transport and none of that exists, so a retryable failure is retried without limit.
  When a peer's Pulse membership ends, its avatar and nametag disappear, its avatar collider is
  removed, and it leaves Nearby and the map's People layer. Overlapping SDK player triggers see
  an exit when the collider is removed and an enter if it overlaps again on rejoin. Peers can
  stay in the LiveKit room for voice and Nearby chat. Nearby resolves @mention links
  through its roster, so an @mention of a room-only peer
  cannot open that peer's profile. Chat lines look up sender names by wallet in the profile store
  and fall back to a shortened address when no name is available. The next Pulse join places the
  avatar at its new pose immediately.
- **Pulse refusals set a floor under the next wait.** Server full (disconnect code 6) waits at least
  60 s, the pre-auth limits (7, 8) and the per-IP connection cap (17) at least 10 s, the per-IP
  scene-listener cap (18) 10 s doubling to 5 minutes, and all five are retried, as the Pulse
  server documents. unity-explorer applies its 10 s and 60 s waits only to a session that was
  already accepted, and ends Pulse on codes 17 and 18, which it does not know (`reconnect.rs`).
- **A rejected Pulse handshake is retried, and a realm change ends a terminal stop.** A rejection,
  or no answer within 10 s, closes the connection and the next attempt is a new one on the
  backoff; a ban, an eviction, misbehaviour or an unknown code stops Pulse until the realm changes,
  or for a scene listener, which follows no realm, until restart. unity-explorer stops Pulse on a
  rejection and on those codes until a new sign-in, which has no counterpart here: the account is
  fixed once in the world. It has no response timeout: it waits, and the server's pre-auth
  deadline ends the attempt with AUTH_TIMEOUT, which it retries.
- **Teleports and emotes are paced.** The client lets out 10 a second with a burst of 8, half of
  what both servers allow for them, and keeps only the latest teleport and the latest emote command
  while that is spent. Overrunning the servers' budget ends the session for good, and a scene that
  repositions the player every frame would do it; unity-explorer has no limiter (`pacing.rs`).
- **The client never sends what the Pulse server refuses for good.** A one-shot emote longer than
  60 s goes out without a duration and ends with an explicit stop, and inputs and teleports from
  outside the parcel grid are not sent (`lib.rs`, `pulse/mod.rs`).
- **A second device on the same wallet takes over; Pulse tells the old one.** Pulse ends the older
  session with `DuplicateSession` and the gatekeeper removes it from its island room; on
  `DuplicateSession` a player ends its island connection for good: the realm's rooms end in the
  identity-conflict state, which nothing retries, and a realm change starts them again, as it ends
  Pulse's terminal stop. A removal from the island room on its own stays a transient failure. The
  archipelago leaves the older socket open and sends it nothing more: it replaces, and tells with
  `Kicked`, only this device's own stale socket (same session key). `already_connected` is set in
  both cases, so it is only logged. A newer device that never reaches Pulse tells the older one
  nothing through Pulse (`archipelago.rs`, `pulse/plugin.rs`).
- **The scene-room request always signs as the explorer.** The engine's own gatekeeper requests
  carry `signer: dcl:explorer` and the comms-handshake intent; only fetches a scene makes sign as
  `decentraland-kernel-scene` (`lib.rs`, `restricted_actions`).
- **IPv6 when no IPv4 address resolves.** The native driver connects to the first IPv4 address and
  falls back to the first IPv6 one, on an IPv6 socket, only when there is none. unity-explorer
  fails without an IPv4 address, and the servers bind IPv4 by default.
- **One Pulse subject per wallet.** A newer join for a wallet replaces the subject it held: the
  avatar snaps to the newcomer, or leaves the older subject's realm when the newcomer joined in
  another. unity-explorer keeps the older subject and promotes it if the newer one leaves. Both
  servers evict a wallet's older session once the newer one authenticates, so the older subject
  is always a session already gone, and promoting it would show its last pose and ask for a
  resync the server never answers. One gap is accepted: the .NET server keeps the older session
  visible for up to 5 s after the eviction, so an observer that first sees both then can hear the
  older join last. The avatar then follows the dead session and leaves with it, and the live one
  shows again only once it re-enters that observer's view. dcl-one's server removes the older
  session before the newer one is visible, so it never happens there.
- **Pulse reconnect waits are jittered.** Every wait, from the 3 s after a drop to each backoff
  step and refusal floor, is scaled by a random factor between 0.8 and 1.2; unity-explorer's waits
  are exact. Clients that fail together, every client of a restarting server or many behind one
  address, would otherwise come back at the same instant. The servers hold at most 512 connections
  waiting to authenticate, 32 per address, so the excess is refused (codes 8 and 7), and the 10 s
  floor would send every refused client back together again. The Pulse server's documentation
  calls jitter mandatory for those codes.
- **Scene raycasts are capped only outside the scenes the player is in.** The system scene, every
  portable and the scenes the player stands in run all their raycasts every frame, with no cap;
  every other scene shares a tenth of the target frame (1.67 ms at 60 fps, 0.69 ms at 144),
  nearest scene first, and what does not fit waits for a later frame. unity-explorer instead caps
  the current scene at 3 ms and runs no other scene's raycasts. We differ because scenes drive
  frame-sensitive logic from ray results: `movement-scene`, the default avatar movement driver,
  runs its ground and wall checks on continuous `RS_AVATAR` rays, and for it a slower frame is
  better than a late answer. A share of the target frame, not a fixed time, keeps the cap the same
  part of the main thread at any frame rate. See [Interaction](interaction.md#scene-raycasts).

- **Photo mode follows the engine's cursor convention.** unity-explorer's in-world camera has its
  own Lock/Unlock cursor actions. Here right-click toggles the lock as everywhere else, left-drag
  looks while the cursor is free (Unity's drag), and Escape closes photo mode. The gallery key (G,
  Unity's K) closes photo mode and opens the gallery, where Unity's K only closes it.
- **Photo mode has no gamepad toggle, and its pan is on the D-pad.** unity-explorer opens the
  in-world camera with the pad's East button and pans with the bumpers (and `dpad/y`). Here East
  is the scenes' secondary action and nothing is reserved outside photo mode, so a pad binding
  would open the camera from inside a scene's own action; the toggle is C, the rail's Camera
  button and `/photo_mode`, like the other HUD hotkeys. Inside photo mode the bumpers already zoom
  (the player camera's binding), so pan is D-pad up/down.
- **Photo mode is not feature-flagged.** unity-explorer gates the camera reel behind
  `alfa-camera-reel`; the Gallery already ships ungated here and photo mode does the same.
- **A photo's `screenRect` is sent and dropped.** Each visible person carries the rectangle they
  cover in the saved image, as unity-explorer computes it; the catalyst's camera-reel service parses
  the metadata into a struct without that field, so it is not stored. Kept for parity with the
  upstream service's clients.
- **The photo viewer (#2073).** Its layout and numbers follow unity-explorer's, with four
  differences. The info panel shows a loading view while a photo's details load; unity-explorer
  never shows one. The owner's name opens their passport; unity-explorer's owner button cannot be
  reached. A person's row is disabled for guests, because the
  profile store cannot tell a profile that is missing from one still loading, which is the case
  unity-explorer disables. Jump In treats a photo as taken in a World when the HUD's `worldOfRealm`
  accepts its realm, so hyphenated ENS names and Worlds-server realm URLs count, where
  unity-explorer's test is `^[a-zA-Z0-9.]+\.eth$` and sends those photos to a Genesis City parcel.
- **The photo camera stops at scene geometry instead of sliding.** unity-explorer moves a capsule
  character controller, which slides along walls; here the move is swept and refused whole when
  the camera's sphere would cross a scene collider. A collider the camera starts inside (the player
  camera has no wall avoidance, so photo mode can open in one) does not block it, so it can leave.
- **No camera-to-player dither in photo mode.** unity-explorer keeps fading what stands between the
  camera and the avatar; here the fade showed in the photos, so the dither depth is zero while the
  mode is on and the player camera's depth comes back when it closes.
- **The photo camera looks through the player camera's input path.** unity-explorer turns it by a
  rotation speed times the frame time, which depends on the frame rate and ignores the sensitivity
  setting; here the pointer delta goes through the player camera's sensitivity and smoothing
  settings, scaled by 0.17, which matches the Unity feel at 60 fps.
- **The photo camera's wheel zoom is per line, and a held zoom button zooms per second.**
  unity-explorer scales the scroll delta by 50 everywhere but macOS, per frame; here a wheel line
  (16 pixels of a trackpad's scroll, on every platform including web) zooms as Unity's 60 fps frame
  would, and a held bumper zooms at Unity's per-second rate, so neither depends on the frame rate.
- **What a scene is doing to the player is listed with its permissions.** A held camera, a
  hidden avatar or passport and changed movement all happen under a permission that allows by
  default, so nothing asks. The permissions popover lists them under "In force now" for the
  scene the player stands in, and each row opens that scene's rule, so the player can refuse
  it there. There is no separate restrictions list.
  - The engine reports a start and an end on the permission-used stream (`inForce`), read
    from the effect itself, not from the grant: a granted camera permission with no camera
    held lists nothing. A stream that attaches is sent everything in force.
  - Each effect is reported under the scene that causes it, recorded where it is applied
    (`PrimaryCamera.scene_override_owner`, `PlayerModifiers.hide_scenes` and
    `locomotion_scenes`), so the row's rule is the one that controls it. A hidden nametag
    counts like a hidden avatar or passport.
  - Movement is listed only while a setting or a block is on: an area with nothing set, or an
    input modifier whose flags are all off, lists nothing.
  - A row opens a stored rule, never "This visit": that answer forgets the rules of every
    level and would not end what the scene is doing.
  - The popover lists the scene the player stands in. What a portable experience or a
    neighbouring scene's area does to the player is reported under that scene and is not
    shown there; `/permissions_in_force` has it.
  - A rule changed from the row applies from the next visit to the scene.
- **Smart wearable permissions are asked on use, not at equip.** unity-explorer asks once, when a
  smart wearable is equipped (or at login for one already worn), to allow the permissions its
  `requiredPermissions` declares, and refuses what it did not declare. Here a smart wearable is a
  portable like any other: each restricted action goes through the stored rules and the per-use
  prompt, which shows what is being done (the URL to open, the realm to change to). So
  `requiredPermissions` is ignored, and nothing is asked before the scene starts. On native,
  `fetch` and WebSocket are asked per use like the rest (`dcl_deno`); on web they are not
  permission-gated for any scene (scene code can reach the page's raw `fetch`), so a
  declared-permission check on them would be bypassable.
- **`portableExperiences: disabled` pauses every listed portable.** unity-explorer unloads smart
  wearables while the player stands in a scene that disables portable experiences and loads them
  again on leaving. Here every portable the player can see (not startup ones, not the HUD's scene)
  is paused through `RendererSceneContext.blocked` and its UI is hidden, so it keeps its state
  across the crossing and nothing has to remember a portable that is not running.

- **Donate is on `/play` only, and for web3 sign-ins only.** unity-explorer offers it to any
  signed-in player through its wallet. Here only the Sites `/play` web shell holds a wallet that can
  sign a transfer, so the Explorer's own HUD shows no Donate. MANA is shown to web3 wallet
  sign-ins only (AGENTS.md), so an in-app (managed) sign-in sees no Donate either, and the shell
  refuses one that asks. See [the ADR](../../../../docs/adr/2026-11-12-play-shell-signs-tips.md).
- **The Donate panel is drawn by the web shell, over the app.** The amount is confirmed where the
  wallet is, and the shell resolves the creator itself; the HUD names the place only.
- **An unclear tip outcome is shown as processing, with no Try again.** unity-explorer's error
  view always offers Try again. Here a failure that may have reached the chain (the relay's answer
  lost or a 5xx after signing, a transaction not yet mined) shows the tip as processing and watches
  it; Try again appears only after a clear failure (the wallet declined, nothing was signed, the
  transaction reverted), so a player never tips twice. A tip still unconfirmed after 30 minutes
  stays "still being processed" with no retry, and opening Donate again shows it until it settles.
  There is no fallback to a transaction from the wallet: the relay's refusal fails the tip, and
  trying again signs the same nonce (pinned), so at most one can land; once that nonce is used, the
  next send signs nothing and watches the failed attempts until one is seen mined.
- **The MANA price comes from the deployment.** unity-explorer reads the MANA/USD rate from
  CoinGecko; the panel reads the catalyst's price service (`/price/api/v3/simple/price`), which
  answers in the same shape.
- **The creator is the scene's deployer.** The places service's `creator_address`, which the tip
  goes to, is the deployer of the place's scene here.
- **Donate's buttons.** The minimap's uses unity-explorer's tip icon, beside the collapse button;
  the map's place panel has it in its row of actions, between Home and Share, as there. The fourth
  amount choice reads OTHER, and the minimum reads "Tips must be ≥ 1 MANA".
- **Gifting is on `/play` only, for web3 and managed sign-ins, never a guest.** As with Donate, only
  the Sites `/play` web shell holds a wallet that can sign the transfer, so the Explorer's own HUD
  offers no Gift; it still shows the gift-received popup, which signs nothing. Both wallet kinds
  sign the gasless meta-transaction, so both may gift; a guest can't, as in the Shop. See
  [the ADR](../../../../docs/adr/2026-11-16-play-shell-signs-gifts.md).
- **The shell asks for a second confirmation.** unity-explorer's picker SEND GIFT starts the
  transfer. Here it opens the shell's "Send Gift to" step (the item and the recipient's name and
  address as the shell read them, SEND GIFT and CANCEL), and only that press signs: an email or
  social account's wallet signs with no prompt, so the app frame alone must not be able to move
  items. The shell also re-reads the token's owner on chain, checks the collection is the
  marketplace index's on the build's chain and that the recipient is another account, and signs
  nothing else (the guard). Closing it before the wallet signs cancels the gift.
- **"Gift Sent" shows once the relay takes it.** As in unity-explorer. A token moves only once,
  so a gift can't be sent twice; an answer lost on the way is settled by the token's owner, and
  TRY AGAIN after a refusal signs the same nonce. There is no fallback to a transaction from the
  wallet.
- **Ethereum items are listed, and refused on SEND.** unity-explorer lists every on-chain item
  and its transfer of a `collections-v1` item can't succeed; here the picker lists them too and
  SEND shows "Cannot send this item as a gift because it's not a transferable NFT.", since the
  relay serves Polygon only.
- **Pending gifts are kept by the shell, per account, for a week.** unity-explorer keeps them in
  its player preferences. The app frame has no storage here, so the shell keeps them; the picker
  hides a token only while the owned catalog still lists it with the `transferredAt` it had when
  sent.
- **The slow-transfer link opens `/shop`'s Activity page.** unity-explorer links the Marketplace's
  activity; here the Shop's Activity page is that page.
- **Where Gift is offered.** The profile card of chat names, nametags and the friend row (after
  View Profile), the community member menu (after Call; greyed for a blocked member either way),
  and another player's passport options (before Block; the picker replaces the passport). The
  voice-stream panel's menu has none, as in unity-explorer. As there, a guest recipient is offered
  Gift too. A picker error is a toast and the
  picker stays open; unity-explorer closes its picker on SEND before it can show the error.
- **Gifting's analytics events and popup sounds are not ported.** unity-explorer reports sent,
  successful, failed and cancelled gifts and plays a sound with the sent and received popups; this
  port leaves both out for now. The shell's "Preparing Gift for" tile shows the
  marketplace index's item category (wearable or emote), so its corner glyph is the generic one.
- **Media reopens on the web.** The web backend keeps unity-explorer's counts and delays: a
  failed URL source is reopened up to 5 times, 2, 4, 8, 16 and 32 s after each failure, the count
  restarts when the scene changes the source, and a source that gave up stays failed until the
  source changes. It differs in five ways:
  - *States between reopens.* unity-explorer reports `VsError` after every failed attempt; here a
    scheduled reopen reports `VsLoading`, so `VsError` means the source gave up or is failed and
    paused.
  - *Failures during playback.* unity-explorer reopens only a source that failed to open; here an
    element error at any time starts the schedule, and a live stream that ends (its feed dropped)
    counts as a failure rather than a normal end, so a stream whose feed drops reconnects without a
    scene reload. A reopened source that is not live resumes where it failed, or where the scene
    seeked meanwhile.
  - *Count restart.* unity-explorer clears the count as soon as a reopen opens; here it restarts
    only after 5 s of playback since the last failure, so a source that keeps failing at the same
    point gives up.
  - *Pause.* unity-explorer reopens a failed source whether or not it plays; here a failed source is
    reopened only while the scene plays it, so a paused screen requests nothing. A failure while
    paused, or before the scene first plays the source, counts and reports `VsError`. The next play
    reports `VsLoading` and reopens once the current delay has passed since the failure, so toggling
    playback never reopens faster than the schedule, and a pause keeps the count. A pause during a
    loading reopen lets it load, as a paused first open does. A live stream resumes at its live
    edge, not at a saved position.
  - *The screen.* unity-explorer renders black whenever a player is in `VsError` or `VsNone`; here
    the image turns black when the source gives up or a non-looping source ends, and keeps the
    last frame while a reopen is pending, so a short drop shows no black flash; the failed source
    is detached meanwhile. A non-looping source that ends reports `VsNone`, as in unity-explorer.
    A source the scene clears (an empty `src`, such as the Admin Toolkit's Deactivate) shows black
    at once on native and web, whatever played before.

  Native (FFmpeg) has no reopen: its first error ends playback. A non-looping source that ends
  there reports `VsPaused` and keeps its last frame, where the web reports `VsNone` and clears it.
- **The map's markers are HUD elements, not a rendered scene.** unity-explorer draws its map and
  markers with a camera into a texture; here the atlas and every marker are elements of the HUD
  page, placed from the map camera's position and zoom. The seven zoom levels, the marker sizes at
  each, the clustering grid, the draw order and the hover and appear timings are the same. Culling
  and pooling have no counterpart, and a click goes to the topmost marker under the pointer.
  The sea around the city is one image, rendered once from unity-explorer's sea shader and placed
  under the satellite map like its tiles; its water stands still, where unity-explorer animates
  it every frame. Nothing on the map is drawn per frame.
- **The minimap draws the full map's layers.** As unity-explorer's, it shows live events,
  points of interest, other players, the home pin and the route, and the full map's Layers panel
  switches them on both, from one read the two maps share; the minimap has no category pins or
  markers setting of its own. A destination's pin stands on the minimap's edge in its direction,
  as unity-explorer's. unity-explorer's minimap doesn't zoom and moves the pin to the edge past a
  fixed distance (about eight parcels); this one zooms, so the pin moves to the edge exactly when
  the destination leaves the visible map. It is the full map's destination pin, and the minimap
  being square, it runs along the square's edge rather than a circle. Scene map pins are on
  both (below). The header's heart marks the place the player stands in as a favourite; in
  a World, the World's scene there.
- **Scene map pins come only from portable experiences.** The engine reads `MapPin` only from
  scenes running as a portable (from the launch option or console, spawned by a scene, or a smart
  wearable's) whose scene.json also declares `isPortableExperience: true`, and streams them to the
  HUD (`getMapPinsStream`); every other scene's pins are ignored, and no flag turns them on for
  other scenes. unity-explorer reads them from any scene that declares `isPortableExperience:
  true`, however it was loaded, and from every scene under `alfa-map-pins`; here a parcel scene or
  World that declares it can't put pins on its visitors' maps. The Layers panel's Quest Mini-Games
  switch (on by default, for the session) shows or hides them on the full map and the minimap. A
  pin stands on the centre of its parcel, its position rounded halves to even; it is never
  clustered, its title and description are not drawn, and a click opens the place panel for its
  parcel. It always shows the default quest pin: the pin's texture is not read, so a scene can't
  make the HUD request a host of its choosing. unity-explorer loads the scene's own icon only
  under `alfa-map-pins-custom-icons`, and otherwise always shows its default pin. The title is cut
  to 100 characters. The pin doesn't pulse.
  This explorer loads no global portable experience at startup: unity-explorer loads the quests'
  one (`globalpx.dcl.eth`) from `alfa-global-portable-experiences`; here it is loaded with the
  launch option (`--portables` or `?portables=`, e.g. `movecontroller.dcl.eth;globalpx.dcl.eth`).
- **Arrival is checked on every position report.** unity-explorer checks the route only after
  the player's squared distance from the map's origin has changed by 300 map units, which can
  skip the arrival spot when the player stops on it or reaches it sideways, so the route never
  ends. The arrival rule itself is the same: within about 5.6 m of the destination parcel's
  centre, then "You have arrived to your destination" and the pin goes.
- **Home belongs to the account, and can be taken back.** As in unity-explorer, the engine keeps
  one home per wallet address in its config, and Set as Home on the place that is home (the place
  panel's and the place detail's buttons, the minimap's switch) takes it back. An account with
  no home has no home pin and no Set as Home on, and starts on parcel 0,0 of the default realm.
  Before sign-in the home is the stored sign-in's account's, else the last account's. A home an
  install kept before homes were per account goes to the first account that signs in. A guest
  with no account (a scene preview's `?guest=1`, `/login_guest`) has a new address each session,
  so it is no account here: it reads the home that applies before sign-in, is not offered Set as
  Home, and neither takes nor writes a home. A guest account signs in and keeps its home like any
  other account.
- **Jump back to Genesis City lands on the home parcel, else on 0,0.** The minimap's button in a
  World travels to the default realm naming a parcel: the account's home when it is in Genesis
  City, Genesis Plaza otherwise; `/goto genesis` always names 0,0. A realm change that names no
  parcel keeps the player's parcel, clamped to the new realm's map, which from a World is a parcel
  of that World's grid. unity-explorer lands the button on its start parcel, read once at launch
  (a launch position, the home parcel, or `alfa-genesis-spawn-parcel`); here the home is read when
  the button is pressed, so a home set during the session counts.
- **Home is set from the map's place panel, the place detail and the minimap's menu.** The place
  detail, opened from the lobby's cards and the Places page, saves the place's base parcel (a
  World, its name) and offers start and exit navigation for a Genesis City place; a Jump in there
  that goes ahead ends a route to that place, as the map's does. unity-explorer also sets home from
  Places cards, which have no such control here. The minimap's menu is unity-explorer's: Set
  as Home, Copy Link and Reload Scene, then this HUD's own Minimap settings. As unity-explorer's,
  the minimap saves the parcel the player stands on, where the place panel saves the place's base
  parcel (a road's, the picked one), and it compares a World's name without case where the place
  panel compares it exactly. A World's copied link names the World and the parcel and carries the
  sharer's address, as unity-explorer's.
- **The map fixes what the upstream panel leaves broken.** Search results carry their LIVE badge
  (upstream keys the lookup by a malformed parcel, so none shows); "remind me" on an event row
  starts from the player's saved interest and keeps working after the list is rebuilt; a like or
  a dislike refreshes the rating shown; a like, a favourite or an interest the service refuses
  goes back to what it was; a place without photos says so; closing the map forgets the whole
  back history; "Link copied!" is a toast of its own instead of replacing the worlds warning's
  text; and the text of a post on X and of a calendar entry is url-encoded.
- **Escape in the map's search field clears it first.** The next press leaves the field and the
  one after closes the map, as on the HUD's other pages.
- **Events page: JUMP IN closes the page and the event detail.** unity-explorer leaves both open
  while the player travels. Here a jump reconnects the realm under the loading screen, and every
  other page closes on a jump, so Events does too.
- **Events page: its links carry the HUD's `utm_source`.** unity-explorer tags them
  `utm_source=explorer`; here they carry `bevy-web-client` or `bevy-desktop-client` like every
  other link out, and keep `utm_campaign=discover`. Event names and descriptions are URL-encoded
  in the share and calendar links, and so are the world, the referrer and the event id in the
  jump and event links; unity-explorer leaves all of them raw.
- **Events page: JUMP IN travels to a world only by its name.** The events service takes any
  text as an event's `server`, and the engine reads anything but a world name as a realm's
  address. unity-explorer builds the worlds server's address from it; here a `server` that is
  not a `.eth` name does not travel, and a toast says the world cannot be reached.
- **Events page: the calendar never steps before today.** unity-explorer steps back by the days
  shown and locks the arrow only on today itself, so after the highlighted events come or go,
  or past midnight, it reaches earlier days. Here a step back stops at today.
- **Events page: a recurrent date's offset keeps its minutes.** unity-explorer writes whole
  hours, `(UTC+5)` in a `+5:30` zone; here it reads `(UTC+5:30)`.
- **Events page: a failed read shows an error with Retry.** unity-explorer shows a toast and
  leaves the loading placeholders up. A read the bridge has not answered in 10 seconds counts as
  failed (the limit `library/explorer-core/lib/shell.ts` gives a request); an answer that arrives later still shows,
  and so does a reminder the service confirms late.
- **Events page: one copy of an event.** unity-explorer changes the reminder on the card's own
  copy, so a card scrolled out and back shows the old state; its day view's card buttons are not
  wired, and its banner is drawn before the friends list arrives. Here the calendar, the day
  view, the banner and the detail read one event, every card's buttons act, and friends show
  when they arrive.
- **Events page: analytics.** unity-explorer reports `event_card_clicked` from calendar cards
  only and nothing for an open from the sidebar; here a day view's cards report it too, and the
  sidebar reports `events_section_opened` with `source: sidebar`.
- **Place detail: no analytics for home or navigation.** unity-explorer reports
  `place_set_as_home` and `place_navigation_started` from its place detail, and
  `home_position_set` whenever home changes; this explorer reports none of them, from the place
  detail or the map.
- **Events page: no "error loading friends" or "My Communities" toast.** The friends list and
  the member-of list reach the HUD as snapshots with no failure signal, so there is nothing to
  report; cards show friends and community marks once the lists are there.
- **Events page: shapes are CSS, icons are unity-explorer's.** Its rounded containers, outlines
  and gradients are nine-slice sprites there; drawn as masks at the HUD's fractional scale they
  show seams between the slices, so here they are radii, borders and gradients with the sprites'
  colours. The canvas scales by the smaller of width and height, as every menu page does, where
  unity-explorer's scales by width.
- **Jump to Location closes the passport.** unity-explorer's passport does not close itself
  when its Jump to Location runs; here the passport closes once the friend's parcel or World is
  known, as it already does for a photo's teleport. A friend found nowhere leaves it open, and
  nothing is shown, as there. The profile card offers the jump after View Profile, since its rows
  already follow a different order from unity-explorer's user menu.
- **INVITE FRIENDS is for a signed-in player only.** unity-explorer's copy runs for any
  identity it holds. Here neither kind of guest gets the button: not the throwaway guest, and not a
  guest account. As there, it copies the deployment site's `/invite/<address>`
  and says "Link copied!"; the panel stays open.
- **A new referral tier opens the site's Referral Rewards tab.** unity-explorer opens
  `profile/accounts/<address>/referral`, which this site does not serve; its profile names the
  same page `/profile/<address>/referral-rewards`.
- **The hills follow the Landscape Distance.** The distance is the config's `landscape_distance`
  (default 1000 m), read at startup and held to 500 to 7000 m.
  Unity displaces its whole terrain and its Landscape Distance setting does not drive it. Here the
  ground's rings are displaced out to the Landscape Distance from the player, never less than the
  832 m the parcel-sized rings cover, and the hills flatten over a further tenth of it, so
  the rest of the 13 km ground costs the vertex shader nothing and walking moves no vertex abruptly.
  Each ring coarser than a parcel snaps to its own cell size, so its vertices stay on the same
  terrain points and the far hills do not swim.
- **Cliffs reach 768 m at most.** Unity culls each cliff prefab whose bounds are beyond the
  Landscape Distance from the camera. Bevy's cliffs are procedural chunks built around the
  player's parcel out to the nearer of 768 m and the Landscape Distance: building them out to
  7 km would mean hundreds of chunks with their invisible walls.
- **The sea is always drawn.** Unity culls its water chunks one by one beyond the Landscape
  Distance. Bevy has one sea plane, a single opaque quad that casts no shadows, so hiding it would
  save almost nothing, and the water material draws without fog, so nothing would soften it
  appearing and disappearing.
- **The wave ocean is not ported.** Unity's ocean (a wave grid, foam and an alpha-blended surface)
  was ported and compared in the same views, and it looked worse than Bevy's current sea, so the
  sea stays as it is.
- **The lens flare's rings are procedural, and its random copies use a hash.** Unity draws the
  rings from URP's sample textures, which are not carried; Bevy draws bands of colour measured
  from them (radius, width and brightness per channel, and the partial ring's arc). Unity places
  the copies with its seeded random generator.
- **The lens flare sees transparent surfaces only through depth of field.** Its occlusion test
  reads the depth buffer and, where the view has depth of field's transparent focus texture, the
  transparent coverage in it. A view without that texture (every MSAA view) is hidden only by
  opaque geometry, not by imposters. The flare is off while a scene hides the sun and moon discs.
- **Graphics presets set fewer settings than Unity's, and four differently.** Low, Medium and High
  carry Unity's values for the frame rate, VSync, render scale, avatar outline, jiggle bones, sun
  shadow quality, the Low and High shadow distances and scene lights; Medium is the default and
  the one every saved config migrates to, except jiggle bones, which stay off by default (see the
  spring bones ADR). Anti-aliasing and depth of field differ (next entry). Every preset keeps
  bloom on, as Unity's Medium and High do; Unity turns bloom off on
  Low. Medium's sun shadow distance is 20 m, not 60 m: Medium
  has one cascade (above), which stops at 40 m, and at 20 m its texels are half as large as at
  40 m. The presets leave out Unity's scene distance (in parcels, where far scenes load as
  LODs; here imposters draw them and the load distance stays the player's), the landscape
  distance (read only at startup; its default is Medium's 1000 m), current-scene streams and
  HDR (always on). The preset is not stored: the HUD reads it off the settings, and any
  difference shows as Custom.
- **Depth of field yields to MSAA.** The fork's transparent focus texture takes the sample count
  of the resolved main texture, so a multisampled transparent pass with depth of field fails
  wgpu validation. While MSAA is on, the camera has no depth of field and the stars draw before
  the transparent pass. Unlike Unity, which has none, the presets keep depth of field on, because
  it hides the lack of detail in distant imposters, and they use FXAA so it stays on: the yield
  only applies when a player picks MSAA. SSAO is off in every preset: bevy 0.16's SSAO is not
  good enough to put in one.
- **VSync caps the frame rate; it does not set it.** Unity's `vSyncCount` hands the rate to the
  monitor and ignores the frame limit. Here VSync only removes tearing: the explorer paces to the
  lower of the Target Frame Rate (or a session override) and the monitor's refresh rate. Medium's
  60 fps holds on a 120 or 144 Hz monitor, and the debug panel's 30 FPS limit, `/fps` and
  `--fps` still apply; Max means the monitor's rate. A target that does not divide the refresh rate
  shows frames for an uneven number of refreshes (60 fps on 144 Hz alternates 2 and 3), so motion
  can judder slightly; 60 fps on 60 or 120 Hz is even.
- **No satellite floor.** unity-explorer lays the minimap's satellite images flat at 0.205 m under the
  terrain in Genesis City; here the ground covers that plane everywhere, so the floor would never be
  seen and is not shipped.
- **Spring bones skip avatars the avatar limit hides.** Unity ranks every avatar in front of the
  camera for its ten simulated places. Here an avatar hidden for being beyond the `max_avatars`
  nearest to the player takes no place: its bones stay at rest and the place goes to the next
  visible avatar, since a swing nobody sees would cost a place and its transform walks. Do not
  rank hidden avatars to match.
- **A scene's colliders answer only where the scene is drawn.** The avatar, the pointer and
  other scenes' world raycasts meet a scene's colliders only over the scene's own parcels, up to
  the height limit of each connected group of them, and 0.05 m past both: the hard edge of the
  scene materials. With the out-of-bounds effect on (`graphics.oob`) the materials dissolve what
  lies past that edge over a wider band; colliders keep to the hard edge. Unity keeps every
  scene's colliders in one physics world, so a collider that is on is solid wherever it reaches,
  over a neighbour's parcels too. Each scene here has a collider set of its own, so a query is
  cut to where the scene applies: the avatar asks a scene only while its capsule touches that
  space, and a ray or a swept capsule is cast against a scene only over the stretches inside it.
  Three consequences look like mistakes:
  - A ray that comes into a scene inside a solid primitive passes through that primitive, as a
    ray does not hit what its origin is inside. Mesh colliders are surfaces, hit as usual.
  - An avatar that comes into a scene already overlapping one of its colliders, having walked
    through the part that is not solid outside, is stopped at the scene's edge when its centre
    line is inside the collider or the overlap would push it back out. Any other overlap lets it
    in, and the push-out resolves it as usual: that is how it steps onto ground a little higher
    than the ground it came from. The top of the drawn space is such an edge: an avatar that
    falls onto a collider which rises past the height its region is drawn to lands at that
    height and stands there, on the ground.
  - A platform that carries the player over the scene's edge drops them once their capsule is
    clear of the scene.

  A scene's casts at its own colliders (in a world cast too), trigger areas, the proximity
  lookup and the photo camera are not cut. Neither is collider data without bounds: portables,
  the terrain, and scenes in a run that draws out-of-bounds geometry. The whole-collider test
  (below) still turns off a collider that leaves the scene's rectangle.
- **A scene's `AvatarNametag` plate is bounded before it is laid out.** unity-explorer draws
  the label and the colours as the scene wrote them. Here the label is cut to its first 128
  characters, a control character in it reads as a space, and a colour channel is kept between
  0 and 1 (one that is not a number reads 0), so no scene sets the cost of shaping its label or
  draws outside the tag. Do not pass them through to match.
- **One scene entity plates an avatar.** In unity-explorer two entities can ask for a plate on
  the same avatar: the last write shows, and the plate goes when either entity does. Here a
  scene has one entity for each player id and only the scene the local player stands in plates
  anyone, so the case does not arise.
- **A plate stands clear of the mention ring.** The plate floats 2 units over the name's box,
  as in unity-explorer, and the ring of a mention reaches 3 units out of that box. While the
  ring shows, the plate rises by the ring's width, so it never covers the ring's top edge.
- **SDK Animator: a reversed clip, a dropped state, a new model and weights.** A clip with a
  negative `speed` (re)starts on its last frame, as the protocol defines the initial state; a
  clip left out of a later `states` list stops; the Animator is applied to the model a new
  `src` loads without waiting for the scene's next write; `weight` is kept between 0 and 1,
  and clips playing together mix by their normalised weights. Unity's player for legacy clips
  passes `speed` and `weight` through, starts every clip on its first frame, leaves a dropped
  clip as it was and gives each clip a layer of its own. Do not copy those.

## Deliberate choices that look like mistakes

- **Fog does not hide the loading edge.** The fog is the sky preset's: one exponential density
  (0.0005 per metre) and a colour by time of day, as the Unity client renders it. It does not
  thicken with a shorter scene load distance, so the edge of the loaded scenes shows through it,
  and a scene's global light does not tint it. A scene sets fog colour and density with its fog
  component. The Off setting is no fog at all.
- **Scene labels lay out as in the Unity client, not as in CSS (#1976).** A label with an `auto`
  axis (every react-ecs node since `@dcl/sdk` 7.5.0) sizes to its text, and its entity's
  `justifyContent` and `alignItems` place it; `textAlign` acts only inside the text's own box, so
  an unstyled label stretched across a row is not centred. A label sized in px or % on an axis is
  a box its text aligns inside by `textAlign`. A label with no size draws over its entity and
  takes no space. Every label's text box has the margin (top 4, right 4, bottom 2, left 2 px)
  and padding (top 4, right 2, bottom 4, left 1 px) that the Unity client's default runtime UI
  theme gives scene labels; CSS adds none. They are logical pixels like every other scene size,
  so on a display with a scale factor of 2 they are twice the size the Unity client draws, which
  counts them in device pixels. Scenes are authored against the Unity client, so its label
  spacing is kept. A scene input's text is inset 2 px from its sides, CSS's default
  `padding-inline`, not the Unity theme's 12 px; a larger inset belongs in react-ecs, where a
  scene can see and override it. The label spacing is an engine default that a scene cannot
  override; if react-ecs starts sending a default label margin and padding, both clients drop
  theirs at the same time, or new scenes get the spacing twice.
- **glTF occlusion textures are ignored.** Scene materials drop a glTF's occlusion texture, so
  baked ambient occlusion does not darken the ambient light. The Unity client's scene shader
  does not sample it either, and content is authored against that look.
- **Colliders are bounded by the scene's rectangle, meshes by its parcels.** A scene collider is
  off as a whole while its box reaches outside the rectangle around the scene's parcels or above
  the scene's height limit; the shaders clip meshes to the parcels themselves. One large collider
  often covers an irregular scene's whole footprint: Genesis Plaza's ground and building colliders
  span the notches its road scenes sit in, and a test against the parcels would turn them off as
  a whole. Unity (`333aa8826979186f1380058cc75f74b9630e4879`) tests the same rectangle, with the
  same allowance at the edge. The height limit splits the same way: colliders take one limit from
  the scene's whole parcel count, as Unity does, while the shaders clip each connected group of
  parcels at its own, so over a scene in several pieces a collider can be on above the height
  its mesh is drawn to (two parcels apart: meshes to 20 m, colliders to 31.7 m), where only the
  scene's own raycasts and trigger areas meet it. The box tested
  for a mesh collider is the mesh's own box turned with the collider, which can be larger than the
  mesh: a wall that runs diagonally in its own model can turn off while it is still inside the
  rectangle. Triggers are not tested in either client. Preview, loopback and editor runs draw
  out-of-bounds geometry and leave its colliders on too, so a creator can stand on, bump into
  and select what reaches outside; every other run turns them off.
- **The map carries over upstream's marker rules as they are**, so the two clients' maps match.
  A place of several parcels is marked on the parcel a walk over them stops at, which depends on
  their order and is not always the middle one; a cluster stands half a parcel south-west of the
  average of its markers; the clustering grid is coarser at zoom level 2 (27 parcels) than at
  level 1 (25); the other players are read when the People layer comes on and after a teleport,
  not on a timer; a route keeps its destination across a teleport; and an empty search says "No results
  founds".
- **`UnapprovedPathMode::Allow`** in the asset server: bevy's check only admits paths under
  the executable's assets directory and the IPFS cache lives elsewhere. Containment is done
  in `ipfs_path.rs` instead. See the security page.
- **Only a url's cached file is dropped when its asset fails to load.** A load its loader rejects
  drops the entry it read, once per entry per session, when that entry is filed under a url, and
  every asset that failed on it is then reloaded once. So a url whose bytes never decode is
  downloaded again in every session. An entry under a content hash is never dropped this way: it
  was checked against the hash before it was written, so a bad one is the asset processor's
  output or disk damage, which belong to the processor and the disk, not to a retry.
- **The service worker does not answer content requests from the content cache.** The engine
  reads `ipfs-path-cache-v2` itself before it queues a download (`crates/ipfs/src/lib.rs`, the
  variants client), so a lookup in the worker could only repeat that miss; the worker only trims
  the cache. A hit missed in between (the same hash finishing under another url, another tab), or
  one that appears before a retry (the engine reads the cache only before the first attempt),
  costs one more download of the same bytes (#726).
- **Web-only engine switches are page params**, not `LaunchOptions` fields.
- **`CommsApi.consumeMessages` resolves to a bare array** of `{ sender, data }`, not the
  protocol's `{ messages }`, because unity-explorer, the client scenes are tested on, returns
  the array. Changing it to the protocol's shape would break those scenes here.
- **A scene's messages use the scene room that carries the scene.** The message bus and topic
  messages, sent and received, and the `isConnectedSceneRoom` flag in `RealmInfo` all ask
  `comms::scene_rooms::SceneRooms`. A message sent while that room is connecting or reconnecting
  waits in the room's channel and goes out once it is connected; with no room that carries the
  scene it is dropped. The flag is true only while that room is connected. A client's room carries
  the scene the player is in. In a local preview or a world with one scene it carries every scene
  of the realm.
- **Native HUD preferences are persisted by the engine** (`hud-prefs.json`), because CEF's
  `localStorage` does not survive.
- **Web is Chrome only.**
- **HLS plays natively on the web.** A media source is assigned to the element's `src` whatever
  its format; there is no hls.js. Desktop Chrome plays TS and fMP4 HLS, live streams included,
  through a plain `src` with `crossOrigin = "anonymous"`, and its frames copy into a
  `VideoFrame`. HEVC in HLS and Chromium builds without proprietary codecs do not play; neither
  is a target.
- **The end-to-end tests use no Playwright.** They drive the client through its console on both
  shells, so one spec runs on the web and the standalone, and what they need from a browser (real
  input, pages outside the client such as sign-in, screenshots) the in-repo web driver does
  natively over the DevTools pipe; no Puppeteer, WebDriver or Synpress either. react-web's own
  Playwright tiers and the scene-kill harness are outside this decision and are not supplied
  in this public checkout. See the
  [ADR](../../../../docs/adr/2026-10-29-explorer-tests-through-the-console.md).
- **The automation surface ships in every build.** The console commands, the HUD and input
  commands natively, and the loopback `dcl/console` server behind `--automation-token` are the
  client's agent surface, so people's own agents drive the deployed clients instead of rebuilding
  the plumbing. The preview-only commands stay preview-only. Same ADR.
- **Collectible cache retention must be generic** across wearables and emotes, and an emote's
  ownership is its membership of the avatar's animation graph. A pruning PR that special-cases
  emotes (#1323) is parked for that reason.
- **Definitive failures on hash-keyed content reads are remembered for the session.** Outside
  preview, a definitive failure (404 and similar) is remembered for that URL when read by hash.
  A catalyst only activates an entity once it holds all of its files, so the answer is final.
  Other reads and failures keep the 10 s retry. Preview asks again on every read (#727).
- **A download checked against its content hash fails on a mismatch.** The failure follows
  the 10 s retry window for dropped transfers. The read that starts the download determines
  whether to check its bytes. Other reads joining the same URL share its result, which can be
  unchecked if the first read used a URL key or a name the check cannot verify. A read under an
  unverifiable content name stays uncached. The content-hash cache serves every origin, so it
  accepts only verified bytes. All 4574 sampled files from live scenes, worlds,
  wearables and emotes matched.
- **Camera layers are identified by entity**, not by an explicit layer number; the cleanup in
  #1320 waits on the protocol change.
- **`-logFile` with one dash is accepted** (rewritten to `--log-file` before clap, `main.rs`):
  the benchmark harness spells it that way. See the benchmark page.
- **The benchmark log carries markup a Rust log wouldn't**: `<color=#…>` tags and a
  `DebugLogReportHandler:LogExceptionInternal` line in every exception block, the shapes the
  harness's parser counts exceptions by. Every error-level event counts as an exception.
- **The benchmark warm-up arrives in world.** A launch without a CSV destination signs in,
  loads and only then ends, so the stored guest and the caches exist before the measured runs.
- **The benchmark guest is local.** Under `--autopilot` the guest key is made on the machine
  (or by the shell on web) and kept, instead of a thirdweb guest wallet: thirdweb refuses
  `localhost` origins, and a kept root key lets an expired identity be signed again with the same
  address. Only that guest signs in under `--autopilot`: an account stored on the machine is
  never used by a benchmark run, where scenes may fetch and sign requests without a prompt. The
  engine ends a run signed in as anything else with exit code 1, and a kept key that cannot be
  read fails the run instead of becoming a new account.
- **The benchmark's GPU time leaves out queued uploads and standalone submits.** Two empty
  compute passes bracket the frame as the first and last nodes of the render graph, so both
  timestamps reach the GPU in the frame's own submission. A bracket submitted on its own ahead of
  the frame also counts the render world's CPU time up to the submission, which on a CPU-bound
  frame is most of the sample (measurements (evidence supplied for the task)).
  See [GPU time](benchmark.md#gpu-time).
- **The profiler is opt-in.** The
  `performance-profiler` cargo feature (Bevy's `trace` spans, the scope profiler's layer, its
  console commands and the Render and Profiler debug panel sections, the counting allocator,
  Bevy's render and system-information diagnostics) is out of `default` and of the web build's features: those
  inputs cost every frame even while nothing records (process CPU per frame +2.1% with recording
  off, +9.8% with it on, native release on an Apple M4 Max), and a macOS build with the feature
  renders incorrectly, recording or not ([profiling](profiling.md)). Tracy covers native
  profiling. A build with the feature, native or web, profiles Bevy's systems. On the web,
  Bevy's `tracing-wasm` layer marks and measures every span on the page's performance timeline,
  entries nothing clears, so a web build with the feature grows the page's memory and main-thread
  time over the session; Bevy keeps that layer, since only profiling builds pay for
  it. Recording stays a runtime switch within a `performance-profiler` build. See
  [profiling](profiling.md#profiler-profile_top-count---sort-metric-profile_enable-onoffreset).
- **The analytics performance event reads the sampler's snapshot.** `PerformanceMetrics` counts
  hiccups against the sampler's threshold (the larger of 50 ms and two target frames, so 66.7 ms
  at a 30 FPS cap), takes real frame times with no 250 ms cap, and is sent once per 15 s sample,
  in world and focused only, instead of once per 1000 frames of virtual time through loading
  screens and unfocused windows. The old event's own sampling was removed rather than kept beside
  the sampler. Dashboards comparing across this change see a step from the definitions.
  The event records the applied threshold, raw hiccup counts and measured sample counts for CPU
  and, when readings are available, GPU. These counts let dashboards compute session rates.
- **Bevy stays on the 0.16 fork for now.** Security-relevant dependencies (V8 and deno,
  libwebrtc, ffmpeg, wgpu) get bumped inside it. Whether to ever move the bevy version is an
  open discussion, not a decision.
- **The HUD scales with the window, with no legibility floor.** `--ui-scale` is
  `min(width / 1920, height / 1080)` (`library/explorer-core/lib/hudScale.ts`), the same rule
  the Unity client's canvas follows, so one layout holds at every size: nothing wraps, hides or
  collides in a small window, and a 4K window gets a proportionally larger HUD. The 0.25 and 4
  bounds only keep the value finite for the layout boxes that divide by it. Known limitation:
  text shrinks with everything else. A 16 px label is about 10.7 px in a 1280x720 window and
  about 5 px at 640x480, which is not legible. A floor on the scale would trade that for a
  layout that no longer fits the window at those sizes.
- **The open chat does not fade out when idle.** Its message column stays tinted and the
  conversation rail is on screen for as long as the chat is open, with Nearby's tab on it even
  when there is no other conversation, so the chat's controls stay reachable without first
  waking the panel. Only the title bar and its navigation wait for hover or focus.
- **Places is one grid.** The page has no separate Live Now or Featured sections: every
  destination appears once, in the order of the chosen sort, and a featured place carries a
  ribbon on its card. The layout is the Unity client's, so a place is found the same way in
  either client; search, the category chips and the sort all act on the one grid.
- **Settings tabs are Graphics, Sounds, Controls, Chat; key bindings sit under Controls.** The
  row is the Unity client's, so a setting is found in the same place in either client; any other
  category the engine sends follows in the engine's order. Key bindings are (action, keys) rows
  from the bindings relay, not engine settings, so they get a card of their own below the
  Controls rows instead of a tab, and the Controls tab is there even when the engine sends no
  Controls category.
- **The rail's order is fixed.** From the top: Sidebar settings, Me, Map, Discover, Shop, System,
  Notifications; Scene permissions, Skybox, Volume, Emotes, Friends, Camera, Nearby Voice, Chat.
  Me, Discover and System open a panel of rows beside the rail: Me has Backpack, Profile
  and Badges, then Sign Out (a sign-in on the web, a guest's included) and Exit (where the page can
  close); Discover has Events, Places and Communities; System has Settings, Create, Report Bug
  and the help links. Shop opens its panel where the Shop runs in world, and the web Shop in
  a new tab elsewhere. Volume opens a panel of controls; Camera opens the rows Take A Shot and
  Gallery.
  The end-to-end suite reads these buttons by name
  (`apps/bevy-explorer/e2e/specs/navbar.test.ts`).
- **A guest signs out as unity-explorer's does.** Sign Out ends a guest's session and keeps the
  guest, and so does an account signing in. The next start does not sign the guest in on its own,
  and Play as guest brings the same guest back: on the web its kept sign-in, or its kept wallet
  session once that sign-in has run out; on the desktop client its kept wallet session, since
  Sign Out drops the sign-in the engine keeps (`/logout --forget`). The desktop client signs out
  from the start lobby only: the engine refuses a sign-out once the world is entered.
- **The Shop tab is kept.** The menu's top bar has Shop and the rail has a Shop button. Shop is
  a destination of this HUD, and matching another client's tab row is not grounds for dropping
  it.
- **A control without a function is hidden, not shown disabled.** The HUD renders a button, tab
  or menu row only when it does something. A dead or greyed control promises a function the
  HUD does not have and reads as a fault; the control appears when its function exists. The
  Portable experiences button follows the same rule and shows only while an experience is
  active.
- **The embedded assets stay in plain git, not Git LFS.** `crates/assets/build.rs` embeds whatever
  is on disk, so an LFS pointer would compile and be wrong at runtime, and an LFS checkout would
  have to be pulled by every Explorer build and matched by every check that hashes the build's
  inputs. The build fails on a file that is an LFS pointer. Revisit if the landscape folder
  (`crates/assets/src/assets/landscape`) reaches tens of MB.
- **Avatar processing has no frame budget.** `process_avatar` processes every avatar whose parts
  are written, whole, in the frame it becomes ready, outside the instantiation slice. It was
  measured before deciding (web build in Chrome, 60 fps target, 64 and 200 demo avatars in base
  wearables spawned at once and then re-dressed three times). The scene spawner writes avatar
  parts under the slice, so avatars become ready one or two a frame, at most five: the longest
  frame of avatar processing took 1.7 ms against a slice of 4.17 ms, and an avatar takes 0.35 ms
  at the median. That is well under a slice of its own, so a gate would only move a millisecond
  or two of work to a later frame. The `instantiation/avatar_processing` diagnostic and the
  Instantiation section of the debug panel show the smoothed cost; with `spawn_timing=debug` in
  the log filter, each frame that took a millisecond or more is logged. Reopen with a
  measurement of a frame of avatar processing over the slice, taken on an idle machine: with a
  compile running alongside, single avatars took up to 43 ms in frames of 100 to 500 ms. Not
  measured: what the scene spawner, glTF processing and avatar processing take together in a
  frame (avatars become ready in frames where the spawner has used the slice, so at worst about
  a slice plus the avatar work); the commands `process_avatar` queues, which are applied after
  it; wearables much heavier than the base set; a target above 60 fps (the slice is 1.7 ms at
  144 fps); and the native client.
- **`shouldReset` restarts a playing clip on every Animator write.** The protocol's comment says
  the state is restored when it goes from stopped to playing. Scenes restart a clip that is
  already playing by writing it again with `playing` and `shouldReset`
  (`Animator.playSingleAnimation`), which Unity's player for legacy clips honours, so this
  engine does too. A
  stopped state keeps its clock whatever its `shouldReset` says: `Animator.stopAllAnimations`
  leaves the flag set, and a later `playSingleAnimation(..., false)` must resume. A state
  with `playing` unset does not play.
- **A clip played once tells its scene when it ends.** The engine writes the scene's Animator
  back with that state's `playing` false, once, as the Unity client does, so a scene need not
  time the clip. The clip then stops contributing and the model holds the pose it ended on.
  Without the report the scene's own state would still say `playing`, and its next write about
  any other clip would play the finished one again. A clip can end while its scene's tick is
  running, and that tick's write still says `playing`: the engine keeps the clip ended, applies
  the rest of the write and sends the scene the result before its next tick, so the scene
  always reads the end. To play the clip again, the scene writes `playing` after it has read
  the end. A model is watched for an end only while its Animator plays a clip once.
