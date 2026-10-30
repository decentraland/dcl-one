import {
  Animator,
  Entity,
  GltfContainer,
  Material,
  MeshRenderer,
  TextShape,
  Transform,
  engine,
} from '@dcl/sdk/ecs'
import { Color4, Quaternion, Vector3 } from '@dcl/sdk/math'
import { setupVisualTest } from './visual-test-setup'

/**
 * animations — Animator component test matrix.
 *
 * Visual contract: five labeled columns arranged left-to-right along X.
 * Each column exercises one Animator-component configuration with BOTH of
 * the locally-bundled animated GLBs (a fantasy door and a pirates lever)
 * so a regression that only affects one rig still shows up in the diff.
 *
 * Snapshot is taken after the scene has had ~3 s to run, by which time:
 *   - "main" clips (single-frame still poses) have settled at frame 0
 *   - The triggered "Open" / "activate" clips have fully played (≈1.25 s
 *     each, loop=false) and frozen on their last frame
 *   - Col 4's deferred "Close" / "deactivate" call (fired at scene-time
 *     ≈1.5 s) has also completed (~2.75 s total), bringing the rig back
 *     to its closed/deactivated pose — visually distinct from Col 3's
 *     held-open frame so resetCursor=true and resetCursor=false don't
 *     produce the same baseline
 *   - The "no Animator" subjects show whatever the renderer does by default
 *     when no Animator component is present (per the GLTF panel scene's
 *     observation, this is "auto-play"; with `main` as the first clip in
 *     both GLBs, that auto-play is itself a still pose, so the column is
 *     deterministic for pixel diff)
 *
 * Column layout (camera at z=-9 looking toward z=12, 2 parcels wide):
 *
 *  Col  X    Case                                    Expected pose
 *  ──────────────────────────────────────────────────────────────────────────
 *   0   4    Animator at frame 0 (idle pose)         door/lever idle
 *             Animator with "main" playing → "main"
 *             has a single keyframe so the clip
 *             effectively pins the rig at frame 0.
 *
 *   1   10   No Animator component                   door/lever idle
 *             Only GltfContainer; the renderer auto-
 *             plays embedded clips. Both GLBs put
 *             "main" first, so auto-play lands on a
 *             still pose. (See packages/gltf which
 *             relies on the same defense for the
 *             damagedHelmet.)
 *
 *   2   16   Animator present, no clip playing       door/lever rest pose
 *             Animator.create(entity, { states: [
 *               { clip: "Open", playing: false }
 *             ] }). The component is present but
 *             nothing is driving the rig.
 *
 *   3   22   Played with resetCursor=true            door swung open,
 *             Animator + playSingleAnimation(entity,  lever activated
 *             "Open", resetCursor=true) called at
 *             init. The clip plays once (loop=false)
 *             from frame 0 and freezes at its end
 *             frame.
 *
 *   4   28   Played with resetCursor=false           door swung CLOSED,
 *             playSingleAnimation("Open", true) at    lever deactivated
 *             init opens the rig, then a one-shot
 *             system fires at scene-time ≈1.5 s and
 *             calls playSingleAnimation("Close",
 *             resetCursor=FALSE) so the close clip
 *             plays from its current cursor (0, since
 *             "Close" was never played before).
 *             Final pose distinguishes this column
 *             from Col 3 — open vs. closed — proving
 *             the second playSingleAnimation call
 *             (with resetCursor=false) actually ran.
 */

// ── animation clip names (must match the GLB's animation list) ──────────────
const DOOR_IDLE_CLIP = 'main'
const DOOR_OPEN_CLIP = 'Open'
const DOOR_CLOSE_CLIP = 'Close'
const LEVER_IDLE_CLIP = 'main'
const LEVER_ACTIVATE_CLIP = 'activate'
const LEVER_DEACTIVATE_CLIP = 'deactivate'

const DOOR_SRC = 'assets/Models/door_fantasy.glb'
const LEVER_SRC = 'assets/Models/lever_pirates.glb'

// ── scene geometry ─────────────────────────────────────────────────────────
const DOOR_Z = 12       // back row — doors are tall and read at distance
const LEVER_Z = 7       // front row — lever is short, sits closer to camera
const LABEL_Z = 4.5     // labels in front of both rows
const COLUMN_XS = [4, 10, 16, 22, 28] as const

/**
 * Scene-time delay before Col 4 triggers its second playSingleAnimation
 * (the resetCursor=FALSE one). Has to be ≥ the first clip's ~1.25 s
 * duration so the Open/activate runs to completion before Close/deactivate
 * is requested. 1.5 s leaves a small margin. The test fixture sleeps long
 * enough for Close (also ~1.25 s) to finish before the snapshot fires.
 */
const COL4_DEFERRED_CALL_AT_SECONDS = 0.1

/**
 * Door GLB origin sits at the hinge with the wood mesh extending along +X
 * for ~1.65 m. Subtract roughly half that width when placing so the column
 * label visually centers under the *door*, not the hinge.
 */
const DOOR_X_OFFSET = -0.8

export function main() {
  // ── floor across both parcels ─────────────────────────────────────────────
  const ground = engine.addEntity()
  Transform.create(ground, {
    position: Vector3.create(16, 0.01, 9),
    scale: Vector3.create(32, 0.02, 18),
  })
  MeshRenderer.setBox(ground)
  Material.setPbrMaterial(ground, {
    albedoColor: Color4.create(0.22, 0.22, 0.25, 1),
    roughness: 1,
    metallic: 0,
  })

  // ── Col 0 — Animator at frame 0 (idle pose) ───────────────────────────────
  // "main" is a single-keyframe clip that holds the rig at its rest pose.
  // Marking it playing=true is the canonical "pin at frame 0" recipe.
  {
    const colX = COLUMN_XS[0]

    const door = makeDoor(colX)
    Animator.create(door, {
      states: [{ clip: DOOR_IDLE_CLIP, playing: true, loop: true }],
    })

    const lever = makeLever(colX)
    Animator.create(lever, {
      states: [{ clip: LEVER_IDLE_CLIP, playing: true, loop: true }],
    })

    makeLabel(colX, 'Animator\n"main" playing\n(frame 0)')
  }

  // ── Col 1 — No Animator component ─────────────────────────────────────────
  // Bare GltfContainer; we let the renderer's default-auto-play behavior do
  // whatever it does. Both GLBs declare "main" first, so this column lands
  // on the same still pose as Col 0 — any visual divergence between these
  // two columns flags a regression in the no-Animator code path.
  {
    const colX = COLUMN_XS[1]
    makeDoor(colX)
    makeLever(colX)
    makeLabel(colX, 'No Animator\n(default auto-play\n→ "main" idle)')
  }

  // ── Col 2 — Animator present, no clip playing ─────────────────────────────
  // States are declared but playing=false, so the Animator is wired up yet
  // inert. The rig should sit at its bind/rest pose — which on these two
  // GLBs happens to coincide visually with frame 0 of "main".
  {
    const colX = COLUMN_XS[2]

    const door = makeDoor(colX)
    Animator.create(door, {
      states: [{ clip: DOOR_OPEN_CLIP, playing: false, loop: false }],
    })

    const lever = makeLever(colX)
    Animator.create(lever, {
      states: [{ clip: LEVER_ACTIVATE_CLIP, playing: false, loop: false }],
    })

    makeLabel(colX, 'Animator\nno clip playing\n(bind pose)')
  }

  // ── Col 3 — playSingleAnimation with resetCursor=true ─────────────────────
  // Declare the play clip stopped, then immediately call
  // playSingleAnimation(..., resetCursor=true) so the cursor is forced to
  // frame 0 before play begins. loop=false on the state means the clip runs
  // once (~1.25 s) and freezes at its end frame — door fully open, lever in
  // its activated position. The snapshot deadline is well past that, so the
  // column is stable for pixel diff.
  {
    const colX = COLUMN_XS[3]

    const door = makeDoor(colX)
    Animator.create(door, {
      states: [{ clip: DOOR_OPEN_CLIP, playing: false, loop: false }],
    })
    Animator.playSingleAnimation(door, DOOR_OPEN_CLIP, true)

    const lever = makeLever(colX)
    Animator.create(lever, {
      states: [{ clip: LEVER_ACTIVATE_CLIP, playing: false, loop: false }],
    })
    Animator.playSingleAnimation(lever, LEVER_ACTIVATE_CLIP, true)

    makeLabel(colX, 'playSingleAnimation\nresetCursor=true\n("Open" / "activate")')
  }

  // ── Col 4 — playSingleAnimation with resetCursor=FALSE ────────────────────
  // To produce a visually distinct outcome from Col 3 (and to actually
  // exercise a second playSingleAnimation call), we run Open/activate at
  // init AND register a one-shot system that — once scene-time crosses
  // COL4_DEFERRED_CALL_AT_SECONDS (~1.5 s, past the open clip's ~1.25 s
  // duration) — fires playSingleAnimation with the *reverse* clip and
  // resetCursor=false. Close/deactivate then run for ~1.25 s and freeze
  // the rig back at its closed pose. By snapshot time the door reads
  // CLOSED on Col 4 and OPEN on Col 3 — the only pixel-level signal that
  // distinguishes resetCursor=true from resetCursor=false on a single
  // still frame.
  {
    const colX = COLUMN_XS[4]

    const door = makeDoor(colX)
    Animator.create(door, {
      states: [
        { clip: DOOR_OPEN_CLIP, playing: false, loop: false },
        { clip: DOOR_CLOSE_CLIP, playing: false, loop: false },
      ],
    })
    Animator.playSingleAnimation(door, DOOR_CLOSE_CLIP, true)

    const lever = makeLever(colX)
    Animator.create(lever, {
      states: [
        { clip: LEVER_ACTIVATE_CLIP, playing: false, loop: false },
        { clip: LEVER_DEACTIVATE_CLIP, playing: false, loop: false },
      ],
    })
    Animator.playSingleAnimation(lever, LEVER_DEACTIVATE_CLIP, true)

    let elapsed = 0
    let fired = false
    engine.addSystem((dt: number) => {
      if (fired) return
      elapsed += dt
      if (elapsed < COL4_DEFERRED_CALL_AT_SECONDS) return
      fired = true
      Animator.playSingleAnimation(door, DOOR_OPEN_CLIP, false)
      Animator.playSingleAnimation(lever, LEVER_ACTIVATE_CLIP, false)
    })

    makeLabel(
      colX,
      'playSingleAnimation\nresetCursor=false\n("Close" / "deactivate")',
    )
  }

  // ── scene title ───────────────────────────────────────────────────────────
  const title = engine.addEntity()
  Transform.create(title, { position: Vector3.create(16, 6.5, DOOR_Z) })
  TextShape.create(title, {
    text: 'Animator · test matrix',
    fontSize: 2.5,
    textColor: Color4.create(1, 1, 1, 1),
    textWrapping: false,
  })

  // ── camera ────────────────────────────────────────────────────────────────
  // Five columns span x=4..28 (24 m) on a 6 m pitch — closer than the 8 m
  // spacing used in 3-parcel scenes, but still leaves ~4 m of clear floor
  // between adjacent doors so swing arcs and shadows don't overlap. Camera
  // pulled back to z=-9 and raised to y=4 so the full row fits in the
  // default HFOV and the ~3 m doors don't crop at the top. lookAt y=2
  // splits the difference between the door's mid-height and the shorter
  // lever in the foreground.
  setupVisualTest({
    lookAtPos: Vector3.create(16, 2, DOOR_Z),
    cameraPos: Vector3.create(16, 4, -9),
    hideAreaCenter: Vector3.create(16, 4, 9),
    hideAreaSize: Vector3.create(34, 16, 22),
  })
}

// ── helpers ──────────────────────────────────────────────────────────────────

function makeDoor(colX: number): Entity {
  const e = engine.addEntity()
  Transform.create(e, {
    position: Vector3.create(colX + DOOR_X_OFFSET, 0, DOOR_Z),
    // Yaw 180° so the door's broad face turns toward the camera at low z;
    // the wood mesh runs along +X in local space, which 180°Y mirrors to -X
    // in world, putting the door body to the left of the hinge from the
    // camera's POV. The "Open" rotation then swings into open space.
    rotation: Quaternion.fromEulerDegrees(0, 180, 0),
  })
  GltfContainer.create(e, { src: DOOR_SRC })
  return e
}

function makeLever(colX: number): Entity {
  const e = engine.addEntity()
  Transform.create(e, {
    position: Vector3.create(colX, 0, LEVER_Z),
    // Same 180°Y yaw as the doors so both rigs present a consistent face
    // to the camera regardless of which clip is driving them.
    rotation: Quaternion.fromEulerDegrees(0, 90, 0),
    scale: Vector3.create(1.6, 1.6, 1.6),
  })
  GltfContainer.create(e, { src: LEVER_SRC })
  return e
}

function makeLabel(colX: number, text: string): void {
  const e = engine.addEntity()
  Transform.create(e, { position: Vector3.create(colX, 0.4, LABEL_Z) })
  TextShape.create(e, {
    text,
    fontSize: 2,
    textColor: Color4.create(0.95, 0.95, 0.95, 1),
    textWrapping: false,
  })
}
