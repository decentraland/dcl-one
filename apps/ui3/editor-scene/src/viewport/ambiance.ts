import { engine, Transform, MeshRenderer, Material } from '@dcl/sdk/ecs'
import { BevyApi } from '../bevy-api'
import { log } from '../log'
import { state } from '../state'
import { PARCEL, gridLayout, type GridLine } from './grid-layout'

// Editor-viewport ambiance: freeze the skybox at dusk and lay a holo-grid
// ground over the scene's parcels. These entities live in the editor system
// scene (a separate scene context from the edited project), so they are
// chrome: never part of the edited scene, its saves, or its entity budget.
//
// The grid is MESH-BASED on purpose. The first cut textured a plane with a
// grid PNG, which put editor chrome through the whole texture pipeline
// (service-worker ipfs cache -> image_processing worker -> BC7/DDS) and
// rendered as the engine's pink fallback when any link failed. Thin emissive
// boxes have no pipeline to fail and their colors are exact.
const DUSK_HOURS = '20.8'
// The ENGINE spawns its own PreviewGridPlane per parcel at y=-0.01 in preview
// mode (scene_runner initialize_scene/loading_quads.rs) -- an opaque white
// grid.png plane. Our grid must sit ABOVE it or it is fully occluded (the
// original -0.02 put the whole holo-grid 5mm under it), and below y=0 so
// user-placed floors do not z-fight editor chrome.
const GRID_Y = -0.008
const LINE_W = 0.05
const LINE_H = 0.005

const BASE_COLOR = { r: 0.027, g: 0.04, b: 0.055, a: 1 }
const MINOR_COLOR = { r: 0.07, g: 0.35, b: 0.45 }
const MAJOR_COLOR = { r: 0.15, g: 0.75, b: 0.95 }

let spawned = false

function line({ alongX, x, z, length, major }: GridLine): void {
  const e = engine.addEntity()
  Transform.create(e, {
    position: {
      x: alongX ? x + length / 2 : x,
      y: GRID_Y + LINE_H,
      z: alongX ? z : z + length / 2
    },
    scale: {
      x: alongX ? length : LINE_W,
      y: LINE_H,
      z: alongX ? LINE_W : length
    }
  })
  MeshRenderer.setBox(e)
  const c = major ? MAJOR_COLOR : MINOR_COLOR
  // Emissive-driven on purpose: the frozen dusk sun tints lit albedo violet
  // (the same reason the textured plane went unlit). Emission ignores scene
  // lighting, so the colors below are what the user actually sees.
  // Intensities stay LOW: at 2.5/1.2 the default bloom (0.15) haloed the
  // lines into a white haze that washed the whole slab pale -- the grid read
  // as a milky plane instead of cyan-on-navy.
  Material.setPbrMaterial(e, {
    albedoColor: { r: 0, g: 0, b: 0, a: 1 },
    emissiveColor: c,
    emissiveIntensity: major ? 0.9 : 0.35,
    metallic: 0,
    roughness: 1
  })
}

export function setupEditorAmbiance(): void {
  engine.addSystem(() => {
    if (spawned) return
    if (state.status !== 'ready') return
    spawned = true

    BevyApi.consoleCommand('time', [DUSK_HOURS, '0']).catch((e) =>
      log.debug('ambiance: time command failed', e)
    )

    const layout = gridLayout(state.scene?.parcels ?? [{ x: 0, y: 0 }])
    console.log(
      `[ambiance] spawning mesh grid (unlit): ${layout.detail}, ${layout.slabs.length + layout.lines.length} entities`
    )
    for (const slab of layout.slabs) {
      const base = engine.addEntity()
      Transform.create(base, {
        position: { x: slab.x + slab.width / 2, y: GRID_Y, z: slab.z + PARCEL / 2 },
        scale: { x: slab.width, y: 0.002, z: PARCEL }
      })
      MeshRenderer.setBox(base)
      // Same emissive trick as the lines: a dark ALBEDO slab under the pink
      // dusk sun rendered as the "pink tiled plane" bug -- lit albedo takes
      // the sun's color. Black albedo + faint navy emission stays navy.
      Material.setPbrMaterial(base, {
        albedoColor: { r: 0, g: 0, b: 0, a: 1 },
        // The dusk exposure + bloom amplify emissives hard: a diagnostic pure
        // red at intensity 1.0 rendered as a glowing peach-white plane. The
        // slab must stay a whisper above black, so intensity lives near zero.
        emissiveColor: { r: BASE_COLOR.r, g: BASE_COLOR.g, b: BASE_COLOR.b },
        emissiveIntensity: 0.08,
        metallic: 0,
        roughness: 1
      })
    }
    for (const l of layout.lines) line(l)
  })
}
