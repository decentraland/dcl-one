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
reads the wearable's first skin in the order it lists its joints. A joint not named `avatar_*` is
a spring bone, declared or not. One whose parent is a body bone starts a chain anchored to that
bone; the spring bones listed after it, up to the next such bone, are the rest of the chain. A
chain simulates its first 12 joints. A bone takes its own declared parameters, else those of the
nearest bone above it that declares some, else stiffness 0.5 and drag 0.4 with no gravity. The
result is a `SpringChains` component on the avatar's model entity, so a rebuilt avatar starts
with fresh chains and a removed one takes its own with it. `process_avatar` moves each branch of
a wearable's own bones under the body bone it hangs off, so the chain root follows the body
through the ordinary hierarchy.

**The simulation runs in fixed steps of 1/60 s.** A frame runs the steps that fit the time since
the last, at most four, and shows each bone blended between its last two steps. The anchor bone
is moved from last frame's pose to this frame's across the steps. A frame longer than four steps
is not simulated: the bones hold their pose, and the next frame starts them at rest where the
avatar then stands. The local player's teleport does the same. Only rotations are written, so
bones never stretch.

**At most ten avatars are simulated** (`MAX_SIMULATED_AVATARS`). The local player and preview
avatars rank first, then avatars by squared distance from the primary camera. Avatars behind the
camera, and those the avatar limit hides, are left out. An avatar that drops out has its bones
put back at rest.

**It runs in a new `PostUpdateSets::SpringBones`,** after `InverseKinematics` and before
`Nametag`. `simulate_spring_bones` computes the poses it needs from local transforms with
`TransformHelper`, as the IK systems do, and writes local rotations before the final propagate.

**One setting turns it on:** `GraphicsSettings::spring_bones`, shown as "Jiggle Bones". It is off
by default.

`gravityDir` is authored for a left-handed world and is read as `(x, y, -z)`, the mapping scene
transforms use.

## Consequences

- Motion does not depend on the frame rate: the same movement settles to the same pose at 30, 60
  and 144 fps (`spring_bones::sim` tests).
- Below 15 fps every frame is longer than four steps, so spring bones hold still.
- An eleventh avatar's hair is rigid. The cap bounds the cost whatever the crowd.
- A wearable's own bones that hang off another of its own nodes, not a body bone, are now kept
  with the avatar; before, they were removed with the wearable's skeleton.
- A branching set of bones is simulated as one chain in list order, so a second branch is solved
  as if it hung off the end of the first.
- `/spring_bones` and the Spring bones debug section report the chains found and the avatars
  simulated.

## Not yet measured

The cost of ten simulated avatars on the web build, and the worst frames while avatars enter and
leave the simulated set. The setting stays off by default until that is recorded.
