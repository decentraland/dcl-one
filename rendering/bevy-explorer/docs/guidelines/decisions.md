# Decisions

Things chosen on purpose in a way that looks like an oversight, or that deliberately differ from
the Unity client. Check here before proposing a change in one of these areas. Each entry says
what was decided and why; the PR or issue has the detail. Add to this page whenever a review
ends in "we don't do that", so the next person doesn't have to rediscover it.

## Deliberate divergences from Unity

- **Toon MASK wearables.** Unity alpha-blends after the cutoff, so thin masked lines look
  fine there and fat and black here. Bevy stays glTF-spec-correct (MASK is binary). We don't
  match.
- **Day and night (#1467).** An explicit, stopped HUD time beats scene time; restarting the
  clock hands control back to the scene. No HUD lock while a scene controls the sky.
  `transitionMode` in scene.json is ignored. HUD and `/time` skips take the nearest
  direction; SDK `SkyboxTime` keeps its own mode. `/time` reports the effective time.
- **Emissive strength.** Unity never parses `KHR_materials_emissive_strength`; bevy honours
  it, spec-correctly. Content authored against Unity can glow (the Nekko cat at 65x). Dropping
  the extension is not the fix; the long-term answer is still open.
- **Always HDR.** Unity's Low and Medium render without HDR, to save bandwidth on an 8-bit
  target. bevy keeps HDR on every preset: the primary camera is spawned once with `hdr: true`
  (`src/lib.rs`) and no setting reaches it, and the post stack assumes the 16-bit float target.
  bevy's bloom only blooms values above 1.0, so it does nothing on an 8-bit target; the lens
  flare, the stars and the avatar glow composite pick their pipeline from `view.hdr` and were
  tuned on the HDR path; tonemapping and the sky envmap feed the same target. A per-preset
  toggle would recreate the camera and its target on every preset change. Unity's Medium has
  bloom enabled with HDR off, so it is effectively invisible there; our Medium maps bloom to Off
  to reproduce what Unity shows rather than the checkbox. This is a look-parity call, not a
  measured one: the cost of the 16-bit target on Low-class machines has not been measured, and
  the render scale is the cheaper lever already in the presets.
- **Lens flare before depth of field.** bevy blooms before DoF, so the flare runs straight after
  the main pass to stay ahead of bloom, as in URP, and DoF blurs it when DoF is on. Also:
  - the ring elements are procedural (Unity's ring textures were not carried) with no radial
    distortion;
  - random copies use a hash, not `UnityEngine.Random`;
  - occlusion uses the directional light's radius;
  - the flare follows the directional light, so at night it rides the moon;
  - only the player's camera flares, and there is no swap dip.
- **Jump marker is a quad.** A ground-aligned quad, not a decal projector, because bevy's forward
  decal needs the depth prepass, which is optional here (on only with the avatar outline or
  SSAO). It is the blob variant (`BlobShadow.mat`) and its ramps follow the shadergraph.
- **Ocean.** Water depth comes from the coast profile, not the depth buffer; the foam is
  procedural surface foam with no intersection term; the waves fade out by 500 m and a flat plane
  1 m below takes over; the ripple normals are ours; lighting stays bevy PBR although
  `Ocean.mat` is `_UNLIT`; refraction has no depth test; the refracted scene is fogged twice.
- **Imposter cross-fade blends.** Imposters fade with alpha blending over 0.5 s, not blue-noise
  dithering; the live scene is not dithered; the floor quad and bigger-mip replacements pop.
- **Presets on the web and vSync.** The web has no MSAA, so web Medium and High use FXAA (High);
  vSync has no HUD setting and defaults on; loading and population settings are outside the
  presets; DoF and SSAO stay available but are Off in every preset because Unity has neither.
- **Render scale compositor.** The world always renders to an image that a 2D camera composites
  onto the window: a plain copy at scale 1, FSR EASU below it; bevy's CAS at 0.92 stands in for
  RCAS; the UI rides the compositor at full resolution; `/screenshot` captures the window.
- **Sky preset curves are linear.** The fog and flare keys are interpolated linearly; Unity's
  curve tangents are not carried.
- **Scene and download priority without distance buckets.** unity-explorer ranks scenes and
  asset loads by distance buckets and a bucket-indexed tick-rate table. We keep the aging
  scheduler and measure raw squared distance from the player to the nearest point of a scene's
  nearest parcel (not its corner); a scene behind the rendering camera counts four times
  farther in the tick queue (no once-a-second cap, no frozen far scenes). Request slots go to
  the head of the best-ranked queue (one per scene or wearable hash, and one for requests with
  no hash): the player's scenes, then avatars' wearables, then the other scenes, the last two by
  parcel ring, in front before behind, then raw distance; then requests no scene or avatar owns
  (no hash, or a hash nobody holds), oldest first; last, the downloads of a scene that has left.
  Scene roots and avatars register their hashes through component hooks; the publisher writes
  the values in place. See the
  [ADR](../../../../docs/adr/2026-11-02-explorer-nearest-first-priority.md).

## Deliberate choices that look like mistakes

- **`UnapprovedPathMode::Allow`** in the asset server: bevy's check only admits paths under
  the executable's assets directory and the IPFS cache lives elsewhere. Containment is done
  in `ipfs_path.rs` instead. See the security page.
- **Web-only engine switches are page params**, not `LaunchOptions` fields.
- **Native HUD preferences are persisted by the engine** (`hud-prefs.json`), because CEF's
  `localStorage` does not survive.
- **Web is Chrome only.**
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
- **Bevy stays on the 0.16 fork for now.** Security-relevant dependencies (V8 and deno,
  libwebrtc, ffmpeg, wgpu) get bumped inside it. Whether to ever move the bevy version is an
  open discussion, not a decision.
