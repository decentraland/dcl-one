import {
  ColliderLayer,
  Entity,
  InputAction,
  Material,
  MeshCollider,
  MeshRenderer,
  RaycastQueryType,
  TextShape,
  Transform,
  TriggerArea,
  engine,
  pointerEventsSystem,
  raycastSystem,
  triggerAreaEventsSystem,
} from '@dcl/sdk/ecs'
import { Color4, Quaternion, Vector3 } from '@dcl/sdk/math'
import { movePlayerTo } from '~system/RestrictedActions'
import { setupVisualTest } from './visual-test-setup'

/**
 * triggers-raycasts — TriggerArea + Raycast test matrix.
 *
 * ──── How to read the scene ────
 *
 * Six columns, each with TWO test cells stacked vertically:
 *   • the BOTTOM cell tests a TriggerArea case
 *   • the TOP cell tests a Raycast case
 *
 * Each cell consists of:
 *   • an INDICATOR CUBE (the only entity whose colour depends on the result)
 *   • the test machinery — an invisible trigger volume + orange "payload"
 *     sphere for trigger cells, or an invisible ray source + purple "target"
 *     sphere(s) for raycast cells
 *   • a black text label naming the case
 *
 * ──── Colour conventions ────
 *
 * INDICATOR CUBE — conditional, set by the runtime when the callback fires:
 *   GRAY   → callback never fired (correct outcome for the negative cases)
 *   GREEN  → onTriggerEnter / HIT_FIRST raycast fired with a hit
 *   BLUE   → onTriggerStay fired (proves the STAY synthesis path works)
 *   YELLOW → RQT_QUERY_ALL returned ≥2 hits
 *
 * ORANGE PAYLOAD SPHERE — constant, identifies a trigger-payload entity.
 *   Colour is purely a visual role marker; it never changes.
 *
 * PURPLE TARGET SPHERE — constant, identifies a raycast-target entity.
 *   Colour is purely a visual role marker; it never changes.
 *
 * If you ever see a cube that's still gray when the column header says it
 * should be coloured, the SDK regressed; if you see a cube coloured when
 * the column says it should be gray, the SDK has a false-positive.
 *
 * ──── Column layout ────
 *
 *  Col   X    Bottom cell (TriggerArea)         Top cell (Raycast)
 *  ─────────────────────────────────────────────────────────────────────────
 *   0     3   Box trigger detects               Local-direction ray
 *             ball on CL_CUSTOM1        GREEN   hits a target              GREEN
 *
 *   1     8   Sphere trigger detects            Global-direction ray ↓
 *             ball on CL_CUSTOM1        GREEN   hits a target below        GREEN
 *
 *   2    13   Trigger on CL_CUSTOM1             Global-target ray hits
 *             vs ball on CL_CUSTOM2             a point past the target    GREEN
 *             — wrong layer, no fire    GRAY
 *
 *   3    18   Multi-layer mask                  Target-entity ray
 *             (CL_CUSTOM1|CL_CUSTOM2)           recomputes direction
 *             accepts ball on either    GREEN   from the target each tick  GREEN
 *
 *   4    23   Default mask CL_PLAYER —          Raycast mask = CL_CUSTOM1
 *             avatar spawned inside             vs target on CL_CUSTOM2
 *             the volume fires it       GREEN   — wrong layer, no hit      GRAY
 *
 *   5    28   onTriggerStay subscription        RQT_QUERY_ALL hits all 3
 *             only (no onTriggerEnter)   BLUE   stacked targets, hits.len  YELLOW
 *                                              ≥ 2
 *
 * Player spawn is pinned at (23, 0, 10) in scene.json — inside the column-4
 * trigger volume — so the avatar's capsule fires the CL_PLAYER trigger.
 *
 * Scene occupies 2 parcels: (0,0) and (1,0). The X spread (3..28) fits inside
 * the 32 m parcel width.
 *
 * Every visible entity carries a hover hint repeating its column's label so
 * an operator inspecting the scene can cursor over any cube/ball/target/text
 * and read what's being tested.
 */

// ── constants ────────────────────────────────────────────────────────────────

const COLUMNS_X = [3, 8, 13, 18, 23, 28] as const

const TRIGGER_Y = 2.5 // centre of the trigger-row volumes
const RAYCAST_Y = 7 //   centre of the raycast-row geometry
const INDICATOR_TRIGGER_Y = 1.6 // bottom-row indicator cube height
const INDICATOR_RAYCAST_Y = 6.0 // top-row indicator cube height
const ROW_Z = 10 // back-plane Z for the test geometry
const INDICATOR_Z = 8.5 // indicators sit in front of the test geometry

const INDICATOR_SCALE = 1.4 // was 0.9 — chunky cubes legible at distance
const BALL_SCALE = 0.7 //      was 0.45
const TARGET_SCALE = 0.55 //   was 0.35
const LABEL_FONT_SIZE = 2.2 // was 1.4
const LABEL_BOTTOM_Y = 4.6 // bottom-row labels float above bottom geometry,
const LABEL_TOP_Y = 10.0 //   top-row labels float above top geometry,
const LABEL_Z = 7 //          both rows of labels sit against the skybox.
const TITLE_FONT_SIZE = 4.5 // was 3.0
const TITLE_Y = 13.2 //       title sits above the top-row labels
const ROW_LABEL_FONT_SIZE = 2.4 // was 1.5
const HINT_MAX_DISTANCE = 100 // generous so the locked virtual camera can hover everything

const COLOR_BLACK = Color4.create(0, 0, 0, 1) // text colour — high contrast against any skybox
const COLOR_GRAY = Color4.create(0.32, 0.32, 0.35, 1)
const COLOR_GREEN = Color4.create(0.18, 0.85, 0.28, 1)
const COLOR_BLUE = Color4.create(0.22, 0.5, 0.95, 1)
const COLOR_YELLOW = Color4.create(1.0, 0.88, 0.15, 1)
const COLOR_BALL = Color4.create(0.95, 0.55, 0.1, 1) // constant — trigger payload role
const COLOR_TARGET = Color4.create(0.8, 0.3, 0.9, 1) // constant — raycast target role
const COLOR_GROUND = Color4.create(0.55, 0.55, 0.58, 1) // light-medium grey
const COLOR_PEDESTAL = Color4.create(0.28, 0.28, 0.32, 1) // dark patch under each column

// ─────────────────────────────────────────────────────────────────────────────
// Helpers
// ─────────────────────────────────────────────────────────────────────────────

/**
 * Attach a hover tooltip to a clickable entity. The entity must carry a
 * collider on CL_POINTER (the helpers below do that automatically). The
 * empty click handler is required by the PointerEvents API; the only
 * observable effect is the hover-text overlay when the cursor is over the
 * entity.
 *
 * Flip DEBUG_HOVER_HINTS to true to wire up hover tooltips while debugging the
 * scene by hand. Kept off for visual-test runs so the cursor never paints a
 * hover outline.
 */
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

/** Indicator cube. Returns the entity so its material can be recoloured later. */
function makeIndicator(x: number, y: number): Entity {
  const e = engine.addEntity()
  Transform.create(e, {
    position: Vector3.create(x, y, INDICATOR_Z),
    scale: Vector3.create(INDICATOR_SCALE, INDICATOR_SCALE, INDICATOR_SCALE),
  })
  MeshRenderer.setBox(e)
  // CL_POINTER only — no CL_PHYSICS — so the cube is hoverable but never
  // blocks avatar movement or appears in the default-mask raycasts above.
  MeshCollider.setBox(e, ColliderLayer.CL_POINTER)
  Material.setPbrMaterial(e, {
    albedoColor: COLOR_GRAY,
    roughness: 0.7,
    metallic: 0,
  })
  return e
}

/** Recolour an indicator cube's PBR material. Idempotent. */
function paint(indicator: Entity, color: Color4): void {
  Material.setPbrMaterial(indicator, {
    albedoColor: color,
    roughness: 0.7,
    metallic: 0,
  })
}

/**
 * Visible orange sphere used as a "trigger payload" inside a trigger volume.
 *
 * IMPORTANT: the collider mask is EXACTLY the requested layer — no CL_POINTER
 * is OR'd in for hover hints. The Explorer treats CL_POINTER-tagged colliders
 * as always raycast/trigger-hittable regardless of the query's collisionMask,
 * which silently breaks the negative-mask tests (Col 2 trigger, Col 4 raycast)
 * by leaking hits past the mask filter. So role-marker spheres trade their
 * hover hint for correct mask filtering. Labels and indicators still carry
 * hover hints, which is more than enough for human inspection.
 */
function makeBall(pos: Vector3, layer: ColliderLayer): Entity {
  const e = engine.addEntity()
  Transform.create(e, {
    position: pos,
    scale: Vector3.create(BALL_SCALE, BALL_SCALE, BALL_SCALE),
  })
  MeshRenderer.setSphere(e)
  MeshCollider.setSphere(e, layer)
  Material.setPbrMaterial(e, {
    albedoColor: COLOR_BALL,
    roughness: 0.6,
    metallic: 0,
  })
  return e
}

/**
 * Visible purple sphere used as a raycast target.
 *
 * Same CL_POINTER caveat as makeBall — targets pinned to an explicit layer
 * get ONLY that layer; the default-layer overload uses the SDK's default
 * MeshCollider mask (CL_POINTER|CL_PHYSICS) so the default raycasts can hit
 * them.
 */
function makeTarget(pos: Vector3, layer?: ColliderLayer): Entity {
  const e = engine.addEntity()
  Transform.create(e, {
    position: pos,
    scale: Vector3.create(TARGET_SCALE, TARGET_SCALE, TARGET_SCALE),
  })
  MeshRenderer.setSphere(e)
  if (layer !== undefined) {
    MeshCollider.setSphere(e, layer)
  } else {
    MeshCollider.setSphere(e)
  }
  Material.setPbrMaterial(e, {
    albedoColor: COLOR_TARGET,
    roughness: 0.6,
    metallic: 0,
  })
  return e
}

/**
 * TextShape label rendered facing −Z (toward the camera). Black, large, with a
 * plane collider so the cursor can hover it for the hint overlay.
 */
function makeLabel(x: number, y: number, text: string): Entity {
  const e = engine.addEntity()
  Transform.create(e, { position: Vector3.create(x, y, LABEL_Z) })
  TextShape.create(e, {
    text,
    fontSize: LABEL_FONT_SIZE,
    textColor: COLOR_BLACK,
    textWrapping: false,
  })
  // Plane collider on the pointer layer lets the cursor pick up the text
  // for the hover hint.
  MeshCollider.setPlane(e, ColliderLayer.CL_POINTER)
  return e
}

/** Thin floor patch under a column — pure visual baseline. */
function makePedestal(x: number): Entity {
  const e = engine.addEntity()
  Transform.create(e, {
    position: Vector3.create(x, 0.03, 9.5),
    scale: Vector3.create(2.6, 0.04, 4),
  })
  MeshRenderer.setBox(e)
  MeshCollider.setBox(e, ColliderLayer.CL_POINTER)
  Material.setPbrMaterial(e, {
    albedoColor: COLOR_PEDESTAL,
    roughness: 1,
    metallic: 0,
  })
  return e
}

// ─────────────────────────────────────────────────────────────────────────────
// MAIN
// ─────────────────────────────────────────────────────────────────────────────

export function main() {
  // ── global ground ────────────────────────────────────────────────────────
  // Light-medium grey so black labels above the ground (against sky) and on
  // ground (very rarely) both read with strong contrast.
  const ground = engine.addEntity()
  Transform.create(ground, {
    position: Vector3.create(16, 0.01, 8),
    scale: Vector3.create(32, 0.02, 18),
  })
  MeshRenderer.setBox(ground)
  Material.setPbrMaterial(ground, {
    albedoColor: COLOR_GROUND,
    roughness: 1,
    metallic: 0,
  })

  const pedestals: Entity[] = []
  for (const x of COLUMNS_X) {
    pedestals.push(makePedestal(x))
  }

  // ═══════════════════════════════════════════════════════════════════════════
  // BOTTOM ROW — TriggerArea cases
  // ═══════════════════════════════════════════════════════════════════════════

  // ── Col 0 — Box trigger + CL_CUSTOM1 match ────────────────────────────────
  // Set-up: an invisible TriggerArea box at (3, 2.5, 11) listens on CL_CUSTOM1.
  // An orange sphere with a CL_CUSTOM1 collider sits inside the box on scene
  // load. The trigger area's mask matches the ball's collider layer, so the
  // ENGINE detects the entity-inside-trigger overlap and fires onTriggerEnter
  // on the first tick.
  //
  // What the test asserts: the box mesh-type trigger detects entities whose
  // collider matches its collisionMask. Indicator GREEN.
  {
    const x = COLUMNS_X[0]
    const text = 'Box trigger\ndetects ball\non CUSTOM1\n→ GREEN'
    const indicator = makeIndicator(x, INDICATOR_TRIGGER_Y)
    const labelE = makeLabel(x, LABEL_BOTTOM_Y, text)

    const trigger = engine.addEntity()
    Transform.create(trigger, {
      position: Vector3.create(x, TRIGGER_Y, ROW_Z + 1),
      scale: Vector3.create(2.5, 2.5, 2.5),
    })
    TriggerArea.setBox(trigger, ColliderLayer.CL_CUSTOM1)

    makeBall(Vector3.create(x, TRIGGER_Y, ROW_Z + 1), ColliderLayer.CL_CUSTOM1)

    triggerAreaEventsSystem.onTriggerEnter(trigger, () => {
      paint(indicator, COLOR_GREEN)
    })

    for (const e of [indicator, labelE, pedestals[0]]) attachHint(e, text)
  }

  // ── Col 1 — Sphere trigger + CL_CUSTOM1 match ─────────────────────────────
  // Identical to Col 0 but the trigger volume uses TriggerArea.setSphere(),
  // proving the SPHERE mesh type also produces enter events for entities
  // whose collider layer matches the mask. The ball is again on CL_CUSTOM1
  // and sits at the centre of the sphere volume.
  //
  // What the test asserts: setSphere() works equivalently to setBox() for
  // detection. Indicator GREEN.
  {
    const x = COLUMNS_X[1]
    const text = 'Sphere trigger\ndetects ball\non CUSTOM1\n→ GREEN'
    const indicator = makeIndicator(x, INDICATOR_TRIGGER_Y)
    const labelE = makeLabel(x, LABEL_BOTTOM_Y, text)

    const trigger = engine.addEntity()
    Transform.create(trigger, {
      position: Vector3.create(x, TRIGGER_Y, ROW_Z + 1),
      scale: Vector3.create(2.5, 2.5, 2.5),
    })
    TriggerArea.setSphere(trigger, ColliderLayer.CL_CUSTOM1)

    makeBall(Vector3.create(x, TRIGGER_Y, ROW_Z + 1), ColliderLayer.CL_CUSTOM1)

    triggerAreaEventsSystem.onTriggerEnter(trigger, () => {
      paint(indicator, COLOR_GREEN)
    })

    for (const e of [indicator, labelE, pedestals[1]]) attachHint(e, text)
  }

  // ── Col 2 — Box trigger CL_CUSTOM1 vs ball on CL_CUSTOM2 (negative) ──────
  // Set-up: trigger mask = CL_CUSTOM1, ball collider = CL_CUSTOM2. The bits
  // don't overlap, so the runtime's mask AND yields zero and never reports
  // an enter event.
  //
  // What the test asserts: layer filtering ACTUALLY filters. Indicator stays
  // GRAY — the negative outcome. If the gray indicator turns green at any
  // point, the SDK is leaking events through the mask check.
  {
    const x = COLUMNS_X[2]
    const text = 'Wrong layer\ntrigger=CUSTOM1\nball=CUSTOM2\n→ stays GRAY'
    const indicator = makeIndicator(x, INDICATOR_TRIGGER_Y)
    const labelE = makeLabel(x, LABEL_BOTTOM_Y, text)

    const trigger = engine.addEntity()
    Transform.create(trigger, {
      position: Vector3.create(x, TRIGGER_Y, ROW_Z + 1),
      scale: Vector3.create(2.5, 2.5, 2.5),
    })
    TriggerArea.setBox(trigger, ColliderLayer.CL_CUSTOM1)

    makeBall(Vector3.create(x, TRIGGER_Y, ROW_Z + 1), ColliderLayer.CL_CUSTOM2)

    // If the SDK ever fires here, the visual diff will catch the regression.
    triggerAreaEventsSystem.onTriggerEnter(trigger, () => {
      paint(indicator, COLOR_GREEN)
    })

    for (const e of [indicator, labelE, pedestals[2]]) attachHint(e, text)
  }

  // ── Col 3 — Box trigger multi-mask (CL_CUSTOM1 | CL_CUSTOM2) ──────────────
  // Set-up: trigger mask is the OR of two layers, passed as an array to
  // setBox(). Ball collider is on CL_CUSTOM2 — only one of the two bits is
  // set, but that's enough; the AND of (CUSTOM1|CUSTOM2) & CUSTOM2 = CUSTOM2,
  // i.e. non-zero, so the trigger fires.
  //
  // What the test asserts: the array form of setBox() correctly OR's the
  // layers into a single mask. Indicator GREEN.
  {
    const x = COLUMNS_X[3]
    const text = 'Multi-mask\nCUSTOM1|CUSTOM2\nball on CUSTOM2\n→ GREEN'
    const indicator = makeIndicator(x, INDICATOR_TRIGGER_Y)
    const labelE = makeLabel(x, LABEL_BOTTOM_Y, text)

    const trigger = engine.addEntity()
    Transform.create(trigger, {
      position: Vector3.create(x, TRIGGER_Y, ROW_Z + 1),
      scale: Vector3.create(2.5, 2.5, 2.5),
    })
    TriggerArea.setBox(trigger, [ColliderLayer.CL_CUSTOM1, ColliderLayer.CL_CUSTOM2])

    makeBall(Vector3.create(x, TRIGGER_Y, ROW_Z + 1), ColliderLayer.CL_CUSTOM2)

    triggerAreaEventsSystem.onTriggerEnter(trigger, () => {
      paint(indicator, COLOR_GREEN)
    })

    for (const e of [indicator, labelE, pedestals[3]]) attachHint(e, text)
  }

  // ── Col 4 — Box trigger + CL_PLAYER (avatar fires it) ─────────────────────
  // Set-up: trigger uses default mask (CL_PLAYER) — no orange ball needed,
  // the player's avatar IS the triggering entity. The trigger volume is
  // tall and reaches the ground (y=0..3.6) so the avatar capsule (~1.8 m
  // tall, feet at y=0) overlaps it completely.
  //
  // We CAN'T rely on the scene.json spawnPoint alone — the runtime treats
  // it as a hint and the avatar can end up elsewhere (loading delays,
  // spawn-collision adjustment, fallback spawn behaviour, etc.). The test
  // explicitly teleports the avatar with movePlayerTo(), and the scene
  // declares ALLOW_TO_MOVE_PLAYER_INSIDE_SCENE in scene.json so the call
  // doesn't require a user-interaction grant.
  //
  // RETRY LOOP. A single fire-and-forget movePlayerTo() in main() is not
  // reliable in practice — the host may not yet have registered the avatar
  // IPC channel when main() runs, in which case the call is silently
  // dropped. We retry every 0.4 s from a system until either the trigger
  // fires (success → stop) or 6 s pass (give up — the SDK has a real bug
  // worth surfacing in the next screenshot anyway). The retry is also a
  // safety net in case the auto-screenshot occurs before the first
  // teleport propagates.
  //
  // The avatar is hidden by the AvatarModifierArea in setupVisualTest, but
  // hide-avatar only suppresses RENDERING; collisions and triggers still
  // see the avatar.
  //
  // What the test asserts: CL_PLAYER is the default trigger mask, and an
  // avatar inside the volume fires the trigger. Indicator GREEN.
  //
  // We subscribe to both ENTER and STAY — whichever the engine emits first
  // paints the indicator. STAY is synthesised every tick the avatar is
  // inside, so it acts as a safety net in case the teleport lands AFTER
  // the only ENTER opportunity (post-teleport the avatar is already inside,
  // not entering).
  {
    const x = COLUMNS_X[4]
    const text = 'Default mask\nCL_PLAYER\navatar moved\ninside → GREEN'
    const indicator = makeIndicator(x, INDICATOR_TRIGGER_Y)
    const labelE = makeLabel(x, LABEL_BOTTOM_Y, text)

    const trigger = engine.addEntity()
    Transform.create(trigger, {
      // Centre at y=1.8 with scale_y=3.6 → volume spans y=0..3.6, covering
      // the full avatar capsule from feet at ground up to head height.
      position: Vector3.create(x, 1.8, ROW_Z),
      scale: Vector3.create(3, 3.6, 3),
    })
    TriggerArea.setBox(trigger) // default mask = CL_PLAYER

    const targetPos = Vector3.create(x, 0.2, ROW_Z)
    const cameraTarget = Vector3.create(x, 1, ROW_Z + 2)
    const moveAvatarIntoTrigger = (): void => {
      void movePlayerTo({ newRelativePosition: targetPos, cameraTarget })
    }

    let triggerFired = false
    triggerAreaEventsSystem.onTriggerEnter(trigger, () => {
      triggerFired = true
      paint(indicator, COLOR_GREEN)
    })
    triggerAreaEventsSystem.onTriggerStay(trigger, () => {
      triggerFired = true
      paint(indicator, COLOR_GREEN)
    })

    // Immediate first attempt — usually a no-op if the avatar isn't ready yet,
    // but cheap to try.
    moveAvatarIntoTrigger()

    let elapsed = 0
    let lastRetry = 0
    const RETRY_INTERVAL = 0.4 // seconds between retries
    const RETRY_DEADLINE = 6.0 // give up after this long
    engine.addSystem((dt: number) => {
      if (triggerFired) return
      elapsed += dt
      if (elapsed > RETRY_DEADLINE) return
      if (elapsed - lastRetry >= RETRY_INTERVAL) {
        lastRetry = elapsed
        moveAvatarIntoTrigger()
      }
    })

    for (const e of [indicator, labelE, pedestals[4]]) attachHint(e, text)
  }

  // ── Col 5 — Box trigger + CL_CUSTOM1 + onTriggerStay only ─────────────────
  // Same trigger geometry as Col 0 but subscribes ONLY to onTriggerStay
  // (NOT onTriggerEnter). The SDK synthesises STAY callbacks on every tick
  // between a wire ENTER and a wire EXIT — so the fact STAY fires here is
  // direct evidence the SDK's enter/exit state machine is tracking the
  // ball correctly.
  //
  // The colour is BLUE (not green) so the visual diff distinguishes this
  // cell from the ENTER-based cells around it. A green here would suggest
  // the wrong callback fired.
  {
    const x = COLUMNS_X[5]
    const text = 'STAY callback\nsubscribed only\nto onTriggerStay\n→ BLUE'
    const indicator = makeIndicator(x, INDICATOR_TRIGGER_Y)
    const labelE = makeLabel(x, LABEL_BOTTOM_Y, text)

    const trigger = engine.addEntity()
    Transform.create(trigger, {
      position: Vector3.create(x, TRIGGER_Y, ROW_Z + 1),
      scale: Vector3.create(2.5, 2.5, 2.5),
    })
    TriggerArea.setBox(trigger, ColliderLayer.CL_CUSTOM1)

    makeBall(Vector3.create(x, TRIGGER_Y, ROW_Z + 1), ColliderLayer.CL_CUSTOM1)

    triggerAreaEventsSystem.onTriggerStay(trigger, () => {
      paint(indicator, COLOR_BLUE)
    })

    for (const e of [indicator, labelE, pedestals[5]]) attachHint(e, text)
  }

  // ═══════════════════════════════════════════════════════════════════════════
  // TOP ROW — Raycast cases
  // ═══════════════════════════════════════════════════════════════════════════
  //
  // Every raycast is registered with continuous=true so it re-fires each tick
  // until the screenshot. That way a first-frame race (e.g. the source's
  // Transform not yet committed when the engine evaluates the ray) is
  // harmless — the second tick will hit and paint the indicator. paint() is
  // idempotent so subsequent ticks no-op visually.

  // ── Col 0 — Local-direction raycast ───────────────────────────────────────
  // Set-up: an invisible source entity at (3, 7, 10). A purple target sphere
  // sits 2.5 m ahead at (3, 7, 12.5). The raycast is registered with a
  // LOCAL direction Vector3.Forward() — the engine interprets that vector
  // in the source's LOCAL frame. Source rotation is Identity, so local-fwd
  // happens to equal world-fwd here.
  //
  // What the test asserts: local-direction raycasts produce hits. Indicator
  // GREEN.
  {
    const x = COLUMNS_X[0]
    const text = 'Local-direction ray\nsource → forward\nhits target\n→ GREEN'
    const indicator = makeIndicator(x, INDICATOR_RAYCAST_Y)
    const labelE = makeLabel(x, LABEL_TOP_Y, text)

    const source = engine.addEntity()
    Transform.create(source, {
      position: Vector3.create(x, RAYCAST_Y, ROW_Z),
      rotation: Quaternion.Identity(),
    })

    makeTarget(Vector3.create(x, RAYCAST_Y, ROW_Z + 2.5))

    raycastSystem.registerLocalDirectionRaycast(
      {
        entity: source,
        opts: {
          direction: Vector3.Forward(),
          maxDistance: 5,
          queryType: RaycastQueryType.RQT_HIT_FIRST,
          continuous: true,
        },
      },
      (result) => {
        if (result.hits.length > 0) paint(indicator, COLOR_GREEN)
      },
    )

    for (const e of [indicator, labelE]) attachHint(e, text)
  }

  // ── Col 1 — Global-direction raycast ──────────────────────────────────────
  // Set-up: source is RAISED 1.5 m above its target. Direction is world-Down
  // (globalDirection), NOT a local vector. The source's rotation is
  // deliberately non-identity (yawed 90°) to PROVE the global mode ignores
  // it; if the SDK accidentally interpreted the vector in local space the
  // ray would shoot sideways and miss.
  //
  // What the test asserts: global-direction raycasts use world axes
  // regardless of source rotation. Indicator GREEN.
  {
    const x = COLUMNS_X[1]
    const text = 'Global-direction ray\nworld-down ↓\nignores source rot\n→ GREEN'
    const indicator = makeIndicator(x, INDICATOR_RAYCAST_Y)
    const labelE = makeLabel(x, LABEL_TOP_Y, text)

    const source = engine.addEntity()
    Transform.create(source, {
      position: Vector3.create(x, RAYCAST_Y + 1.5, ROW_Z + 1),
      // Non-identity rotation — global mode must ignore this.
      rotation: Quaternion.fromEulerDegrees(0, 90, 0),
    })

    makeTarget(Vector3.create(x, RAYCAST_Y - 0.5, ROW_Z + 1))

    raycastSystem.registerGlobalDirectionRaycast(
      {
        entity: source,
        opts: {
          direction: Vector3.Down(),
          maxDistance: 4,
          queryType: RaycastQueryType.RQT_HIT_FIRST,
          continuous: true,
        },
      },
      (result) => {
        if (result.hits.length > 0) paint(indicator, COLOR_GREEN)
      },
    )

    for (const e of [indicator, labelE]) attachHint(e, text)
  }

  // ── Col 2 — Global-target raycast ─────────────────────────────────────────
  // Set-up: instead of a direction vector, the raycast is given a world-space
  // POINT (globalTarget) at (x, RAYCAST_Y, ROW_Z+3.5). The engine derives
  // direction as (target − source). The visible purple target sphere is
  // placed at (x, RAYCAST_Y, ROW_Z+2.5) — 1 m in front of the world point
  // — so the ray hits the sphere on its way to the unrendered global-target
  // point.
  //
  // What the test asserts: target-based aiming derives the ray direction
  // correctly. Indicator GREEN.
  {
    const x = COLUMNS_X[2]
    const text = 'Global-target ray\naimed at a point\nhits target en route\n→ GREEN'
    const indicator = makeIndicator(x, INDICATOR_RAYCAST_Y)
    const labelE = makeLabel(x, LABEL_TOP_Y, text)

    const source = engine.addEntity()
    Transform.create(source, {
      position: Vector3.create(x, RAYCAST_Y, ROW_Z),
      rotation: Quaternion.Identity(),
    })

    makeTarget(Vector3.create(x, RAYCAST_Y, ROW_Z + 2.5))

    raycastSystem.registerGlobalTargetRaycast(
      {
        entity: source,
        opts: {
          target: Vector3.create(x, RAYCAST_Y, ROW_Z + 3.5),
          maxDistance: 5,
          queryType: RaycastQueryType.RQT_HIT_FIRST,
          continuous: true,
        },
      },
      (result) => {
        if (result.hits.length > 0) paint(indicator, COLOR_GREEN)
      },
    )

    for (const e of [indicator, labelE]) attachHint(e, text)
  }

  // ── Col 3 — Target-entity raycast ─────────────────────────────────────────
  // Set-up: ray is told to track a specific ENTITY each tick (targetEntity).
  // The direction is recomputed from the target's live position — so if the
  // target ever moved, the ray would follow.
  //
  // What the test asserts: entity-tracking aim mode resolves the target's
  // current position and produces a hit on it. Indicator GREEN.
  {
    const x = COLUMNS_X[3]
    const text = 'Target-entity ray\ntracks an entity\nhits it each tick\n→ GREEN'
    const indicator = makeIndicator(x, INDICATOR_RAYCAST_Y)
    const labelE = makeLabel(x, LABEL_TOP_Y, text)

    const source = engine.addEntity()
    Transform.create(source, {
      position: Vector3.create(x, RAYCAST_Y, ROW_Z),
      rotation: Quaternion.Identity(),
    })

    const target = makeTarget(Vector3.create(x, RAYCAST_Y, ROW_Z + 2.5))

    raycastSystem.registerTargetEntityRaycast(
      {
        entity: source,
        opts: {
          targetEntity: target,
          maxDistance: 5,
          queryType: RaycastQueryType.RQT_HIT_FIRST,
          continuous: true,
        },
      },
      (result) => {
        if (result.hits.length > 0) paint(indicator, COLOR_GREEN)
      },
    )

    for (const e of [indicator, labelE]) attachHint(e, text)
  }

  // ── Col 4 — collisionMask mismatch (negative) ─────────────────────────────
  // Set-up: same geometry as Col 0 BUT the ray's collisionMask is restricted
  // to CL_CUSTOM1, and the target's collider is set to CL_CUSTOM2. The bits
  // don't overlap so the ray's mask AND the target's mask = 0 — the engine
  // treats the target as invisible to this ray.
  //
  // What the test asserts: raycast collisionMask actually filters. The ray
  // returns hits.length === 0 every tick → indicator stays GRAY. If gray
  // ever turns green, the SDK is leaking through the mask.
  {
    const x = COLUMNS_X[4]
    const text = 'Wrong mask\nray=CUSTOM1\ntarget=CUSTOM2\n→ stays GRAY'
    const indicator = makeIndicator(x, INDICATOR_RAYCAST_Y)
    const labelE = makeLabel(x, LABEL_TOP_Y, text)

    const source = engine.addEntity()
    Transform.create(source, {
      position: Vector3.create(x, RAYCAST_Y, ROW_Z),
      rotation: Quaternion.Identity(),
    })

    makeTarget(Vector3.create(x, RAYCAST_Y, ROW_Z + 2.5), ColliderLayer.CL_CUSTOM2)

    raycastSystem.registerLocalDirectionRaycast(
      {
        entity: source,
        opts: {
          direction: Vector3.Forward(),
          maxDistance: 5,
          queryType: RaycastQueryType.RQT_HIT_FIRST,
          collisionMask: ColliderLayer.CL_CUSTOM1,
          continuous: true,
        },
      },
      (result) => {
        if (result.hits.length > 0) paint(indicator, COLOR_GREEN)
      },
    )

    for (const e of [indicator, labelE]) attachHint(e, text)
  }

  // ── Col 5 — RQT_QUERY_ALL with multiple targets in line ───────────────────
  // Set-up: three purple target spheres are stacked along the ray at
  // z=ROW_Z+1.5, +2.5, +3.5 — all on the default collider mask
  // (CL_POINTER|CL_PHYSICS). The raycast uses RQT_QUERY_ALL instead of
  // RQT_HIT_FIRST so it should return ALL intersected colliders, not just
  // the nearest one.
  //
  // What the test asserts: QUERY_ALL returns more than one hit when more
  // than one collider is on the ray. The callback checks hits.length ≥ 2
  // and paints YELLOW — a colour distinct from green so the screenshot can
  // tell QUERY_ALL apart from HIT_FIRST. If only one hit comes back, the
  // indicator stays gray and the diff catches it.
  {
    const x = COLUMNS_X[5]
    const text = 'QUERY_ALL\nhits ≥ 2 of 3\nstacked targets\n→ YELLOW'
    const indicator = makeIndicator(x, INDICATOR_RAYCAST_Y)
    const labelE = makeLabel(x, LABEL_TOP_Y, text)

    const source = engine.addEntity()
    Transform.create(source, {
      position: Vector3.create(x, RAYCAST_Y, ROW_Z),
      rotation: Quaternion.Identity(),
    })

    // Three targets along +Z, spaced 1 m apart.
    makeTarget(Vector3.create(x, RAYCAST_Y, ROW_Z + 1.5))
    makeTarget(Vector3.create(x, RAYCAST_Y, ROW_Z + 2.5))
    makeTarget(Vector3.create(x, RAYCAST_Y, ROW_Z + 3.5))

    raycastSystem.registerLocalDirectionRaycast(
      {
        entity: source,
        opts: {
          direction: Vector3.Forward(),
          maxDistance: 5,
          queryType: RaycastQueryType.RQT_QUERY_ALL,
          continuous: true,
        },
      },
      (result) => {
        if (result.hits.length >= 2) paint(indicator, COLOR_YELLOW)
      },
    )

    for (const e of [indicator, labelE]) attachHint(e, text)
  }

  // ── scene title ──────────────────────────────────────────────────────────
  const titleText = 'Triggers · Raycasts · test matrix'
  const titleE = engine.addEntity()
  Transform.create(titleE, { position: Vector3.create(15.5, TITLE_Y, ROW_Z) })
  TextShape.create(titleE, {
    text: titleText,
    fontSize: TITLE_FONT_SIZE,
    textColor: COLOR_BLACK,
    textWrapping: false,
  })
  MeshCollider.setPlane(titleE, ColliderLayer.CL_POINTER)
  attachHint(titleE, titleText)

  // Row dividers (text labels at the far-left margin for each row).
  const bottomRowText = 'TRIGGERS row'
  const rowLabelBottom = engine.addEntity()
  Transform.create(rowLabelBottom, { position: Vector3.create(0.4, TRIGGER_Y, 7) })
  TextShape.create(rowLabelBottom, {
    text: 'TRIGGERS →',
    fontSize: ROW_LABEL_FONT_SIZE,
    textColor: COLOR_BLACK,
    textWrapping: false,
  })
  MeshCollider.setPlane(rowLabelBottom, ColliderLayer.CL_POINTER)
  attachHint(rowLabelBottom, bottomRowText)

  const topRowText = 'RAYCASTS row'
  const rowLabelTop = engine.addEntity()
  Transform.create(rowLabelTop, { position: Vector3.create(0.4, RAYCAST_Y, 7) })
  TextShape.create(rowLabelTop, {
    text: 'RAYCASTS →',
    fontSize: ROW_LABEL_FONT_SIZE,
    textColor: COLOR_BLACK,
    textWrapping: false,
  })
  MeshCollider.setPlane(rowLabelTop, ColliderLayer.CL_POINTER)
  attachHint(rowLabelTop, topRowText)

  // ── camera ────────────────────────────────────────────────────────────────
  // The composition now spans y≈0 (ground) to y≈14.6 (top of title text), so
  // the camera is centred near y=7 to fit it all in frame. Camera is pulled
  // back to z=−6 so the increased vertical extent + the column spread
  // (x=3..28) both fit inside the default ~60° VirtualCamera FOV.
  setupVisualTest({
    lookAtPos: Vector3.create(15.5, 6, ROW_Z),
    cameraPos: Vector3.create(15.5, 6, -6),
    // 2-parcel scene — widen the AvatarModifierArea to cover x=0..32, z=0..16.
    hideAreaCenter: Vector3.create(16, 8, 8),
    hideAreaSize: Vector3.create(32, 16, 16),
  })
}
