import {
  Billboard,
  BillboardMode,
  Entity,
  Font,
  InputAction,
  Material,
  MeshCollider,
  MeshRenderer,
  TextAlignMode,
  TextShape,
  Transform,
  engine,
  pointerEventsSystem,
} from '@dcl/sdk/ecs'
import { Color3, Color4, Quaternion, Vector3 } from '@dcl/sdk/math'
import { setupVisualTest } from './visual-test-setup'

/**
 * text-shape — exhaustive TextShape property matrix.
 *
 * Visual contract: each row varies ONE TextShape axis (or one tightly-related
 * cluster) while holding the rest constant. Any regression in font rasterizing,
 * color/alpha, drop-shadow compositing, outline stroke, layout (alignment,
 * padding, wrapping, line spacing), auto-size fit, billboard orientation, or
 * depth occlusion shows up as drift in the corresponding row.
 *
 * Layout is a single 0,0 parcel with subjects stacked in Y, all packed inside
 * the camera FOV (camera at (8, 7, -2) looking at (8, 3, 8)). z is held at 8
 * for nearly every entity so X/Y screen position maps cleanly back to row/col.
 *
 * Y stack (bottom → top), labels are the axis under test:
 *   y≈0.6  title
 *   y≈1.5  fontSize         · 5 cells, increasing pt
 *   y≈2.4  textColor        · 5 cells incl. alpha
 *   y≈3.3  font             · F_SANS_SERIF / F_SERIF / F_MONOSPACE
 *   y≈4.1..5.1  textAlign   · 3×3 TAM_ matrix on visible bounding rectangles
 *   y≈5.8  shadow           · shadowBlur × shadowOffset × shadowColor
 *   y≈6.5  outline          · outlineWidth × outlineColor
 *   y≈7.2  padding & line   · paddingLeft/Right vs lineSpacing on multi-line
 *   y≈7.9  wrapping & auto  · textWrapping + lineCount + fontAutoSize
 *   y≈8.6  billboard / occlusion · BM_ALL must face camera; plane must occlude
 *
 * Every entity has a hover hint describing the case it covers; max distance is
 * bumped well past the SDK default so the locked virtual camera at z=−2 still
 * resolves hovers on every text entity.
 *
 * Default text orientation in DCL faces the −Z viewer direction, so most
 * entities here use no rotation; the billboard row deliberately starts at
 * 180°Y so any regression in BM_ALL leaves the text mirrored/backwards.
 */
export function main() {
  // ── ground ───────────────────────────────────────────────────────────────
  const ground = engine.addEntity()
  Transform.create(ground, {
    position: Vector3.create(8, 0, 8),
    scale: Vector3.create(16, 0.05, 16),
  })
  MeshRenderer.setBox(ground)
  Material.setPbrMaterial(ground, {
    albedoColor: Color4.create(0.4, 0.4, 0.42, 1),
    roughness: 0.9,
    metallic: 0,
  })

  const zRow = 8

  // Default textColor for cells that are NOT testing color or border. White
  // is the SDK default, but white text washes out against both the bright
  // sky and the gray ground; a dark slate gives the most consistent contrast
  // across the whole stack.
  const TEXT_DARK = Color4.create(0.1, 0.1, 0.14, 1)

  // ── title ────────────────────────────────────────────────────────────────
  makeText({
    pos: Vector3.create(8, 0.6, zRow),
    text: 'TextShape · property matrix',
    fontSize: 2,
    textColor: TEXT_DARK,
    hint: 'Title · default font/align, fontSize=2, dark',
  })

  // ── fontSize row (y=1.5) ─────────────────────────────────────────────────
  // Five "Aa" cells with increasing fontSize. fontSize is independent of
  // width/height (autoSize off), so any change in glyph metrics or DPR maps
  // straight onto the relative size ratios across this row. Range capped so
  // even the largest cell fits inside its column without bleeding into the
  // colors row above (every fontSize unit ≈ 5 cm of glyph height in world).
  const sizeRow = 1.5
  const sizes: { x: number; size: number }[] = [
    { x: 2, size: 1 },
    { x: 5, size: 2 },
    { x: 8, size: 3 },
    { x: 11, size: 4 },
    { x: 14, size: 6 },
  ]
  for (const { x, size } of sizes) {
    makeText({
      pos: Vector3.create(x, sizeRow, zRow),
      text: 'Aa',
      fontSize: size,
      textColor: TEXT_DARK,
      hint: `fontSize=${size}`,
    })
  }

  // ── textColor row (y=2.4) ────────────────────────────────────────────────
  // RGB primaries + a yellow + a 50%-alpha white over the dark ground prove
  // the color and alpha channels are wired through correctly.
  const colorRow = 2.4
  const colors: { x: number; color: Color4; label: string; hint: string }[] = [
    { x: 2, color: Color4.create(1, 0.2, 0.2, 1), label: 'RED', hint: 'textColor=red' },
    { x: 5, color: Color4.create(0.2, 1, 0.3, 1), label: 'GREEN', hint: 'textColor=green' },
    { x: 8, color: Color4.create(0.25, 0.55, 1, 1), label: 'BLUE', hint: 'textColor=blue' },
    { x: 11, color: Color4.create(1, 0.95, 0.2, 1), label: 'YELLOW', hint: 'textColor=yellow' },
    { x: 14, color: Color4.create(1, 1, 1, 0.4), label: 'ALPHA', hint: 'textColor white α=0.4' },
  ]
  for (const c of colors) {
    makeText({
      pos: Vector3.create(c.x, colorRow, zRow),
      text: c.label,
      fontSize: 2,
      textColor: c.color,
      hint: c.hint,
    })
  }

  // ── font row (y=3.3) ─────────────────────────────────────────────────────
  // Same string in each of the three font families. Shape differences
  // between sans/serif/mono should be obvious in the diff.
  const fontRow = 3.1
  const fonts: { x: number; font: Font; label: string; name: string }[] = [
    { x: 4, font: Font.F_SANS_SERIF, label: 'Quick brown · SANS', name: 'F_SANS_SERIF' },
    { x: 8, font: Font.F_SERIF, label: 'Quick brown · SERIF', name: 'F_SERIF' },
    { x: 12, font: Font.F_MONOSPACE, label: 'Quick brown · MONO', name: 'F_MONOSPACE' },
  ]
  for (const f of fonts) {
    makeText({
      pos: Vector3.create(f.x, fontRow, zRow),
      text: f.label,
      font: f.font,
      fontSize: 1.5,
      textColor: TEXT_DARK,
      hint: `font=${f.name}`,
    })
  }

  // ── textAlign 3×3 grid (y=4.1..5.1) ──────────────────────────────────────
  // Each cell shows ONE alignment mode inside a visible bounding plane. The
  // plane is colored so the alignment of the (white) text within the box is
  // unambiguous — TAM_TOP_LEFT puts the glyphs against the top-left corner,
  // TAM_BOTTOM_RIGHT against the bottom-right, etc.
  const alignCellSize = 0.8
  const alignGridY = 4.6
  const alignGridX = 8
  const aligns: { row: number; col: number; mode: TextAlignMode; label: string }[] = [
    { row: 0, col: 0, mode: TextAlignMode.TAM_TOP_LEFT, label: 'TL' },
    { row: 0, col: 1, mode: TextAlignMode.TAM_TOP_CENTER, label: 'TC' },
    { row: 0, col: 2, mode: TextAlignMode.TAM_TOP_RIGHT, label: 'TR' },
    { row: 1, col: 0, mode: TextAlignMode.TAM_MIDDLE_LEFT, label: 'ML' },
    { row: 1, col: 1, mode: TextAlignMode.TAM_MIDDLE_CENTER, label: 'MC' },
    { row: 1, col: 2, mode: TextAlignMode.TAM_MIDDLE_RIGHT, label: 'MR' },
    { row: 2, col: 0, mode: TextAlignMode.TAM_BOTTOM_LEFT, label: 'BL' },
    { row: 2, col: 1, mode: TextAlignMode.TAM_BOTTOM_CENTER, label: 'BC' },
    { row: 2, col: 2, mode: TextAlignMode.TAM_BOTTOM_RIGHT, label: 'BR' },
  ]
  for (const a of aligns) {
    const cx = alignGridX + (a.col - 1) * (alignCellSize + 0.15)
    // row 0 = TOP visually, so subtract from grid center Y
    const cy = alignGridY - (a.row - 1) * (alignCellSize + 0.15)

    // Visible bounding rectangle so the alignment within it is readable.
    const frame = engine.addEntity()
    Transform.create(frame, {
      position: Vector3.create(cx, cy, zRow + 0.01),
      scale: Vector3.create(alignCellSize, alignCellSize, 1),
    })
    MeshRenderer.setPlane(frame)
    MeshCollider.setPlane(frame)
    Material.setPbrMaterial(frame, {
      albedoColor: Color4.create(0.15, 0.15, 0.2, 1),
      roughness: 0.9,
      metallic: 0,
    })
    attachHint(frame, `Align cell · backdrop for TAM_${a.label}`)

    // Text fills the same footprint as the backdrop. width/height match the
    // backdrop's scale so the alignment honours the entire cell.
    makeText({
      pos: Vector3.create(cx, cy, zRow),
      text: a.label,
      fontSize: 1.5,
      width: alignCellSize,
      height: alignCellSize,
      textAlign: a.mode,
      hint: `textAlign=TAM_${a.label}`,
    })
  }

  // ── drop shadow row (y=5.8) ──────────────────────────────────────────────
  // shadowBlur / shadowOffsetX / shadowOffsetY / shadowColor. Three
  // progressively heavier shadows; any regression in shadow compositing,
  // blur kernel, or color tint shows up here.
  const shadowRow = 6.2
  makeText({
    pos: Vector3.create(4, shadowRow, zRow),
    text: 'shadow soft',
    fontSize: 1.5,
    shadowBlur: 1,
    shadowOffsetX: 1,
    shadowOffsetY: -1,
    shadowColor: Color3.create(0, 0, 0),
    hint: 'shadow · blur=1, offset=(1,-1), black',
  })
  makeText({
    pos: Vector3.create(8, shadowRow, zRow),
    text: 'shadow hard',
    fontSize: 1.5,
    shadowBlur: 0,
    shadowOffsetX: 2,
    shadowOffsetY: -2,
    shadowColor: Color3.create(0.9, 0.1, 0.1),
    hint: 'shadow · blur=0, offset=(2,-2), red',
  })
  makeText({
    pos: Vector3.create(12, shadowRow, zRow),
    text: 'shadow blur',
    fontSize: 1.5,
    shadowBlur: 5,
    shadowOffsetX: 0,
    shadowOffsetY: 0,
    shadowColor: Color3.create(0.2, 0.6, 1),
    hint: 'shadow · blur=5, offset=(0,0), cyan glow',
  })

  // ── outline row (y=6.5) ──────────────────────────────────────────────────
  // outlineWidth in {0.1, 0.3, 0.5} with distinct outlineColor each. Outline
  // is a per-glyph stroke independent of the shadow channel; if a regression
  // collapses width to 0 or swaps color → fill, the row diverges visibly.
  const outlineRow = 6.5
  makeText({
    pos: Vector3.create(4, outlineRow, zRow),
    text: 'OUTLINE thin',
    fontSize: 1.5,
    outlineWidth: 0.1,
    outlineColor: Color3.create(0, 0, 0),
    textColor: Color4.create(1, 1, 1, 1),
    hint: 'outline · width=0.1, black',
  })
  makeText({
    pos: Vector3.create(8, outlineRow, zRow),
    text: 'OUTLINE wide',
    fontSize: 1.5,
    outlineWidth: 0.3,
    outlineColor: Color3.create(0.9, 0.1, 0.6),
    textColor: Color4.create(1, 1, 1, 1),
    hint: 'outline · width=0.3, magenta',
  })
  makeText({
    pos: Vector3.create(12, outlineRow, zRow),
    text: 'OUTLINE max',
    fontSize: 1.5,
    outlineWidth: 0.5,
    outlineColor: Color3.create(0.1, 0.9, 0.3),
    textColor: Color4.create(0, 0, 0, 1),
    hint: 'outline · width=0.5, green over black fill',
  })

  // ── padding + line spacing row (y=7.2) ───────────────────────────────────
  // (a) padding: same multi-line text in two cells of equal width — the
  //     left has paddingLeft=0.3 and the right has none. The visible bounding
  //     plane lets you see the gap shrink the usable text area.
  // (b) lineSpacing: same 3-line text with lineSpacing=0 vs lineSpacing=20
  //     proves the engine is honouring the inter-line gap.
  const padRow = 7.2

  // Padding cell — backdrop + text with paddingLeft/Right
  makePaddedCell({
    centerX: 3,
    centerY: padRow,
    z: zRow,
    width: 1.8,
    height: 0.6,
    text: 'PAD-LEFT',
    fontSize: 1.2,
    paddingLeft: 0.4,
    align: TextAlignMode.TAM_MIDDLE_LEFT,
    hint: 'padding · paddingLeft=0.4 over 1.8-wide cell',
  })

  // No-padding reference next to it
  makePaddedCell({
    centerX: 5.5,
    centerY: padRow,
    z: zRow,
    width: 1.8,
    height: 0.6,
    text: 'NO-PAD',
    fontSize: 1.2,
    paddingLeft: 0,
    align: TextAlignMode.TAM_MIDDLE_LEFT,
    hint: 'padding · paddingLeft=0 (reference)',
  })

  makeText({
    pos: Vector3.create(10.5, padRow, zRow),
    text: 'tight\nline\nstack',
    fontSize: 1,
    lineSpacing: 0,
    textAlign: TextAlignMode.TAM_MIDDLE_CENTER,
    width: 2,
    height: 1.0,
    textColor: TEXT_DARK,
    hint: 'lineSpacing=0 · 3 lines pressed together',
  })
  makeText({
    pos: Vector3.create(13.5, padRow, zRow),
    text: 'wide\nline\ngap',
    fontSize: 1,
    lineSpacing: 30,
    textAlign: TextAlignMode.TAM_MIDDLE_CENTER,
    width: 2,
    height: 1.2,
    textColor: TEXT_DARK,
    hint: 'lineSpacing=30 · 3 lines pulled apart',
  })

  // ── wrapping + lineCount + autosize row (y=7.9) ──────────────────────────
  const wrapRow = 7.9

  // Long sentence, narrow width, wrapping ON → multi-line layout.
  makePaddedCell({
    centerX: 3,
    centerY: wrapRow,
    z: zRow,
    width: 2.0,
    height: 0.6,
    text: 'wrap when text overflows the box',
    fontSize: 0.9,
    textWrapping: true,
    align: TextAlignMode.TAM_MIDDLE_LEFT,
    hint: 'textWrapping=true · long text wraps inside narrow box',
  })

  // Same long text, wrapping OFF + lineCount=2 → first 2 lines only, rest
  // gets clipped or the text exceeds the cell horizontally. Distinguishes
  // wrapping logic from line clamping.
  makePaddedCell({
    centerX: 5.5,
    centerY: wrapRow,
    z: zRow,
    width: 2.0,
    height: 0.6,
    text: 'first\nsecond\nthird\nfourth',
    fontSize: 0.9,
    lineCount: 2,
    align: TextAlignMode.TAM_MIDDLE_LEFT,
    hint: 'lineCount=2 · only first two of four explicit lines render',
  })

  // fontAutoSize comparison: same text/cell, autoSize ON vs OFF. The DCL
  // implementation does NOT clamp glyph height to the box — instead it
  // scales relative to fontSize as an upper bound, so we use a small
  // fontSize=1 baseline and a short label to keep the auto-sized text from
  // bleeding into the rows above. If autoSize regresses, the two cells
  // become indistinguishable.
  makePaddedCell({
    centerX: 9.5,
    centerY: wrapRow,
    z: zRow,
    width: 1.6,
    height: 0.5,
    text: 'AUTO',
    fontSize: 1,
    fontAutoSize: true,
    align: TextAlignMode.TAM_MIDDLE_CENTER,
    hint: 'fontAutoSize=true · 1.6×0.5 cell · text scales to fit',
  })
  makePaddedCell({
    centerX: 12.5,
    centerY: wrapRow,
    z: zRow,
    width: 1.6,
    height: 0.5,
    text: 'AUTO',
    fontSize: 1,
    fontAutoSize: false,
    align: TextAlignMode.TAM_MIDDLE_CENTER,
    hint: 'fontAutoSize=false reference · same cell, fontSize honoured',
  })

  // ── billboard + occlusion row (y=8.6) ────────────────────────────────────
  // Billboard text: created at 180°Y so its natural facing is AWAY from the
  // camera. BM_ALL must override that rotation every frame. A regression
  // where the billboard system stops applying leaves the text mirrored or
  // edge-on.
  const topRow = 8.6
  const billboardText = engine.addEntity()
  Transform.create(billboardText, {
    position: Vector3.create(5, topRow, zRow),
    rotation: Quaternion.fromEulerDegrees(0, 180, 0),
  })
  TextShape.create(billboardText, {
    text: 'BILLBOARD',
    fontSize: 2,
    textColor: Color4.create(0.95, 0.4, 0.85, 1),
    outlineWidth: 0.1,
    outlineColor: Color3.create(0, 0, 0),
  })
  Billboard.create(billboardText, { billboardMode: BillboardMode.BM_ALL })
  // Tiny collider to give the hover hint somewhere to land.
  MeshCollider.setPlane(billboardText)
  attachHint(
    billboardText,
    'Billboard · initial rotation 180°Y (faces away) · BM_ALL must orient to camera',
  )

  // Occlusion: a text entity at z=8 with an opaque plane in front (closer to
  // the camera at smaller z). The plane MUST occlude the text. If depth-test
  // regresses for text rendering, the text bleeds through the plane.
  const occludedText = engine.addEntity()
  Transform.create(occludedText, { position: Vector3.create(11, topRow, zRow) })
  TextShape.create(occludedText, {
    text: 'OCCLUDED',
    fontSize: 6,
    textColor: Color4.create(1, 0.3, 0.3, 1),
  })
  MeshCollider.setPlane(occludedText)
  attachHint(occludedText, 'Occlusion subject · sits behind the gray plane')

  // The occluding plane: same X/Y as the text but closer to the camera.
  // Sized generously larger than the OCCLUDED text footprint (≈1.6×0.4 at
  // fontSize=2) so the glyphs are fully covered with margin to spare — a
  // partially-occluding plane would let edges peek through and read as
  // "depth test broken" in the diff regardless of actual behavior.
  const occluder = engine.addEntity()
  Transform.create(occluder, {
    position: Vector3.create(11, topRow, zRow - 1.5),
    scale: Vector3.create(3.5, 1.2, 1),
  })
  MeshRenderer.setPlane(occluder)
  MeshCollider.setPlane(occluder)
  Material.setPbrMaterial(occluder, {
    albedoColor: Color4.create(0.7, 0.7, 0.75, 1),
    roughness: 0.6,
    metallic: 0,
  })
  attachHint(occluder, 'Occluder plane · must hide the OCCLUDED text behind it')

  // ── camera ───────────────────────────────────────────────────────────────
  // Pulled back to z=−2 and up to y=4.5 so the y=0.6..8.6 stack frames cleanly.
  setupVisualTest({
    lookAtPos: Vector3.create(8, 5, 8),
    cameraPos: Vector3.create(8, 5, 0),
  })
}

// Hover hint: show `text` while the cursor is over the entity. Empty click
// handler — only the hover tooltip matters here. maxDistance bumped past the
// SDK default so the locked virtual camera at z=−2 still resolves hovers on
// every text entity in the back of the parcel.
const HINT_MAX_DISTANCE = 100

// Flip to true to wire up hover tooltips while debugging the scene by hand.
// Kept off for visual-test runs so the cursor never paints a hover outline.
const DEBUG_HOVER_HINTS = false

function attachHint(entity: Entity, text: string) {
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

interface MakeTextOpts {
  pos: Vector3
  text: string
  fontSize?: number
  font?: Font
  textColor?: Color4
  textAlign?: TextAlignMode
  width?: number
  height?: number
  paddingTop?: number
  paddingRight?: number
  paddingBottom?: number
  paddingLeft?: number
  lineSpacing?: number
  lineCount?: number
  textWrapping?: boolean
  fontAutoSize?: boolean
  shadowBlur?: number
  shadowOffsetX?: number
  shadowOffsetY?: number
  shadowColor?: Color3
  outlineWidth?: number
  outlineColor?: Color3
  hint: string
}

// Shared text-entity factory: build an entity with a Transform + TextShape +
// hover hint, leaving the only-set fields on PBTextShape so the engine's own
// defaults apply elsewhere. A small invisible plane collider goes on every
// entity so the hover hint resolves even on tiny text.
function makeText(opts: MakeTextOpts): Entity {
  const e = engine.addEntity()
  Transform.create(e, { position: opts.pos })
  TextShape.create(e, {
    text: opts.text,
    fontSize: opts.fontSize,
    font: opts.font,
    textColor: opts.textColor,
    textAlign: opts.textAlign,
    width: opts.width,
    height: opts.height,
    paddingTop: opts.paddingTop,
    paddingRight: opts.paddingRight,
    paddingBottom: opts.paddingBottom,
    paddingLeft: opts.paddingLeft,
    lineSpacing: opts.lineSpacing,
    lineCount: opts.lineCount,
    textWrapping: opts.textWrapping,
    fontAutoSize: opts.fontAutoSize,
    shadowBlur: opts.shadowBlur,
    shadowOffsetX: opts.shadowOffsetX,
    shadowOffsetY: opts.shadowOffsetY,
    shadowColor: opts.shadowColor,
    outlineWidth: opts.outlineWidth,
    outlineColor: opts.outlineColor,
  })
  MeshCollider.setPlane(e)
  attachHint(e, opts.hint)
  return e
}

interface PaddedCellOpts {
  centerX: number
  centerY: number
  z: number
  width: number
  height: number
  text: string
  fontSize?: number
  paddingLeft?: number
  paddingRight?: number
  paddingTop?: number
  paddingBottom?: number
  lineCount?: number
  textWrapping?: boolean
  fontAutoSize?: boolean
  align: TextAlignMode
  hint: string
}

// makePaddedCell: a colored backdrop plane + a text entity with the same
// width/height. The backdrop makes padding, wrapping, lineCount and autosize
// effects readable — without a visible bounding box it's hard to tell whether
// the text is honouring the cell.
function makePaddedCell(opts: PaddedCellOpts): void {
  const backdrop = engine.addEntity()
  Transform.create(backdrop, {
    position: Vector3.create(opts.centerX, opts.centerY, opts.z + 0.01),
    scale: Vector3.create(opts.width, opts.height, 1),
  })
  MeshRenderer.setPlane(backdrop)
  MeshCollider.setPlane(backdrop)
  Material.setPbrMaterial(backdrop, {
    albedoColor: Color4.create(0.15, 0.15, 0.2, 1),
    roughness: 0.9,
    metallic: 0,
  })
  attachHint(backdrop, `Cell · ${opts.hint}`)

  makeText({
    pos: Vector3.create(opts.centerX, opts.centerY, opts.z),
    text: opts.text,
    fontSize: opts.fontSize ?? 2,
    width: opts.width,
    height: opts.height,
    paddingLeft: opts.paddingLeft,
    paddingRight: opts.paddingRight,
    paddingTop: opts.paddingTop,
    paddingBottom: opts.paddingBottom,
    lineCount: opts.lineCount,
    textWrapping: opts.textWrapping,
    fontAutoSize: opts.fontAutoSize,
    textAlign: opts.align,
    hint: opts.hint,
  })
}
