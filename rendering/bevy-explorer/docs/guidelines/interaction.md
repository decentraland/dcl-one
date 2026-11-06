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
| Cursor ray | 100 m from the camera; a scene entity farther away is never hovered or clicked, whatever its `max_camera_distance` | `CURSOR_RAY_MAX_DISTANCE` |

The trigger-area ray (`update_world/trigger_area.rs`) does not use the 100 m limit.

## Scene raycasts

Every scene's raycast requests share one time budget per frame (`update_scene/raycast_budget.rs`,
`raycast_result.rs`).

- **Budget.** `raycast_budget_ms` in `AppConfig`, 2 by default, an integer from 0 to 10. It is the
  "Raycast budget" setting (Performance category) and the `/raycast_budget [ms]` command: without
  an argument it answers `raycast budget: <n> ms`; a value over 10 fails with `raycast budget must
  be between 0 and 10 ms`; a larger value written into the config file by hand counts as 10.
- **Order.** Requests due this frame run in this order: requests deferred for 60 frames or more
  first (the longest-deferred first), so a scene that fills the budget every frame cannot starve the
  others; then the player's scene, then lower scene priority (the nearer scene), then requests that
  never ran, then the longest-waiting, then entity id so the order is stable.
- **Deferral.** Casting stops once the time spent reaches the budget. A cast that starts under
  budget finishes, so the last one may cross the line. The requests left wait for a later frame;
  none is dropped.
- **At least one per frame.** A budget of 0 runs one raycast per frame.
- **`/raycast_stats`.** JSON for the last frame that had at least one raycast to cast (frames
  between scene ticks usually have none and leave it as it was): `budgetMs`, `ran`, `deferred` and
  `spentMs`.
- **Debug panel.** The "Raycasts" section shows the same four values and has a number input for
  the budget (0 to 10) that sends `/raycast_budget`.

### Unity reference

unity-explorer casts one 100 m camera ray per frame, shares a 3 ms raycast budget between scene
worlds, runs scene raycasts for the current scene only, and defers the ones that do not fit. Ours
differ on purpose: see [Decisions](decisions.md).

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

`/click_entity <entity> [--bounds] [--button pointer|primary|secondary] [--event click|down|up]
[--timeout-frames N]`:

| Option | Meaning | Default |
| --- | --- | --- |
| `--button` | The action to press, by its binding | `pointer` |
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
| `beyond 100 m` | No collider hit, and the aimed point is past the 100 m cursor ray |
| `entity not live` | The scene has no live entity for the id, or it was replaced while the job ran |
| `input held by the HUD` | The HUD holds the bound input, so the press would not reach the scene |
| `timed out waiting for <stage>` | The job passed its deadline; `<stage>` is `target` (the ray hit nothing within 100 m before the deadline: no collider, or one still loading), `hover` (a hover did not finish), `tick` (the scene never logged the press) or `release` (the release was never played) |

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
