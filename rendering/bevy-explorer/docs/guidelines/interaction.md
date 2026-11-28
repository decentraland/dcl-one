# Interaction

How the pointer and the scene raycasts decide what a scene can reach, and how a test drives them.
Paths are relative to `source/`; the pointer code is in `crates/scene_runner/src/update_scene/`.

## Distances

| What | Value | Where |
| --- | --- | --- |
| Default reach of a pointer entry | 10 m from the player, when the entry sets neither `max_distance` nor `max_camera_distance` | `pointer_results.rs`, `passes_distance_check` |
| `max_player_distance` | A deprecated alias of `max_distance`; when both are set the larger wins | `pointer_results.rs`, `max_player_distance` |
| `max_camera_distance` | Compared with the distance from the camera to the hit point. With `max_distance` set too, either one passing is enough (OR) | `pointer_results.rs`, `passes_distance_check` |
| Proximity reach | 3 m from the player, for a PROXIMITY entry that sets no distance | `PROXIMITY_DEFAULT_MAX_DISTANCE` |
| Proximity cone | 120 degrees wide, horizontal, around the body's forward direction | `PROXIMITY_FOV_ANGLE_DEGREES` |
| Scenes considered | The scenes within 16 m of the player (`get_area(player, PARCEL_SIZE)`) | `pointer_results.rs`, `update_pointer_target` |
| Cursor ray | Unbounded, and cast against each scene considered only where that scene is drawn (over its parcels, up to their height limit): it ends at the first such hit, and the entry's distances decide whether that hit is in reach | `pointer_results.rs`, `update_pointer_target` |

## Scene raycasts

Scene raycast requests run in two tiers each frame (`update_scene/raycast_budget.rs`,
`raycast_result.rs`).

- **Uncapped.** Requests of the system scene (`SuperUserScene`), every portable and the scenes the
  player stands in (`get_area(player, PLAYER_COLLIDER_RADIUS)`) all run every frame they are due.
  Scenes drive movement from their results (`movement-scene` casts continuous `RS_AVATAR` rays for
  ground and walls), so these never wait. They cast together, on the compute pool once eight or
  more are due (see [scheduling](scheduling.md)).
- **Capped.** Requests of every other scene run nearest scene first (`RendererSceneContext::priority`),
  then the one that ran longest ago (never-run first), then entity id, while the time spent is
  under the budget. A cast that starts under budget finishes, so the last one may cross the line.
  The rest wait for a later frame. When the player walks into a capped scene it becomes uncapped,
  so its requests run then.
- **World casts are cut per scene.** A cast that includes the world (`include_world`, or any cast
  of a portable) meets each scene it crosses only where that scene is drawn: over its parcels, up
  to their height limit. The caster's own colliders are not cut, in a world cast or in a cast
  at its own scene. See [Decisions](decisions.md).
- **Budget.** A tenth of the target frame: `SceneUpdates::frame_target`, which `run_scene_loop`
  sets from the fps target (with the session override), capped at the refresh rate under vsync.
  That is 1.67 ms at 60 fps, 0.69 ms at 144 and 0.42 ms at 240. It follows the target, not the
  measured frame, so a slow frame does not raise the next one's budget. Raycasts run before the scene loop, so they use
  the previous frame's target. With an uncapped frame rate (fps 0) it is a tenth of a 60 fps frame.
- **`/raycast_budget [ms|auto]`.** Without an argument it answers
  `raycast budget: <ms> ms (auto|override)`. A value from 0 to 10 overrides the capped budget for
  the session (0 defers every capped request); `auto` returns it to the frame share. Anything else
  fails with ``raycast budget must be `auto` or between 0 and 10 ms``. It is not a setting.
- **`/raycast_stats`.** JSON: `budgetMs`, and for the capped tier's last frame with a request to
  cast `ran`, `deferred` and `spentMs`; for the uncapped tier's last such frame `uncappedRan` and
  `uncappedSpentMs`. A frame with nothing due in a tier (most frames between scene ticks) leaves
  that tier's values as they were.
- **Debug panel.** The "Raycasts" section shows those values, has a number input that overrides
  the capped budget (0 to 10) and an "Automatic budget" action that sends `/raycast_budget auto`.

### Unity reference

unity-explorer casts one 100 m camera ray per frame, caps scene raycasts at 3 ms, runs them for the
current scene only, and defers the ones that do not fit. Ours differ on purpose: see
[Decisions](decisions.md).

## Driving the pointer

`/hover_entity` and `/click_entity` (`src/automation/pointer.rs`, `src/automation/pointer/machine.rs`)
hover and click a scene entity of the active scene by id through the production pointer
pipeline, on native and web alike. The scene is the one `/set_scene` picked, else the player's.
A command sets a synthetic aim (`SyntheticPointer`, `synthetic_pointer.rs`): the pointer ray goes
from the camera to the entity's anchor point whatever the cursor does, and runs through the same
casts, distance checks and result writers as a real cursor. A click presses the key or mouse
button bound to the action through the input script, so the press reaches the scene as a
person's would.

On the web these commands reach the engine through `engine_console_command` and write Bevy
input events. Unlike `/press_key` and `/click_at`, they do not go through the page's trusted
input (`window.__dclTrustedInput`), so they need no DevTools client.

### Options

`/hover_entity <entity> [--bounds] [--frames N] [--timeout-frames N]` and `/hover_entity --clear`:

| Option | Meaning | Default |
| --- | --- | --- |
| `<entity>` | An id as `/scene_entities` lists it, or `root`, `player`, `camera` | required, unless `--clear` |
| `--bounds` | Aim at the centre of the bounds of the visible meshes of the entity and its descendants; the origin when it has none | the entity's origin |
| `--frames` | How long to hover; at most 1800 (more is refused) | 30 |
| `--timeout-frames` | Frames the job may take in all. A hover's deadline is `max(timeout, frames + 3)`, so the default never cuts a long hover short | 600 |
| `--clear` | End any aim and release a held press, instead of hovering | off |

`/click_entity <entity> [--bounds] [--button <action>] [--event click|down|up] [--timeout-frames N]`:

| Option | Meaning | Default |
| --- | --- | --- |
| `--button` | The scene input action to press, by its SDK name in lowercase without `IA_`: `pointer`, `primary`, `secondary`, `forward`, `backward`, `right`, `left`, `jump`, `walk`, `action_3` to `action_6`, `modifier`. The command presses the key or mouse button bound to it, so the scene gets the press as that action (`events[].button`, e.g. `IaJump`). The avatar reads the same bindings, so `forward`, `backward`, `left` and `right` also move it, `jump` jumps and `walk` and `modifier` change its gait, and under `--event down` it keeps moving until the `up`, which changes `playerDistance` while the press is held | `pointer` |
| `--event` | `click` presses, then releases on a later scene tick; `down` presses and holds; `up` releases a held press | `click` |
| `--timeout-frames` | Frames the click may take in all | 600 |

### The reply

```json
{"hit": {"entity": 512, "container": 512, "scene": "pointer-targets", "playerDistance": 4.0, "cameraDistance": 6.2},
 "entries": [{"button": "IaPointer", "event": "PetDown", "hoverText": "Near", "enabled": true, "tooFar": false}],
 "events": [{"event": "PetDown", "button": "IaPointer", "tick": 31}],
 "diagnosis": null}
```

- `hit`: what the pointer pipeline targeted, or `null`. `entity` is the hit entity and
  `container` the entity that owns the collider. A hit that is no entity of the scene (an avatar,
  another scene) reports id 4294967295 for both. `scene` is the hit scene's title.
  `playerDistance` is from the avatar to the nearest point of the collider, `cameraDistance`
  from the camera along the ray.
- `entries`: the cursor pointer-event entries of the hit container, each with `button`, `event`,
  `hoverText`, `enabled` and `tooFar` (the distance check failed).
- `events`: the pointer events the scene received for the entity or the hit entity, each with
  `event`, `button` and the scene `tick` it was written for.
- `diagnosis`: `null` when the entity was hovered or clicked, otherwise one of the strings below.

### Diagnoses

A diagnosis is a resolved reply with `diagnosis` set, not a failure: read it from the JSON.

| `diagnosis` | Meaning |
| --- | --- |
| `blocked by entity <id>` | The ray hit another entity's collider first; `<id>` is that container |
| `entity not live` | The scene has no live entity for the id, or it was replaced while the job ran |
| `input held by the HUD` | The HUD holds the bound input, so the press would not reach the scene |
| `timed out waiting for <stage>` | The job passed its deadline; `<stage>` is `target` (the ray hit nothing before the deadline: no collider, or one still loading), `hover` (a hover did not finish), `tick` (the scene never logged the press) or `release` (the release was never played) |

### Refusals

A refusal fails the command (the web call rejects, the loopback answers `ok: false`):

- a pointer command is still running, or its release has not been played yet: wait for its reply
  or send `/hover_entity --clear`;
- a press is held by an earlier `--event down`: send `/click_entity <id> --event up` or
  `/hover_entity --clear`; an `up` with nothing held fails with `no press is held`;
- `--frames` over 1800, an unknown `--event` or `--button`, a button with no binding, or a
  button bound to a gamepad input;
- neither an entity nor `--clear`, or an id that does not parse.

### The open press

`--event down` presses and keeps the aim open on the entity (no expiry) once the scene has logged
the press. The press stays held until `--event up` on the same entity or `/hover_entity --clear`;
other pointer commands are refused meanwhile. A hover or a click ends its aim when it replies.
When a press is still held at the end (`--clear`, or a click that failed after pressing), the
release is played on the aimed entity first: the reply, and the end of the aim, wait until it has
been played and the pointer pipeline has run after it, and other pointer commands are refused
until then.

## Reading the pointer target

`/debug_pointer [true|false]` (preview only, `pointer_results.rs`) shows or hides the pointer
target in the debug overlay, toggling it without an argument, and replies, as JSON, with what the
pointer targets now. It reads the pipeline's last target and ray, so it presses and hovers
nothing; under a `/hover_entity` or `/click_entity` aim it reads that aim's target.

```json
{"kind": "scene_entity", "container": 512, "scene": "pointer-targets", "mesh": null,
 "inPlayerScene": true, "hitPoint": {"x": 8.0, "y": 1.0, "z": 6.0},
 "playerDistance": 4.0, "cameraDistance": 5.22,
 "hasPointerEvents": true, "withinRange": true, "hoverText": "Near",
 "entries": [{"button": "IaPointer", "event": "PetDown", "hoverText": "Near", "enabled": true, "tooFar": false}],
 "cursorLocked": true, "rayFromScreenCenter": true, "aimedByCommand": false,
 "ray": {"origin": {"x": 8.0, "y": 2.5, "z": 1.0}, "direction": {"x": 0.0, "y": -0.2873, "z": 0.9578}}}
```

- `kind`: `scene_entity` (a collider of any scene, the player's or a neighbouring one), `ui` (a
  scene UI element), `avatar` or `nothing`.
- `container` and `scene`: the scene entity that owns the target, named as in `/click_entity`'s
  `hit`, and its scene's title, or `null` (an avatar); `mesh` names the collider mesh hit, when it
  has a name.
- `inPlayerScene`: the player stands in the target's scene. Cursor entries reach only such a
  target, so a neighbouring scene's entity reads `false` with every entry out of reach and not
  `tooFar`.
- `hitPoint` and `ray` are in world coordinates, as `/world_state` reports them; `hitPoint` is
  `null` for a UI target. `playerDistance` and `cameraDistance` are measured and named as in
  `/click_entity`'s `hit`, rounded to the centimetre.
- `entries` are the target's cursor entries, as in `/click_entity`'s reply; `hasPointerEvents`
  says it has any, `withinRange` that one is in reach, and `hoverText` is the text of one in reach
  before one out of reach.
- `cursorLocked`: the camera holds the cursor; `rayFromScreenCenter`: the ray goes through the
  screen centre (locked, and no command aims); `aimedByCommand`: a pointer command's aim drives
  the ray. `ray` is `null` when the pipeline's last pass cast none: no camera or player, the
  cursor outside the window, or an aim at an entity not placed yet.

The reply reads the pipeline's last results without waiting for a fresh pass. The ray is cast
early in the frame and the target resolved with the scene input, which the command is not ordered
against, so the target can trail the ray by a frame. With the cursor outside the window, `ray` is
`null` while the target keeps the last one it had.
