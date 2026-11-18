# ADR: Spring bones are simulated on the ten nearest avatars, in fixed steps

- Status: Proposed
- Date: 2026-11-16

## Context

A wearable's skin lists the avatar skeleton's bones and may list bones of its own: strands of
hair, a tail, the panels of a cape. A wearable can declare how each of those swings in its entity
metadata, under `data.springBones`: per model file (keyed by the file's content hash), a map of
bone name to `stiffness`, `drag`, `gravityDir` and `gravityPower`. The metadata is the contract
with creators and with every client that renders the wearable, so the motion it describes has to
look the same wherever the avatar is seen. Nothing about it is sent between clients: each one
simulates what it shows.

The Explorer binds a wearable to the body by name. `process_avatar` points each skin joint at the
body's bone of the same name and removes the wearable's own copy of the skeleton. A wearable's own
bones were kept only when they were joints hanging directly off a body bone, and nothing moved
them, so they rendered rigid.

Three constraints shape the design:

- The bones hang off body bones that animation, player movement and IK move in `PostUpdate`. The
  stored `GlobalTransform`s there date from before that movement.
- The simulation integrates positions. Run once a frame with the frame's time, it would swing
  differently at 30, 60 and 144 fps, and a long frame would throw the bones.
- Every simulated chain costs a walk up the hierarchy and a few steps of arithmetic a frame, for
  every avatar in view, on the web build's single thread as well.

## Decision

**Chains are found when the avatar is built and live on it.** `spring_bones::wearable_chains`
reads the joints of the wearable's skins, each once; the order they are listed in changes nothing.
A joint not named `avatar_*` is a spring bone, declared or not. One whose parent is a body bone
starts a chain anchored to that bone, and the chain runs down from parent to child. Where a bone
has several spring bones under it, the first by name carries the chain on and each of the others
starts a chain of its own, anchored to that bone and listed after the chain it branches from. A
chain simulates its first 12 joints. A bone takes its own declared parameters, else those of the
nearest bone above it that declares some, else stiffness 0.5 and drag 0.4 with no gravity. The
result is a `SpringChains` component on the avatar's model entity, so a rebuilt avatar starts
with fresh chains and a removed one takes its own with it. `process_avatar` moves each branch of
a wearable's own bones under the body bone it hangs off, so the chain root follows the body
through the ordinary hierarchy.

**The simulation runs in fixed steps of 1/60 s.** A frame runs the steps that fit the time since
the last, at most four, and shows each bone blended between its last two steps. The anchor bone
is moved from last frame's pose to this frame's across the steps. Chains are stepped in order, and
a chain that branches from another takes its anchor from the joint as just stepped. A frame longer than four steps
is not simulated: the bones hold their pose, and the next frame starts them at rest where the
avatar then stands. The local player's teleport does the same. Only rotations are written, so
bones never stretch.

**At most ten avatars are simulated** (`MAX_SIMULATED_AVATARS`). The local player ranks first,
then avatars by squared distance from the primary camera. Avatars behind the
camera, and those the avatar limit hides, are left out. An avatar that drops out has its bones
put back at rest.

**It runs in a new `PostUpdateSets::SpringBones`,** after `InverseKinematics` and before
`Nametag`. `simulate_spring_bones` computes the poses it needs from local transforms with
`TransformHelper`, as the IK systems do, and writes local rotations before the final propagate.

**One setting turns it on:** `GraphicsSettings::spring_bones`, shown as "Jiggle Bones". It is off
by default and in the Low preset, and on in the Medium and High presets.
A config saved before the presets carried the setting keeps it off: one that matched Medium or
High then reads as Custom until the player picks the preset again.

`gravityDir` is authored for a left-handed world and is read as `(x, y, -z)`, the mapping scene
transforms use.

## Consequences

- Motion does not depend on the frame rate: every frame at 30, 60 and 144 fps shows a point of
  the same swing (`spring_bones::sim` tests).
- Below 15 fps every frame is longer than four steps, so spring bones hold the pose of the last
  simulated frame, wherever in the swing that was, until a shorter frame starts them again at rest
  in place. They are not put back at rest meanwhile: that would jump on a single long frame.
- An eleventh avatar's hair is rigid. The cap bounds the cost whatever the crowd.
- A wearable's own bones that hang off another of its own nodes, not a body bone, are now kept
  with the avatar; before, they were removed with the wearable's skeleton.
- A fork is solved under the bone it forks from, a frame's steps after that bone: its anchor moves
  from the bone's pose at the end of the last frame to its pose at the end of this one.
- `/spring_bones` and the Spring bones debug section report the chains found and the avatars
  simulated.

## Measured cost

Measured on the web build on 2026-11-18, with CI's release builds (wasm-opt): main at `0180c200a`
against a build whose presets carry the setting. Chrome 157 on an Apple
M1 Pro (macOS 26.5), a 1600×900 window at a device pixel ratio of 2, a new account with the Medium
preset picked in Settings, so main ran with the setting off and the change with it on. Each build
then ran the same scenes with the setting turned the other way by hand. Three runs of each build,
interleaved; a reading is 45 s of sampled frames, about 1,000, from `/performance_state`.

The scenes are in a loopback realm and hold 12 or 14 `AvatarShape` avatars in live wearables that
declare spring bones (hair, hats, earrings, wings, tails, a jacket, trousers, boots, gloves, a
beard), up to nine on one avatar. Ten avatars were simulated, the player among them, with 180 to
230 chains between them:

- **steady**: 12 avatars swaying in place; the simulated ten never change.
- **churn**: 14 avatars, seven of them walking 2.5 to 9.5 m from the camera and back every 4 s,
  out of step, so the ten nearest change several times a second.
- **flip**: the steady crowd, with the camera turning its back on it and facing it again every
  2 s, so every simulated avatar but the player leaves the set at once and enters it again.

| Scene | Frame, main (off) | Frame, change (on) | On − off, frame | On − off, main-world schedules |
| --- | ---: | ---: | ---: | ---: |
| steady | 43.05 ms | 43.34 ms | +0.41 ms (+0.16 to +0.53) | +0.44 ms (+0.06 to +0.58) |
| churn | 45.39 ms | 46.00 ms | +0.77 ms (+0.35 to +0.84) | +0.49 ms (+0.23 to +0.75) |
| flip | 44.51 ms | 44.79 ms | +0.14 ms (+0.04 to +0.34) | +0.03 ms (−0.60 to +0.74) |

The frame columns are the median of each build's three runs of the mean frame time. The on − off
columns pair the two settings within a run of one build, six runs in all, as median and range:
a run's mean moves by more between runs (41.7 to 43.6 ms in steady) than the setting moves it.
Ten simulated avatars cost about half a millisecond of the main world's schedules a frame: 1 to
2 % of this frame, and about 3 % of the 16.7 ms frame of the 60 fps target.

Worst frames, setting on against off, over the six runs:

| Scene | Slowest frame of a run, on | off | Frames over 50 ms, on | off |
| --- | ---: | ---: | ---: | ---: |
| steady | 83.0 to 88.8 ms | 82.3 to 86.4 ms | 3.0 % | 2.9 % |
| churn | 87.5 to 100.6 ms | 86.8 to 112.7 ms | 11.1 % | 8.1 % |
| flip | 88.8 to 91.9 ms | 89.8 to 110.6 ms | 8.0 % | 7.8 % |

Avatars entering and leaving the simulated set add no frame slower than the scenes already have
with the setting off. In churn more frames pass 50 ms with the setting on: its 95th percentile
frame is 50 ms either way, so the added time moves frames across that line.

Limits of the measurement: one machine. Its frame here, 43 to 46 ms, is not bound by the main
world, whose schedules take 13 ms of it, and Chrome reported no GPU time. The cost on a slower CPU,
where the main world bounds the frame, is not measured.
