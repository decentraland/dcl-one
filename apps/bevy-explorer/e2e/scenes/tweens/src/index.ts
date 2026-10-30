import {
  Entity,
  InputAction,
  Material,
  MeshCollider,
  MeshRenderer,
  TextShape,
  Transform,
  Tween,
  TweenSequence,
  TweenState,
  TweenStateStatus,
  engine,
  pointerEventsSystem,
  tweenSystem,
} from '@dcl/sdk/ecs'
import { Color3, Color4, Quaternion, Vector2, Vector3 } from '@dcl/sdk/math'
import { setupVisualTest } from './visual-test-setup'

/**
 * tweens — Tween component test matrix.
 *
 * Visual contract: six labeled columns arranged left-to-right along X,
 * each demonstrating one tween case. All tweens use duration=50ms so they
 * complete within the very first few frames. The screenshot captures the
 * final resting state, not any animation in progress.
 *
 * Column layout (camera at z=-1, looking at z=8):
 *
 *  Col  X    Case
 *  ───────────────────────────────────────────────────────────
 *   0   2    After tween completed — position tween, then on
 *             completion callback moves entity 1 m higher
 *   1   6    Tween sequence — A→B→C moves, final pos visible
 *   2   10   Rotation tween — ends at 90° Y from start
 *   3   14   Scale tween — grows from 0.3 to 1.5
 *   4   18   Parent tween affecting child — parent moves right,
 *             child rides along
 *   5   22   After material tween completed — UV tween, on
 *             completion entity moves 0.5 m up proving callback
 *             fired; final UV offset also clearly different
 *
 * Every entity carries a hover hint (PointerEvents) and a TextShape label.
 * Scene occupies 3 parcels (0,0 / 1,0 / 2,0) to accommodate the X spread.
 */

// ── constants ────────────────────────────────────────────────────────────────

const TWEEN_DURATION = 50  // ms — finishes in < 100 ms as required
const Z_SUBJECT = 8        // all subject boxes sit at this depth
const LABEL_Z = 5          // column labels sit slightly closer to camera
const HINT_MAX_DISTANCE = 100

// ── state flags — set true once each completion system has run ───────────────
// These prevent the system callbacks from re-firing on subsequent frames.
let col0Done = false
let col1Done = false
let col5Done = false

// ── helper: attach a hover tooltip to any entity ────────────────────────────
// Flip DEBUG_HOVER_HINTS to true to wire up hover tooltips while debugging the
// scene by hand. Kept off for visual-test runs so the cursor never paints a
// hover outline.
const DEBUG_HOVER_HINTS = false

function attachHint(entity: Entity, text: string): void {
  if (!DEBUG_HOVER_HINTS) return
  pointerEventsSystem.onPointerDown(
    {
      entity,
      opts: {
        button: InputAction.IA_POINTER,
        hoverText: text,
        maxDistance: HINT_MAX_DISTANCE,
      },
    },
    () => { },
  )
}

// ── helper: place a TextShape label below a column ──────────────────────────
function makeLabel(x: number, text: string): void {
  const e = engine.addEntity()
  Transform.create(e, { position: Vector3.create(x, 1.0, LABEL_Z) })
  TextShape.create(e, {
    text,
    fontSize: 2.5,
    textColor: Color4.create(0.95, 0.95, 0.95, 1),
    textWrapping: false,
  })
  MeshCollider.setPlane(e)
  attachHint(e, `Label: ${text}`)
}

// ── helper: make a colored box with a collider ───────────────────────────────
function makeBox(
  pos: Vector3,
  color: Color4,
  scale: Vector3 = Vector3.create(0.85, 0.85, 0.85),
): Entity {
  const e = engine.addEntity()
  Transform.create(e, { position: pos, scale })
  MeshRenderer.setBox(e)
  MeshCollider.setBox(e)
  Material.setPbrMaterial(e, {
    albedoColor: color,
    roughness: 0.8,
    metallic: 0,
  })
  return e
}

// ── helper: ground strip under a column (visual reference baseline) ──────────
function makeGroundPatch(x: number): void {
  const g = engine.addEntity()
  Transform.create(g, {
    position: Vector3.create(x, 0.03, 8),
    scale: Vector3.create(3.2, 0.04, 10),
  })
  MeshRenderer.setBox(g)
  Material.setPbrMaterial(g, {
    albedoColor: Color4.create(0.28, 0.28, 0.32, 1),
    roughness: 1,
    metallic: 0,
  })
}

// ─────────────────────────────────────────────────────────────────────────────
// MAIN
// ─────────────────────────────────────────────────────────────────────────────

export function main() {

  // ── global ground ─────────────────────────────────────────────────────────
  const ground = engine.addEntity()
  Transform.create(ground, {
    position: Vector3.create(12, 0.01, 8),
    scale: Vector3.create(26, 0.02, 18),
  })
  MeshRenderer.setBox(ground)
  Material.setPbrMaterial(ground, {
    albedoColor: Color4.create(0.22, 0.22, 0.25, 1),
    roughness: 1,
    metallic: 0,
  })

  for (const x of [2, 6, 10, 14, 18, 22]) {
    makeGroundPatch(x)
  }

  // ── Col 0 — After tween completed ─────────────────────────────────────────
  // A cyan box starts at y=1. A Move tween takes it to y=2 (clearly higher).
  // On completion, a system moves it an additional 1 m up to y=3 and changes
  // its color to bright yellow. The screenshot shows a yellow box at y=3,
  // proving the completion callback fired.
  //
  // Visual contract:
  //   start   → y=1, cyan
  //   tween   → y=2, cyan    (50 ms)
  //   callback→ y=3, yellow  (applied on the first frame after completion)
  {
    const startPos = Vector3.create(2, 1, Z_SUBJECT)
    const tweenEndPos = Vector3.create(2, 2, Z_SUBJECT)
    const callbackPos = Vector3.create(2, 3, Z_SUBJECT)

    const box = makeBox(startPos, Color4.create(0.2, 0.9, 0.9, 1))
    attachHint(box, 'Col 0: after-tween callback box — should be YELLOW at y=3')

    Tween.setMove(box, startPos, tweenEndPos, TWEEN_DURATION)

    engine.addSystem(() => {
      if (col0Done) return
      if (!tweenSystem.tweenCompleted(box)) return
      col0Done = true

      // Remove the finished tween so the entity stays still.
      Tween.deleteFrom(box)

      // Snap to the post-completion position and recolor to yellow.
      Transform.getMutable(box).position = callbackPos
      Material.setPbrMaterial(box, {
        albedoColor: Color4.create(1, 0.95, 0.1, 1),
        roughness: 0.8,
        metallic: 0,
      })
    })

    makeLabel(2, 'after-tween\ncallback\n(yellow at y=3)')
  }

  // ── Col 1 — Tween sequence ─────────────────────────────────────────────────
  // An orange box runs a 3-step position sequence: start→A→B→C.
  // The TweenSequence component drives each PBTween in order. On sequence
  // completion the TweenState becomes TS_COMPLETED; a system then snaps
  // the color to green to confirm the whole sequence ran.
  //
  // Positions:
  //   start  x=6, y=1, z=Z_SUBJECT
  //   A      x=6, y=3, z=Z_SUBJECT   (+2 m up)
  //   B      x=6, y=3, z=Z_SUBJECT-1 (1 m forward)
  //   C      x=6, y=5, z=Z_SUBJECT-1 (+2 m up again — final resting place)
  //
  // Visual contract: green box at y=5, z=Z_SUBJECT-1.
  {
    const seqStart = Vector3.create(6, 1, Z_SUBJECT)
    const seqA = Vector3.create(6, 3, Z_SUBJECT)
    const seqB = Vector3.create(6, 3, Z_SUBJECT - 1)
    const seqC = Vector3.create(6, 5, Z_SUBJECT - 1)

    const box = makeBox(seqStart, Color4.create(1, 0.5, 0.1, 1))
    attachHint(box, 'Col 1: tween-sequence box — should be GREEN at y=5')

    // The first tween on the entity kicks the sequence off.
    Tween.createOrReplace(box, {
      duration: TWEEN_DURATION,
      easingFunction: 0, // EF_LINEAR
      mode: Tween.Mode.Move({
        start: seqStart,
        end: seqA,
      }),
    })

    TweenSequence.createOrReplace(box, {
      sequence: [
        {
          duration: TWEEN_DURATION,
          easingFunction: 0,
          mode: Tween.Mode.Move({ start: seqA, end: seqB }),
        },
        {
          duration: TWEEN_DURATION,
          easingFunction: 0,
          mode: Tween.Mode.Move({ start: seqB, end: seqC }),
        },
      ],
    })

    engine.addSystem(() => {
      if (col1Done) return
      // The sequence is complete when TweenState shows TS_COMPLETED *and*
      // the TweenSequence.sequence array is empty (all steps consumed).
      // Checking sequence.length === 0 guards against firing on intermediate
      // step completions — each step also emits TS_COMPLETED before the
      // sequence system advances to the next one.
      const state = TweenState.getOrNull(box)
      if (!state || state.state !== TweenStateStatus.TS_COMPLETED) return
      const seq = TweenSequence.getOrNull(box)
      if (!seq || seq.sequence.length !== 0) return
      col1Done = true

      Tween.deleteFrom(box)
      TweenSequence.deleteFrom(box)

      // Snap to exact final position to eliminate any floating-point drift.
      Transform.getMutable(box).position = seqC
      Material.setPbrMaterial(box, {
        albedoColor: Color4.create(0.2, 0.9, 0.3, 1),
        roughness: 0.8,
        metallic: 0,
      })
    })

    makeLabel(6, 'tween-seq\n(green at\ny=5, z=7)')
  }

  // ── Col 2 — Rotation tween ────────────────────────────────────────────────
  // A blue elongated box (tall in Y) starts upright (identity rotation) and
  // rotates 90° around Y. The box is non-square so the 90° turn is visually
  // unambiguous: what was a tall rectangle becomes a sideways rectangle.
  //
  // Start: Quaternion.Identity()
  // End:   90° around Y axis
  {
    const rotBox = engine.addEntity()
    Transform.create(rotBox, {
      position: Vector3.create(10, 1.5, Z_SUBJECT),
      scale: Vector3.create(0.45, 2.1, 0.85),
      rotation: Quaternion.Identity(),
    })
    MeshRenderer.setBox(rotBox)
    MeshCollider.setBox(rotBox)
    Material.setPbrMaterial(rotBox, {
      albedoColor: Color4.create(0.25, 0.55, 1, 1),
      roughness: 0.8,
      metallic: 0,
    })
    attachHint(rotBox, 'Col 2: rotation tween — should be rotated 90° around Y')

    Tween.setRotate(
      rotBox,
      Quaternion.Identity(),
      Quaternion.fromEulerDegrees(0, 90, 0),
      TWEEN_DURATION,
    )

    makeLabel(10, 'rotation\ntween\n(90° Y)')
  }

  // ── Col 3 — Scale tween ───────────────────────────────────────────────────
  // A magenta box starts very small (0.2 uniform) and scales up to 1.6.
  // The contrast (tiny→large) is impossible to miss.
  //
  // Start scale: (0.3, 0.3, 0.3)
  // End scale:   (2.2, 2.2, 2.2)
  {
    const scaleBox = engine.addEntity()
    Transform.create(scaleBox, {
      position: Vector3.create(14, 1, Z_SUBJECT),
      scale: Vector3.create(0.3, 0.3, 0.3),
    })
    MeshRenderer.setBox(scaleBox)
    MeshCollider.setBox(scaleBox)
    Material.setPbrMaterial(scaleBox, {
      albedoColor: Color4.create(0.9, 0.25, 0.8, 1),
      roughness: 0.8,
      metallic: 0,
    })
    attachHint(scaleBox, 'Col 3: scale tween — should be scale 2.2 (large box)')

    Tween.setScale(
      scaleBox,
      Vector3.create(0.3, 0.3, 0.3),
      Vector3.create(2.2, 2.2, 2.2),
      TWEEN_DURATION,
    )

    makeLabel(14, 'scale tween\n(0.3→2.2)')
  }

  // ── Col 4 — Parent tween affecting child ──────────────────────────────────
  // A white parent box starts at x=18, y=1. A Move tween moves it to x=20.
  // A smaller red child box is parented to it, offset by (0, 1.5, 0) in
  // local space. After the tween the child is at world x=20, y=2.5 —
  // clearly displaced from x=18, y=2.5 where it would be without the tween.
  //
  // Visual contract: white parent at x=20, red child riding above it.
  {
    const parentStart = Vector3.create(18, 1, Z_SUBJECT)
    const parentEnd = Vector3.create(20, 1, Z_SUBJECT)

    const parent = makeBox(parentStart, Color4.create(0.9, 0.9, 0.9, 1))
    attachHint(parent, 'Col 4: parent box — tween moves it right to x=20')

    // Child entity: parented to parent, local offset +1.5 in Y.
    const child = engine.addEntity()
    Transform.create(child, {
      parent,
      position: Vector3.create(0, 1.5, 0),
      scale: Vector3.create(0.55, 0.55, 0.55),
    })
    MeshRenderer.setBox(child)
    MeshCollider.setBox(child)
    Material.setPbrMaterial(child, {
      albedoColor: Color4.create(1, 0.2, 0.2, 1),
      roughness: 0.8,
      metallic: 0,
    })
    attachHint(child, 'Col 4: child box — rides the parent tween to world x≈20')

    Tween.setMove(parent, parentStart, parentEnd, TWEEN_DURATION)

    makeLabel(18, 'parent→child\ntween\n(parent x+2)')
  }

  // ── Col 5 — After material (texture) tween completed ─────────────────────
  // A plane displays a texture from the FanstasyPack_TX sprite sheet.
  // A TextureMove tween shifts the UV offset from the "Greens" region of the
  // sheet (bottom-right, ~offset 0.6, 0.0) to a clearly different palette
  // region (mid-left, ~offset 0.0, 0.5). Because each palette block is a
  // solid distinct color, the face of the plane visibly changes color when
  // the tween completes.
  //
  // On tween completion, the system:
  //   1. Removes the tween (freezes the UV at the final offset).
  //   2. Moves the entity 0.5 m upward — positional proof the callback fired.
  //      (No recolor needed; the texture region itself proves the tween ran.)
  //
  // The mesh is a plane (MeshRenderer.setPlane) facing +Z toward the camera
  // so the full texture face is visible in the screenshot.
  //
  // Texture: assets/Images/FanstasyPack_TX.png (512×512 sprite sheet)
  //
  // Visual contract:
  //   before callback: plane at y=1, UV showing green region
  //   after  callback: plane at y=1.5, UV showing a different palette region
  {
    const matBox = engine.addEntity()
    const matStartPos = Vector3.create(22, 1, Z_SUBJECT)
    const matCallbackPos = Vector3.create(22, 1.5, Z_SUBJECT)

    Transform.create(matBox, {
      position: matStartPos,
      scale: Vector3.create(1.4, 1.4, 1.4),
    })
    MeshRenderer.setPlane(matBox)
    MeshCollider.setPlane(matBox)

    // Apply the sprite-sheet texture. The UV tiling is set to 0.25×0.25 so
    // the plane shows roughly a quarter of the sheet at a time — each palette
    // swatch is large enough to fill the face with a solid recognizable color.
    Material.setPbrMaterial(matBox, {
      texture: Material.Texture.Common({
        src: 'assets/Images/FanstasyPack_TX.png',
        // Start UV: 0.25× tiling shows about a quarter of the sheet at a time;
        // initial offset (0.6, 0.0) lands on the Greens block (bottom-right area).
        offset: Vector2.create(0.6, 0.0),
        tiling: Vector2.create(0.25, 0.25),
      }),
      roughness: 0.8,
      metallic: 0,
    })
    attachHint(
      matBox,
      'Col 5: texture UV tween — UV shifts from Greens region; at y=1.5 callback fired',
    )

    // TextureMove tween: UV offset (0.6, 0.0) → (0.0, 0.5).
    // Moves from the solid-green Greens block to a different palette region,
    // producing a clearly different color on the plane face.
    Tween.setTextureMove(
      matBox,
      { x: 0.6, y: 0.0 },
      { x: 0.0, y: 0.5 },
      TWEEN_DURATION,
    )

    engine.addSystem(() => {
      if (col5Done) return
      if (!tweenSystem.tweenCompleted(matBox)) return
      col5Done = true

      Tween.deleteFrom(matBox)

      // Move 0.5 m up — the explicit "after completion" positional proof.
      Transform.getMutable(matBox).position = matCallbackPos
    })

    makeLabel(22, 'material-tween\ncallback\n(UV shift→y=1.5)')
  }

  // ── scene title ───────────────────────────────────────────────────────────
  const titleE = engine.addEntity()
  Transform.create(titleE, { position: Vector3.create(12, 7.5, Z_SUBJECT) })
  TextShape.create(titleE, {
    text: 'Tween · test matrix',
    fontSize: 5,
    textColor: Color4.create(1, 1, 1, 1),
    textWrapping: false,
  })
  MeshCollider.setPlane(titleE)
  attachHint(titleE, 'Title: Tween test matrix')

  // ── camera ────────────────────────────────────────────────────────────────
  // Pulled back to z=-1 and raised to y=3 so all six columns (x=2..22) fit
  // in frame together. LookAt points to the horizontal and vertical center
  // of the layout.
  setupVisualTest({
    lookAtPos: Vector3.create(12, 3, Z_SUBJECT),
    cameraPos: Vector3.create(12, 4, -5.5),
    hideAreaCenter: Vector3.create(12, 4, 8),
    hideAreaSize: Vector3.create(30, 16, 20),
  })
}
