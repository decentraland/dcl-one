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
  friendship buttons; its ⋮ opens unity-explorer's member menu: View Profile, Chat, the owner's
  and moderators' actions (Demote or Promote Moderator, Make Community Owner, Remove Member, Ban)
  with unity-explorer's rules and confirmations, then Block and Report. Call, Gift and Invite to
  Community are left out until private calls, gifting and invites exist (unity-explorer shows
  Gift only behind its gifting flag). Chat is always offered in-world: unity-explorer greys it
  only while its chat area is down. unity-explorer's row shows no presence, so neither does
  this one; "N Mutual Friends" is not shown because no members API returns the count. A failed
  action shows its message on the card instead of as a toast, and the tab lists the first 100
  members without unity-explorer's paging.
- **Pulse only.** unity-explorer falls back to LiveKit-only after five failed start-up attempts,
  through dual-transport proxies and LiveKit broadcast targeting. Here Pulse is the only
  avatar-state transport and none of that exists, so a retryable failure is retried without limit.
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

## Deliberate choices that look like mistakes

- **`UnapprovedPathMode::Allow`** in the asset server: bevy's check only admits paths under
  the executable's assets directory and the IPFS cache lives elsewhere. Containment is done
  in `ipfs_path.rs` instead. See the security page.
- **Web-only engine switches are page params**, not `LaunchOptions` fields.
- **`CommsApi.consumeMessages` resolves to a bare array** of `{ sender, data }`, not the
  protocol's `{ messages }`, because unity-explorer, the client scenes are tested on, returns
  the array. Changing it to the protocol's shape would break those scenes here.
- **A scene's messages need a connected scene room that carries the scene.** The message bus and
  topic messages, sent and received, and the `isConnectedSceneRoom` flag in `RealmInfo` all ask
  `comms::scene_rooms::SceneRooms`. A message sent before the room connects is dropped, not
  queued; the flag turns true when sending starts to work. A client's room carries the scene the
  player is in. In a local preview or a world with one scene it carries every scene of the realm.
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
