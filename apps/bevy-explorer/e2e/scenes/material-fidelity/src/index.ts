import type {
  Entity,
  PBMaterial_PbrMaterial,
  PBMaterial_UnlitMaterial,
} from '@dcl/sdk/ecs'
import {
  AvatarModifierArea,
  AvatarModifierType,
  GltfContainer,
  GltfNodeModifiers,
  InputAction,
  MainCamera,
  Material,
  MaterialTransparencyMode,
  MeshRenderer,
  PointerEventType,
  TextAlignMode,
  TextShape,
  TextureFilterMode,
  TextureWrapMode,
  Transform,
  VirtualCamera,
  engine,
  inputSystem,
} from '@dcl/sdk/ecs'
import { Color3, Color4, Quaternion, Vector2, Vector3 } from '@dcl/sdk/math'

/**
 * material-fidelity: charts of the SDK material for comparing two clients. `../README.md` lists
 * what each board holds.
 *
 * Each board has a virtual camera on its normal, so two clients at the same board render the
 * same frame. The scene adds no light: the sun and the sky of each client light the boards.
 */

const LAVA = 'assets/lava.jpg'
const SMOKE = 'assets/smoke.png' // white puff, alpha 0 to 0.97
const BUMPS = 'assets/bumps-normal.png' // tangent-space normal map, +Y up
// Red top-left, green top-right, blue bottom-left, yellow bottom-right, an arrow to the top.
const ORIENTATION = 'assets/orientation.png'
// A 16 px checker with one dark corner cell; each file is used by one cell only.
const checker = (name: string) => `assets/checker-${name}.png`

const COLS = 6
const ROWS = 6
const COL_SPACING = 2
const ROW_SPACING = 1.9
const RADIUS = 0.55
const STATION_SPACING = 20
const FIRST_STATION_X = 10
const BOARD_CENTRE_Y = 4.9
const BOARD_CENTRE_Z = 9.5
const BOARD_TILT = 45
const CAMERA_DISTANCE = 11.5

type Pbr = Partial<PBMaterial_PbrMaterial>
type Unlit = Partial<PBMaterial_UnlitMaterial>
/** `planeUvs` is a plane with its four corners' UVs given, the same picture as the default. */
type Shape = 'sphere' | 'box' | 'plane' | 'cylinder' | 'cone' | 'planeUvs'
/**
 * `'none'` is a shape with no Material component. `turn` rolls the shape about the board's
 * normal, in degrees. `modifier` is put on every node of the model.
 */
type Cell =
  | { pbr: Pbr; shape?: Shape; turn?: number }
  | { unlit: Unlit; shape?: Shape }
  | { gltf: string; modifier?: Pbr }
  | 'none'
type Row = { label: string; cells: Cell[] }
type Station = { title: string; rows: Row[]; tiles?: boolean }

const RAMP = [0, 0.2, 0.4, 0.6, 0.8, 1]
const RAMP_LABEL = '0 · 0.2 · 0.4 · 0.6 · 0.8 · 1'

const WHITE = Color4.create(0.9, 0.9, 0.9, 1)
const RED = Color4.create(0.9, 0.2, 0.15, 1)
const BLUE = Color4.create(0.1, 0.2, 0.6, 1)
const BLACK = Color4.create(0, 0, 0, 1)

const HUES: Color3[] = [
  Color3.create(1, 0, 0),
  Color3.create(0, 1, 0),
  Color3.create(0, 0, 1),
  Color3.create(1, 1, 0),
  Color3.create(0, 1, 1),
  Color3.create(1, 0, 1),
]
const HUES_LABEL = 'red · green · blue · yellow · cyan · magenta'

const tex = (src: string) => Material.Texture.Common({ src })
const pbrRow = (label: string, cells: Pbr[]): Row => ({ label, cells: cells.map((pbr) => ({ pbr })) })
const unlitRow = (label: string, cells: Unlit[]): Row => ({ label, cells: cells.map((unlit) => ({ unlit })) })
const withAlpha = (c: Color4, a: number) => Color4.create(c.r, c.g, c.b, a)

function metallicByRoughness(albedoColor: Color4): Row[] {
  return RAMP.map((metallic) =>
    pbrRow(
      `metallic ${metallic} · roughness ${RAMP_LABEL}`,
      RAMP.map((roughness) => ({ albedoColor, metallic, roughness })),
    ),
  )
}

const REFLECTIVITY: Color3[] = [
  Color3.create(1, 1, 1),
  Color3.create(0.5, 0.5, 0.5),
  Color3.create(0, 0, 0),
  Color3.create(1, 0, 0),
  Color3.create(0, 1, 0),
  Color3.create(0, 0, 1),
]
const REFLECTIVITY_LABEL = 'white · grey · black · red · green · blue'
const DIRECT = [0, 0.5, 1, 2, 4, 8]

// Unit cubes with normals and uvs and no tangents (assets/cube-*.glb).
const cube = (name: string) => `assets/cube-${name}.glb`
const BUMPED: Pbr = { albedoColor: WHITE, metallic: 0, roughness: 0.4, bumpTexture: tex(BUMPS) }
const SHAPES: Shape[] = ['sphere', 'box', 'plane', 'cylinder', 'cone', 'planeUvs']
const SHAPES_LABEL = 'sphere · box · plane · cylinder · cone · plane with uvs'
const GREYS = [0, 0.1, 0.25, 0.5, 0.75, 1]
const grey = (v: number) => Color4.create(v, v, v, 1)
type Sampling = Omit<Parameters<typeof Material.Texture.Common>[0], 'src'>
const sampled = (name: string, texture: Sampling): Pbr => ({
  texture: Material.Texture.Common({ src: checker(name), ...texture }),
  metallic: 0,
  roughness: 1,
})

const smokePbr = (extra: Pbr): Pbr => ({ albedoColor: RED, texture: tex(SMOKE), metallic: 0, roughness: 0.5, ...extra })

const STATIONS: Station[] = [
  { title: '1 · Metallic by roughness, white albedo', rows: metallicByRoughness(WHITE) },
  { title: '2 · Metallic by roughness, red albedo', rows: metallicByRoughness(RED) },
  {
    title: '3 · Specular, reflectivity and direct light',
    rows: [
      pbrRow(
        `specularIntensity ${RAMP_LABEL} · metallic 0, roughness 0.2`,
        RAMP.map((specularIntensity) => ({ albedoColor: BLUE, metallic: 0, roughness: 0.2, specularIntensity })),
      ),
      pbrRow(
        `specularIntensity ${RAMP_LABEL} · metallic 1, roughness 0.2`,
        RAMP.map((specularIntensity) => ({ albedoColor: WHITE, metallic: 1, roughness: 0.2, specularIntensity })),
      ),
      pbrRow(
        `reflectivityColor ${REFLECTIVITY_LABEL} · metallic 0, roughness 0.2`,
        REFLECTIVITY.map((reflectivityColor) => ({ albedoColor: WHITE, metallic: 0, roughness: 0.2, reflectivityColor })),
      ),
      pbrRow(
        `reflectivityColor ${REFLECTIVITY_LABEL} · metallic 1, roughness 0.2`,
        REFLECTIVITY.map((reflectivityColor) => ({ albedoColor: WHITE, metallic: 1, roughness: 0.2, reflectivityColor })),
      ),
      pbrRow(
        `directIntensity ${DIRECT.join(' · ')} · metallic 0, roughness 0.5`,
        DIRECT.map((directIntensity) => ({ albedoColor: RED, metallic: 0, roughness: 0.5, directIntensity })),
      ),
      pbrRow(
        `directIntensity ${DIRECT.join(' · ')} · metallic 1, roughness 0.3`,
        DIRECT.map((directIntensity) => ({ albedoColor: WHITE, metallic: 1, roughness: 0.3, directIntensity })),
      ),
    ],
  },
  {
    title: '4 · Emissive',
    rows: [
      pbrRow(
        'emissiveIntensity 0 · 0.5 · 1 · 2 · 4 · 8 · white emissive, black albedo',
        [0, 0.5, 1, 2, 4, 8].map((emissiveIntensity) => ({
          albedoColor: BLACK,
          emissiveColor: Color3.White(),
          emissiveIntensity,
        })),
      ),
      pbrRow(
        'emissiveIntensity 16 · 32 · 64 · 128 · 256 · 512 · white emissive, black albedo',
        [16, 32, 64, 128, 256, 512].map((emissiveIntensity) => ({
          albedoColor: BLACK,
          emissiveColor: Color3.White(),
          emissiveIntensity,
        })),
      ),
      pbrRow(
        `emissiveColor ${HUES_LABEL} · default intensity, black albedo`,
        HUES.map((emissiveColor) => ({ albedoColor: BLACK, emissiveColor })),
      ),
      pbrRow(
        `emissiveColor ${HUES_LABEL} · default intensity, white albedo`,
        HUES.map((emissiveColor) => ({ albedoColor: WHITE, emissiveColor })),
      ),
      pbrRow(
        'emissiveTexture, white emissive · emissiveIntensity 0 · 0.5 · 1 · 2 · 4 · 8',
        [0, 0.5, 1, 2, 4, 8].map((emissiveIntensity) => ({
          albedoColor: BLACK,
          emissiveTexture: tex(LAVA),
          emissiveColor: Color3.White(),
          emissiveIntensity,
        })),
      ),
      pbrRow(
        `emissiveTexture · emissiveColor ${HUES_LABEL}`,
        HUES.map((emissiveColor) => ({ albedoColor: BLACK, emissiveTexture: tex(LAVA), emissiveColor })),
      ),
    ],
  },
  {
    title: '5 · Transparency',
    tiles: true,
    rows: [
      pbrRow(
        `alpha blend · albedo alpha ${RAMP_LABEL}`,
        RAMP.map((a) => ({
          albedoColor: withAlpha(RED, a),
          transparencyMode: MaterialTransparencyMode.MTM_ALPHA_BLEND,
        })),
      ),
      pbrRow(
        `auto (mode unset) · albedo alpha ${RAMP_LABEL}`,
        RAMP.map((a) => ({ albedoColor: withAlpha(RED, a) })),
      ),
      pbrRow(
        `opaque · albedo alpha ${RAMP_LABEL}`,
        RAMP.map((a) => ({ albedoColor: withAlpha(RED, a), transparencyMode: MaterialTransparencyMode.MTM_OPAQUE })),
      ),
      pbrRow(
        `alpha test, smoke texture · alphaTest ${RAMP_LABEL}`,
        RAMP.map((alphaTest) => smokePbr({ transparencyMode: MaterialTransparencyMode.MTM_ALPHA_TEST, alphaTest })),
      ),
      pbrRow('smoke texture · opaque · test · blend · test and blend · auto · unset', [
        smokePbr({ transparencyMode: MaterialTransparencyMode.MTM_OPAQUE }),
        smokePbr({ transparencyMode: MaterialTransparencyMode.MTM_ALPHA_TEST }),
        smokePbr({ transparencyMode: MaterialTransparencyMode.MTM_ALPHA_BLEND }),
        smokePbr({ transparencyMode: MaterialTransparencyMode.MTM_ALPHA_TEST_AND_ALPHA_BLEND }),
        smokePbr({ transparencyMode: MaterialTransparencyMode.MTM_AUTO }),
        smokePbr({}),
      ]),
      pbrRow('blend, alpha 0.5 · rough 0 · rough 1 · metal rough 0 · metal rough 0.5 · emissive · alphaTexture', [
        { albedoColor: withAlpha(RED, 0.5), metallic: 0, roughness: 0 },
        { albedoColor: withAlpha(RED, 0.5), metallic: 0, roughness: 1 },
        { albedoColor: withAlpha(WHITE, 0.5), metallic: 1, roughness: 0 },
        { albedoColor: withAlpha(WHITE, 0.5), metallic: 1, roughness: 0.5 },
        { albedoColor: withAlpha(BLACK, 0.5), emissiveColor: Color3.create(0, 1, 1) },
        // The deprecated PBR alpha texture, on an otherwise opaque material.
        { albedoColor: RED, alphaTexture: tex(SMOKE) },
      ].map((cell) => ({ transparencyMode: MaterialTransparencyMode.MTM_ALPHA_BLEND, ...cell }))),
    ],
  },
  {
    title: '6 · Textures, unlit and defaults',
    rows: [
      pbrRow('albedo texture · plain · red tint · grey tint · metal · roughness 0 · roughness 1', [
        { texture: tex(LAVA) },
        { texture: tex(LAVA), albedoColor: RED },
        { texture: tex(LAVA), albedoColor: Color4.create(0.5, 0.5, 0.5, 1) },
        { texture: tex(LAVA), metallic: 1, roughness: 0.2 },
        { texture: tex(LAVA), metallic: 0, roughness: 0 },
        { texture: tex(LAVA), metallic: 0, roughness: 1 },
      ]),
      pbrRow('bumpTexture · none · rough 0.5 · rough 0.1 · metal · with albedo texture · with emissive', [
        { albedoColor: WHITE, metallic: 0, roughness: 0.5 },
        { albedoColor: WHITE, metallic: 0, roughness: 0.5, bumpTexture: tex(BUMPS) },
        { albedoColor: WHITE, metallic: 0, roughness: 0.1, bumpTexture: tex(BUMPS) },
        { albedoColor: WHITE, metallic: 1, roughness: 0.2, bumpTexture: tex(BUMPS) },
        { texture: tex(LAVA), bumpTexture: tex(BUMPS) },
        { albedoColor: BLACK, emissiveColor: Color3.create(1, 0.5, 0), bumpTexture: tex(BUMPS) },
      ]),
      pbrRow('sampling · repeat 2x · mirror 4x · clamp 2x, offset · point · bilinear · trilinear', [
        { src: LAVA, wrapMode: TextureWrapMode.TWM_REPEAT, tiling: Vector2.create(2, 2) },
        { src: LAVA, wrapMode: TextureWrapMode.TWM_MIRROR, tiling: Vector2.create(4, 4) },
        { src: LAVA, wrapMode: TextureWrapMode.TWM_CLAMP, tiling: Vector2.create(2, 2), offset: Vector2.create(-0.5, -0.5) },
        { src: LAVA, filterMode: TextureFilterMode.TFM_POINT, tiling: Vector2.create(0.1, 0.1) },
        { src: LAVA, filterMode: TextureFilterMode.TFM_BILINEAR, tiling: Vector2.create(0.1, 0.1) },
        { src: LAVA, filterMode: TextureFilterMode.TFM_TRILINEAR, tiling: Vector2.create(0.1, 0.1) },
      ].map((texture) => ({ texture: Material.Texture.Common(texture) }))),
      unlitRow('unlit · white · red · grey · texture · texture, red tint · smoke texture', [
        { diffuseColor: Color4.White() },
        { diffuseColor: RED },
        { diffuseColor: Color4.create(0.5, 0.5, 0.5, 1) },
        { texture: tex(LAVA) },
        { texture: tex(LAVA), diffuseColor: RED },
        { texture: tex(SMOKE), diffuseColor: RED },
      ]),
      unlitRow('unlit alphaTexture · alphaTest 0 · 0.25 · 0.5 (unset) · 0.75 · 1 · diffuse alpha 0.5', [
        { diffuseColor: RED, alphaTexture: tex(SMOKE), alphaTest: 0 },
        { diffuseColor: RED, alphaTexture: tex(SMOKE), alphaTest: 0.25 },
        { diffuseColor: RED, alphaTexture: tex(SMOKE) },
        { diffuseColor: RED, alphaTexture: tex(SMOKE), alphaTest: 0.75 },
        { diffuseColor: RED, alphaTexture: tex(SMOKE), alphaTest: 1 },
        { diffuseColor: withAlpha(RED, 0.5) },
      ]),
      {
        label: 'shadows and defaults · PBR casts · PBR does not · unlit casts · unlit does not · no Material · empty PBR',
        cells: [
          { pbr: { albedoColor: RED, castShadows: true } },
          { pbr: { albedoColor: RED, castShadows: false } },
          { unlit: { diffuseColor: RED, castShadows: true } },
          { unlit: { diffuseColor: RED, castShadows: false } },
          'none',
          { pbr: {} },
        ],
      },
    ],
  },
  {
    title: '7 · Orientation, samplers, shapes and emissive rules',
    rows: [
      {
        label: `orientation texture · ${SHAPES_LABEL}`,
        cells: SHAPES.map((shape) => ({ pbr: { texture: tex(ORIENTATION), metallic: 0, roughness: 1 }, shape })),
      },
      pbrRow('one image each · repeat 2x · mirror 2x · clamp 2x, offset · point · bilinear · tiling 0.5, clamp', [
        sampled('repeat', { wrapMode: TextureWrapMode.TWM_REPEAT, tiling: Vector2.create(2, 2) }),
        sampled('mirror', { wrapMode: TextureWrapMode.TWM_MIRROR, tiling: Vector2.create(2, 2) }),
        sampled('clamp', { wrapMode: TextureWrapMode.TWM_CLAMP, tiling: Vector2.create(2, 2), offset: Vector2.create(-0.5, -0.5) }),
        sampled('point', { filterMode: TextureFilterMode.TFM_POINT }),
        sampled('bilinear', { filterMode: TextureFilterMode.TFM_BILINEAR }),
        { texture: Material.Texture.Common({ src: ORIENTATION, tiling: Vector2.create(0.5, 0.5) }), metallic: 0, roughness: 1 },
      ]),
      {
        label: `bumpTexture, white, roughness 0.4 · ${SHAPES_LABEL}`,
        cells: SHAPES.map((shape) => ({
          pbr: { albedoColor: WHITE, metallic: 0, roughness: 0.4, bumpTexture: tex(BUMPS) },
          shape,
        })),
      },
      pbrRow('emissive · texture only · texture, black · red with albedo texture · empty src · white 1 · grey 0.5 at 1', [
        { albedoColor: BLACK, emissiveTexture: tex(LAVA) },
        { albedoColor: BLACK, emissiveTexture: tex(LAVA), emissiveColor: Color3.Black() },
        { texture: tex(ORIENTATION), emissiveColor: Color3.Red(), emissiveIntensity: 1 },
        { albedoColor: BLACK, emissiveTexture: tex('') },
        { albedoColor: BLACK, emissiveColor: Color3.White(), emissiveIntensity: 1 },
        { albedoColor: BLACK, emissiveColor: Color3.create(0.5, 0.5, 0.5), emissiveIntensity: 1 },
      ]),
      pbrRow(
        `albedo grey ${GREYS.join(' · ')} · metallic 0, roughness 1`,
        GREYS.map((v) => ({ albedoColor: grey(v), metallic: 0, roughness: 1 })),
      ),
      unlitRow(`unlit grey ${GREYS.join(' · ')}`, GREYS.map((v) => ({ diffuseColor: grey(v) }))),
    ],
  },
  {
    title: '8 · Turned bumps and glTF cubes',
    rows: [
      {
        label: 'bumpTexture · plane turned 0 · 90 · 180 · 270 · sphere turned 90 · box turned 90',
        cells: [
          { pbr: BUMPED, shape: 'plane', turn: 0 },
          { pbr: BUMPED, shape: 'plane', turn: 90 },
          { pbr: BUMPED, shape: 'plane', turn: 180 },
          { pbr: BUMPED, shape: 'plane', turn: 270 },
          { pbr: BUMPED, shape: 'sphere', turn: 90 },
          { pbr: BUMPED, shape: 'box', turn: 90 },
        ],
      },
      {
        label: 'glTF cube · as it is · red · bumpTexture · bump, metal · albedo texture · emissive orange',
        cells: [
          { gltf: cube('plain') },
          { gltf: cube('plain'), modifier: { albedoColor: RED, metallic: 0, roughness: 0.5 } },
          { gltf: cube('plain'), modifier: BUMPED },
          { gltf: cube('plain'), modifier: { ...BUMPED, metallic: 1, roughness: 0.2 } },
          { gltf: cube('plain'), modifier: { texture: tex(ORIENTATION), metallic: 0, roughness: 1 } },
          { gltf: cube('plain'), modifier: { albedoColor: BLACK, emissiveColor: Color3.create(1, 0.5, 0), emissiveIntensity: 1 } },
        ],
      },
      {
        label: 'glTF emissive, black base · factor 0.25 · 0.5 · 1 · orange · texture · strength 4',
        cells: ['emissive-025', 'emissive-05', 'emissive-1', 'emissive-orange', 'emissive-texture', 'emissive-strength'].map(
          (name) => ({ gltf: cube(name) }),
        ),
      },
    ],
  },
]

export function main() {
  const centreX = FIRST_STATION_X + ((STATIONS.length - 1) * STATION_SPACING) / 2
  const tilt = Quaternion.fromEulerDegrees(BOARD_TILT, 0, 0)

  // Local +Y runs up the slope and -Z faces the cameras.
  const backing = engine.addEntity()
  Transform.create(backing, { position: Vector3.create(centreX, BOARD_CENTRE_Y, BOARD_CENTRE_Z), rotation: tilt })
  const slab = engine.addEntity()
  Transform.create(slab, {
    parent: backing,
    position: Vector3.create(0, 0, 0.1),
    // a little short of the scene, whose edge fades what touches it
    scale: Vector3.create(STATIONS.length * STATION_SPACING - 2, 13.2, 0.2),
  })
  MeshRenderer.setBox(slab)
  Material.setPbrMaterial(slab, { albedoColor: Color4.create(0.08, 0.08, 0.08, 1), metallic: 0, roughness: 1 })

  const hideAvatars = engine.addEntity()
  Transform.create(hideAvatars, { position: Vector3.create(centreX, 8, 8) })
  AvatarModifierArea.create(hideAvatars, {
    area: Vector3.create(160, 16, 16),
    modifiers: [AvatarModifierType.AMT_HIDE_AVATARS, AvatarModifierType.AMT_DISABLE_PASSPORTS],
    excludeIds: [],
  })

  const cameras = STATIONS.map((station, index) => buildStation(station, index, tilt))

  // Stop `cameras.length` is the free player camera.
  let stop = 0
  const show = (next: number) => {
    stop = (next + cameras.length + 1) % (cameras.length + 1)
    MainCamera.createOrReplace(engine.CameraEntity, { virtualCameraEntity: cameras[stop] })
  }
  show(0)
  engine.addSystem(() => {
    if (inputSystem.isTriggered(InputAction.IA_PRIMARY, PointerEventType.PET_DOWN)) show(stop + 1)
    if (inputSystem.isTriggered(InputAction.IA_SECONDARY, PointerEventType.PET_DOWN)) show(stop - 1)
  })
}

function buildStation(station: Station, index: number, tilt: Quaternion): Entity {
  const centre = Vector3.create(FIRST_STATION_X + index * STATION_SPACING, BOARD_CENTRE_Y, BOARD_CENTRE_Z)
  const root = engine.addEntity()
  Transform.create(root, { position: centre, rotation: tilt })

  label(root, `${station.title}     E next · F previous`, ((ROWS - 1) / 2) * ROW_SPACING + 1.05, 4.5)

  station.rows.forEach((row, r) => {
    const y = ((ROWS - 1) / 2 - r) * ROW_SPACING
    label(root, row.label, y - ROW_SPACING / 2, 3.2)
    row.cells.forEach((cell, c) => {
      const x = (c - (COLS - 1) / 2) * COL_SPACING
      if (station.tiles) tiles(root, x, y)
      const subject = engine.addEntity()
      const place = (rotation: Quaternion, scale: number) =>
        Transform.create(subject, {
          parent: root,
          position: Vector3.create(x, y, -RADIUS),
          scale: Vector3.create(scale, scale, scale),
          rotation,
        })
      if (cell !== 'none' && 'gltf' in cell) {
        place(SOLID_TURN, 1.4 * RADIUS)
        GltfContainer.create(subject, { src: cell.gltf })
        if (cell.modifier) {
          GltfNodeModifiers.create(subject, {
            modifiers: [{ path: '', material: { material: { $case: 'pbr', pbr: cell.modifier } } }],
          })
        }
        return
      }
      const shape = cell === 'none' ? 'sphere' : (cell.shape ?? 'sphere')
      // A plane faces the camera; a box and a cylinder turn to show a side as well.
      const solid = shape === 'box' || shape === 'cylinder' || shape === 'cone'
      const roll = Quaternion.fromEulerDegrees(0, 0, cell !== 'none' && 'turn' in cell ? (cell.turn ?? 0) : 0)
      place(solid ? Quaternion.multiply(roll, SOLID_TURN) : roll, 2 * RADIUS)
      setMesh(subject, shape)
      if (cell === 'none') return
      if ('pbr' in cell) Material.setPbrMaterial(subject, cell.pbr)
      else Material.setBasicMaterial(subject, cell.unlit)
    })
  })

  const normal = Vector3.rotate(Vector3.create(0, 0, -1), tilt)
  const camera = engine.addEntity()
  Transform.create(camera, { position: Vector3.add(centre, Vector3.scale(normal, CAMERA_DISTANCE)) })
  VirtualCamera.create(camera, {
    lookAtEntity: root,
    defaultTransition: { transitionMode: VirtualCamera.Transition.Time(0) },
  })
  return camera
}

const SOLID_TURN = Quaternion.fromEulerDegrees(-25, 30, 0)
// Two faces of four corners each: the texture's (0,0), (0,1), (1,1), (1,0).
const PLANE_UVS = [0, 0, 0, 1, 1, 1, 1, 0, 0, 0, 0, 1, 1, 1, 1, 0]

function setMesh(entity: Entity, shape: Shape) {
  if (shape === 'box') MeshRenderer.setBox(entity)
  else if (shape === 'plane') MeshRenderer.setPlane(entity)
  else if (shape === 'planeUvs') MeshRenderer.setPlane(entity, PLANE_UVS)
  else if (shape === 'cylinder') MeshRenderer.setCylinder(entity, 0.5, 0.5)
  else if (shape === 'cone') MeshRenderer.setCylinder(entity, 0.5, 0.1)
  else MeshRenderer.setSphere(entity)
}

function label(root: Entity, text: string, y: number, fontSize: number) {
  const entity = engine.addEntity()
  Transform.create(entity, { parent: root, position: Vector3.create(0, y, -0.03) })
  TextShape.create(entity, { text, fontSize, textAlign: TextAlignMode.TAM_MIDDLE_CENTER })
}

/** A black and a white tile behind a sphere, so what shows through it has contrast. */
function tiles(root: Entity, x: number, y: number) {
  const half = RADIUS + 0.1
  for (const [side, shade] of [[-1, 0.02], [1, 0.9]]) {
    const tile = engine.addEntity()
    Transform.create(tile, {
      parent: root,
      position: Vector3.create(x + (side * half) / 2, y, -0.011),
      scale: Vector3.create(half, 2 * half, 0.02),
    })
    MeshRenderer.setBox(tile)
    Material.setBasicMaterial(tile, { diffuseColor: Color4.create(shade, shade, shade, 1), castShadows: false })
  }
}
