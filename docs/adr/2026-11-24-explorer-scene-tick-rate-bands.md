# ADR: The Explorer caps each scene's tick rate by distance band

- Status: Accepted
- Date: 2026-11-24
- Supersedes: [nearest-first priority](2026-11-02-explorer-nearest-first-priority.md), for its
  rejection of a tick-rate table and of a cap behind the camera; its partition, tick key and
  download ordering remain.

## Context

The scene loop sends the next tick to the eligible scene with the lowest key, squared distance
over time since last sent, four times farther behind the camera, under the scene thread limit.
A far scene still ticks whenever it wins a slot, so every scene in load range costs scene-thread
and main-thread time each frame there is spare time, whether or not the player can see it change.

## Decision

- **A maximum rate per distance band.** `scene_tick_rate` reads a scene's `Partition`: under
  16 m from the player to its nearest parcel, 40 ticks a second; under 32 m, 30; under 48 m, 15;
  under 64 m, 10; under 80 m, 5 (`SCENE_TICK_RATE_BANDS`); from 80 m on, once every two seconds
  (`FAR_SCENE_TICK_RATE`). Distance is the partition's, from the player to the nearest point of
  the scene's nearest parcel.
- **Behind the camera, at most once a second** (`BEHIND_SCENE_TICK_RATE`), from 16 m on and
  while the camera is near level (`behind_cap_applies`). In the nearest band a scene keeps its
  rate behind the camera: the player can back into its colliders, such as doors and moving
  platforms, and hears its spatial audio, which would otherwise move in steps of up to a
  second. The behind test compares parcels with the flat forward direction, so looking steeply
  up or down it can call a scene behind while its content is on screen.
- **The farthest band keeps a slow tick, and unread global updates make a scene due.** The
  engine sends every global update (player joins, leaves, identities, avatars, movement) to each
  scene's host as it happens: the IPC forwarder on native, the relay pump on the web. The host
  holds them for the scene in a channel of 1,000 entries (`SCENE_GLOBAL_UPDATE_CAPACITY`, 1,024
  after tokio's rounding), which the scene drains only at a tick, and past which the oldest are
  dropped with no resynchronization: a lost join or leave shows a missing or ghost player when
  the player returns. A busy island can send more than that in two seconds, so a rate alone
  does not bound the loss. `GlobalCrdtState::sent` counts the updates sent to the context's
  hosts. Each dispatch records the count of the scene's own context
  (`CrdtContexts::try_for_scene_hash`) on the scene (`global_updates_read`), and a capped
  scene with `GLOBAL_UPDATE_BACKLOG_DUE` (250) or more delivered since is due whatever its band.
  Only that scene is released. An update is lost only after more than 1,024 arrive before the
  scene ticks, 774 more than the 250 that make it due. The scene is due
  once 250 are unread, but it is then ordered by the queue's key like any other, so under
  scene-thread pressure it can wait frames while more arrive; the margin holds while it is sent
  within those 774. The count is taken when an update enters the broadcast, not when the host
  receives it: the native forwarder interleaves ticks and queued updates in either order, so a
  tick can reach the host before updates already counted as read, which stay unseen by the
  check until the next dispatch. The margin absorbs that unless the forwarder is far behind.
- **A scene subscribes to global updates at spawn.** Its initial store is seeded with the
  global state when it starts loading, and its receiver is taken when it spawns, after its
  script has downloaded, with the store brought to the global state of that moment: the players
  who left meanwhile removed, newer components taken (`refresh_global_state`). The time of day
  and the camera FOV travel only as updates, not in the global state, so the context keeps the
  last of each and sends them again at the subscription
  (`GlobalCrdtState::subscribe_with_time_and_fov`): the new scene starts with both, and the
  scenes already live receive the same values again. The native
  forwarder, which swaps one receiver per context and forwards what the new one holds to every
  scene of the context, then has only the updates sent between the subscription and its swap to
  forward again, which the scenes apply idempotently. Subscribing at load start would hold every
  update sent while the script downloads, up to 1,024, and send them to every live scene a
  second time, past the count above.
- **Unread peer messages make a scene due too.** A scene's inbound message queues, the string
  and binary buses (1,024 each) and the topic inbox (630, `TOPIC_INBOX_CAPACITY`), also drain
  only at a tick, and drop new messages when full. `SceneMessagesDelivered`, on the scene root,
  counts the peer messages handed to the scene's queues; each dispatch records it
  (`scene_messages_read`), and a capped scene with `SCENE_MESSAGE_BACKLOG_DUE` (157, a quarter of
  the topic inbox) or more since is due whatever its band. The count is summed over the three queues, so the release comes no
  later than any one queue's quarter.
- **A rate is a minimum interval.** `update_scene_priority` leaves a capped scene out of the
  frame's queue until `1 / rate` seconds have passed since it was last sent. A scene that is due
  is ordered by the existing key, with its age taken from when its interval passed, at least a
  frame (`queue_age`): holding a scene back does not rank it as starved, so a scene behind the
  camera still yields first. A scene released on an unread backlog keeps its age since it
  was last sent. Age grows a frame at a time, so
  the interval counts as passed within a quarter of a frame of it: jitter cannot miss an
  interval of whole frames, and one of a frame and a half always waits for the second frame. A
  rate is therefore rounded to whole frames and can be passed by up to a quarter of a frame: at
  60 fps the 40 and 30 bands both tick every other frame, and between 45 and 50 fps the 40 band
  ticks every frame, 45 to 50 times a second.
- **Full rate.** The player's scenes, the scenes at the player's position while out of world,
  portable and global scenes, the named priority scenes, the system scene, a scene under the
  inspector, the scene being baked into an imposter, and every scene before
  `SCENE_LOADED_TICK` (so loading, which waits on a scene's first ticks, never stalls on a slow
  band).
- **Only where a camera renders around the player.** Without a `PrimaryCamera` that has a
  `Camera`, as on the headless authoritative server, no scene is capped. Neither is one while the
  photo camera frames a shot (`PhotoMode`), which can sit up to 50 m from the player, or while a
  scene's cinematic camera (`CameraOverride::Cinematic`) does, which has no distance limit:
  bands measured from the player would cap the scenes they frame.
- **Inspectable.** Each scene's context holds its current cap (`tick_rate_cap`), and
  `SceneUpdates::skipped_rate_capped` counts the scene-frames the cap skipped and
  `released_by_backlog` the ticks sent before their band allowed on unread global updates or
  peer messages; the runtime debug
  panel's "Scene Tick Rates" section (`/scene_tick_rate_state`) shows them, whether the behind
  cap applies at the camera's pitch, and why nothing is capped when that is so
  (`tick_rate_caps_lifted`). `/scene_tick_rate_caps false` lifts every cap, so one build
  compares frame time with and without them.

## Consequences

- Scenes from 80 m on change at most every two seconds; their SDK systems and tweens driven
  from script step in large intervals until the player comes nearer.
- A scene 16 m or more behind the camera delivers up to a second of changes in one reply.
  Whether that hitches when the camera turns is still open: Genesis Plaza, measured below,
  caps no scene, so it needs a spot where the neighbouring scenes run.
- Walking through a busy area, the global-update release can take back part of the saving. At
  Genesis Plaza it released no tick.
- **Measured at Genesis Plaza** (2026-11-27): release builds of `main` and this change, native
  and web, two runs each, a guest with fetch and websocket permissions preset. Three scenes run
  there, the player's scene and two exempt scenes 18 to 22 m away, all at the full rate (the
  panel reads `full` 3 and no capped scene); the plaza's neighbours do not start. The change
  measures as neutral:
  - ticks per scene per second over 60 s standing: the plaza 53.5 on `main` and 55.5 with the
    change on native, 25.0 and 25.0 on web; the other two 60 on native and 50 on web, on both;
  - the main world's mean frame time 15.6 to 15.7 ms native and 12.7 ms web on both; the scene
    loop's 5.1 ms on `main` and 5.4 ms with the change native, 1.4 ms web on both;
  - turning the camera at 90°/s, frame p99 23 ms native and 36 ms web on both; the worst frame
    44 to 64 ms on web on both, and on native 26 to 27 ms with the change against 314 and
    542 ms single frames on `main`, too few runs to attribute;
  - walking 17 to 52 m through the plaza, `released` stayed 0 and `skipped` reached 224
    scene-frames, from one scene that started on the way.
- On the client every scene shares one crdt context, so the global-update release follows the
  island's update rate: at about 500 updates a second every capped scene is due about every
  half second, and the 1/s and 0.5/s caps act as about 2/s where the island is busiest. A scene
  receiving peer messages is likewise due at least every 157 of them. The panel's `released`
  next to `skipped` shows how much of the cap a spot keeps.
- A capped scene loses peer messages only when more than the remaining room in a queue (at
  least 473 topic messages, or 867 bus messages) arrive between its release and its dispatch.
- The photo and cinematic exemptions lift every cap, not just the scenes the camera frames. A
  scene that keeps a cinematic camera or camera-mode area active while the player is inside it
  ticks the whole world at the full rate for that time. Measuring the bands from the rendering
  camera while it is away from the player would keep the caps there.
- A scene's imposter or LOD stand-in goes once it passes tick 5, before the cap starts at
  `SCENE_LOADED_TICK`. A far scene that builds its content over many ticks shows it built at one
  tick every two seconds.
- A pointer result reaches a capped scene at its next tick: a click on a scene 80 m or more away,
  or on one behind the camera inside a wide field of view, answers up to two seconds later.
- A scene sends at most `MAX_SEND_MESSAGES_PER_TICK` (512) messages a tick, so a capped scene
  that sends from timers between ticks is limited to 512 a second behind the camera and 256 past
  80 m; the excess fails in the op.
- The behind test compares parcels with the camera plane only, so a scene 16 m or more behind a
  level camera can cast shadows into view that move once a second.
- Bands and rates are code constants, not settings; the switch only turns them all off.

## Rejected

- **Keeping only the aging key.** It shares spare frame time by distance but never removes far
  ticks, so far scenes still take scene threads and main-thread receive and apply time.
- **Capping the player's scene or portables.** They are what the player interacts with, and
  their tick rate is the input latency the player feels.
