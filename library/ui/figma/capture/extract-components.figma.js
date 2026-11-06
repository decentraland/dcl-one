// Read-only extraction script for the official Figma MCP `use_figma` tool.
//
// Usage: set PAGE_IDS (or EXTERNAL_IDS), then run it. Pages without a ready Sync status are skipped.
// Each run returns at most ~48 KB (the tool limit), so a
// big job comes back in chunks: re-run with START_PAGE/START_COMPONENT set to the returned `resume` until
// `resume` is null. Split and merge the chunks into figma-snapshot/components/<Page>.json with
// `npm run cli -- capture-ingest <chunk.json>...`.
//
// The output is the raw snapshot format read by src/ir/raw-components.ts: per component set one fully
// serialised base variant plus, for every other variant, a list of patches against the base.
const PAGE_IDS = []
// Component ids from outside the captured pages (e.g. a library component instantiated on a page); they are
// captured as one synthetic "External" page.
const EXTERNAL_IDS = []
// Explicit owner-authorized exceptions, scoped to one component set on one disabled page.
// Each entry requires pageId, componentId and a non-empty reason. Invalid/missing controls stay blocked.
const NOT_READY_EXCEPTIONS = []
// For a transport response, repeat the same capture with this offset set to transport.next.
const OUTPUT_START = 0
const START_PAGE = 0
const START_COMPONENT = 0
// Returns one checksum per component instead of the components, to verify a snapshot against the file.
const VERIFY = false
// Complex pages: only the base variant is captured in depth; the others keep their root-level differences.
const OTHERS_ROOT_ONLY = false
// A set too large for one run is captured in parts: only the component at START_COMPONENT, with
// VARIANT_LIMIT of its non-base variants from VARIANT_START. 0 captures every variant. capture-ingest joins
// the parts and refuses a set with missing ones, so pass every part in the same ingest.
const VARIANT_START = 0
const VARIANT_LIMIT = 0
const BUDGET = 38000
const FULL_DEPTH = 12
// Large vector artwork can fall back to PNG. Component structure is always retained.
const ASSET_AT = 150
const SVG_MAX = 12000
const PNG_MAX = 72000

const r2 = (n) => (typeof n === 'number' ? Math.round(n * 100) / 100 : n)
const hex = (c) =>
  '#' +
  [c.r, c.g, c.b]
    .map((v) =>
      Math.round(v * 255)
        .toString(16)
        .padStart(2, '0')
    )
    .join('')
const varCache = new Map()
async function varName(id) {
  if (!varCache.has(id)) {
    const v = await figma.variables.getVariableByIdAsync(id)
    if (!v) varCache.set(id, null)
    else {
      const c = await figma.variables.getVariableCollectionByIdAsync(v.variableCollectionId)
      varCache.set(id, `${c.name}/${v.name}`)
    }
  }
  return varCache.get(id)
}
const styleCache = new Map()
async function styleName(id) {
  if (!id || id === figma.mixed) return null
  if (!styleCache.has(id)) {
    const s = await figma.getStyleByIdAsync(id)
    styleCache.set(id, s ? s.name : null)
  }
  return styleCache.get(id)
}
async function paints(list) {
  if (list === figma.mixed) return 'mixed'
  const out = []
  for (const p of list || []) {
    if (p.visible === false) continue
    const o = { type: p.type }
    if (p.opacity !== undefined && p.opacity !== 1) o.opacity = r2(p.opacity)
    if (p.type === 'SOLID') {
      o.hex = hex(p.color)
      const alias = p.boundVariables && p.boundVariables.color
      if (alias) o.token = await varName(alias.id)
    } else if (p.type.startsWith('GRADIENT')) {
      o.stops = p.gradientStops.map((s) => ({
        position: r2(s.position),
        hex: hex(s.color),
        alpha: r2(s.color.a)
      }))
      o.transform = p.gradientTransform.map((row) => [...row])
    } else if (p.type === 'IMAGE') o.scaleMode = p.scaleMode
    out.push(o)
  }
  return out
}
async function effects(node) {
  const out = []
  for (const e of node.effects || []) {
    if (e.visible === false) continue
    const o = { type: e.type, radius: r2(e.radius) }
    if (e.color) ((o.color = hex(e.color)), (o.alpha = r2(e.color.a)))
    if (e.offset) o.offset = [r2(e.offset.x), r2(e.offset.y)]
    if (e.spread) o.spread = r2(e.spread)
    out.push(o)
  }
  return out
}
async function boundMap(node) {
  const bv = node.boundVariables || {}
  const out = {}
  for (const [key, value] of Object.entries(bv)) {
    if (
      key === 'fills' ||
      key === 'strokes' ||
      key === 'effects' ||
      key === 'textRangeFills' ||
      key === 'fontSize'
    )
      continue
    const alias = Array.isArray(value) ? value[0] : value
    if (alias && alias.id) out[key] = await varName(alias.id)
  }
  return out
}
const VECTORISH = new Set(['VECTOR', 'ELLIPSE', 'STAR', 'POLYGON', 'LINE', 'BOOLEAN_OPERATION'])
// Mirrors BOX_VECTOR_NAME in src/mappers/views.ts: these vectors are boxes, not artwork.
const BOX_VECTOR_NAME = /^(rectangle|bg|container)/i
const descendantCount = (node) => ('findAll' in node ? node.findAll(() => true).length : 0)
// Artwork is exported as an inline picture so the generator can embed it: SVG first, a 2x PNG when the SVG
// is too long to ship inside a snapshot, and `heavy` when neither fits.
// A checksum travels with each picture so a copy mistake between the plugin and the snapshot file is caught.
function checksum(text) {
  let h = 0x811c9dc5
  for (let i = 0; i < text.length; i++) {
    h ^= text.charCodeAt(i)
    h = Math.imul(h, 0x01000193)
  }
  return (h >>> 0).toString(36)
}
async function exportAsset(node, count) {
  const out = await exportPicture(node, count)
  const body = out.svg || out.png
  if (body) out.sum = checksum(body)
  return out
}
async function exportPicture(node, count) {
  const out = {}
  if (count <= ASSET_AT) {
    try {
      const svg = await node.exportAsync({ format: 'SVG_STRING' })
      if (svg.length <= SVG_MAX) {
        out.svg = svg
        return out
      }
    } catch (e) {
      // fall through to PNG
    }
  }
  try {
    const bytes = await node.exportAsync({ format: 'PNG', constraint: { type: 'SCALE', value: 2 } })
    const png = figma.base64Encode(bytes)
    if (png.length <= PNG_MAX) {
      out.png = png
      return out
    }
  } catch (e) {
    // fall through to heavy
  }
  out.heavy = true
  return out
}
function onlyDrawings(node) {
  if (VECTORISH.has(node.type)) return true
  if (node.type === 'GROUP' || node.type === 'FRAME' || node.type === 'INSTANCE')
    return node.children.length > 0 && node.children.every(onlyDrawings)
  return false
}
// Tint remains available for icon aliases and captures that reach the depth limit.
async function tintOf(node) {
  const stack = [node]
  const tint = {}
  while (stack.length) {
    const n = stack.shift()
    if (!tint.fill && 'fills' in n) {
      const f = await paints(n.fills)
      const solid = Array.isArray(f) && f.find((p) => p.type === 'SOLID')
      if (solid) tint.fill = { hex: solid.hex, token: solid.token, opacity: solid.opacity }
    }
    if (!tint.stroke && 'strokes' in n) {
      const s = await paints(n.strokes)
      const solid = Array.isArray(s) && s.find((p) => p.type === 'SOLID')
      if (solid)
        tint.stroke = {
          hex: solid.hex,
          token: solid.token,
          opacity: solid.opacity,
          weight: n.strokeWeight === figma.mixed ? null : r2(n.strokeWeight)
        }
    }
    if ('children' in n) stack.push(...n.children)
  }
  for (const k of ['fill', 'stroke'])
    if (tint[k])
      for (const kk of Object.keys(tint[k]))
        if (tint[k][kk] === undefined || tint[k][kk] === null) delete tint[k][kk]
  return Object.keys(tint).length ? tint : undefined
}
function hasVisibleText(node) {
  if (node.visible === false) return false
  return node.type === 'TEXT' || ('children' in node && node.children.some(hasVisibleText))
}
async function serialize(node, depth, maxDepth) {
  const o = { type: node.type, name: node.name, w: r2(node.width), h: r2(node.height) }
  if ('clipsContent' in node) o.clipsContent = node.clipsContent
  if ('isMask' in node && node.isMask) {
    o.isMask = true
    o.maskType = node.maskType
  }
  if (node.opacity !== undefined && node.opacity !== 1) o.opacity = r2(node.opacity)
  const isContainer = node.type !== 'COMPONENT' && (node.type !== 'INSTANCE' || onlyDrawings(node))
  if (
    'fills' in node &&
    Array.isArray(node.fills) &&
    node.fills.some((paint) => paint.type === 'IMAGE') &&
    !hasVisibleText(node)
  ) {
    o.type = 'DRAWING'
    Object.assign(o, await exportAsset(node, 0))
    return o
  }
  const count = depth > 0 && isContainer ? descendantCount(node) : 0
  if (isContainer && depth > 0 && 'children' in node && onlyDrawings(node)) {
    o.type = 'DRAWING'
    o.tint = await tintOf(node)
    Object.assign(o, await exportAsset(node, count))
    return o
  }
  // A bare vector is artwork unless it is named like a box (see BOX_VECTOR_NAME).
  if (isContainer && depth > 0 && VECTORISH.has(node.type) && !BOX_VECTOR_NAME.test(node.name)) {
    o.tint = await tintOf(node)
    Object.assign(o, await exportAsset(node, 0))
    return o
  }
  if ('layoutMode' in node && node.layoutMode !== 'NONE') {
    o.layout = {
      mode: node.layoutMode,
      pad: [node.paddingTop, node.paddingRight, node.paddingBottom, node.paddingLeft].map(r2),
      gap: r2(node.itemSpacing),
      main: node.primaryAxisAlignItems,
      cross: node.counterAxisAlignItems
    }
    if (node.layoutWrap === 'WRAP') o.layout.wrap = true
  }
  if (
    'layoutSizingHorizontal' in node &&
    node.parent &&
    'layoutMode' in node.parent &&
    node.parent.layoutMode !== 'NONE'
  ) {
    o.sizing = [node.layoutSizingHorizontal, node.layoutSizingVertical]
  }
  if ('cornerRadius' in node) {
    if (node.cornerRadius === figma.mixed)
      o.radius = [
        node.topLeftRadius,
        node.topRightRadius,
        node.bottomRightRadius,
        node.bottomLeftRadius
      ].map(r2)
    else if (node.cornerRadius) o.radius = r2(node.cornerRadius)
  }
  if ('fills' in node && node.type !== 'INSTANCE') {
    const f = await paints(node.fills)
    if (f === 'mixed' || f.length) o.fills = f
    if (!(Array.isArray(f) && f.some((p) => p.token))) {
      const fs = await styleName(node.fillStyleId)
      if (fs) o.fillStyle = fs
    }
  }
  if ('strokes' in node) {
    const s = await paints(node.strokes)
    if (s.length) {
      o.strokes = s
      o.strokeWeight = node.strokeWeight === figma.mixed ? 'mixed' : r2(node.strokeWeight)
      if ('strokeTopWeight' in node) {
        o.strokeWeights = [
          node.strokeTopWeight,
          node.strokeRightWeight,
          node.strokeBottomWeight,
          node.strokeLeftWeight
        ].map(r2)
      }
      o.strokeAlign = node.strokeAlign
      if (node.dashPattern && node.dashPattern.length) o.dash = node.dashPattern
      if (!s.some((p) => p.token)) {
        const ss = await styleName(node.strokeStyleId)
        if (ss) o.strokeStyle = ss
      }
    }
  }
  if ('effects' in node) {
    const e = await effects(node)
    if (e.length) o.effects = e
  }
  const bound = await boundMap(node)
  if (Object.keys(bound).length) o.bound = bound
  if (node.type === 'TEXT') {
    o.text = node.characters
    o.textAlignHorizontal = node.textAlignHorizontal
    o.textAlignVertical = node.textAlignVertical
    if (node.lineHeight !== figma.mixed) o.lineHeight = node.lineHeight
    o.paragraphSpacing = node.paragraphSpacing
    const segments = node.getStyledTextSegments([
      'fontName',
      'fontSize',
      'textStyleId',
      'textCase',
      'letterSpacing',
      'lineHeight',
      'fills'
    ])
    const seg = segments[0]
    if (segments.length > 1) {
      o.textRuns = []
      for (const run of segments)
        o.textRuns.push({
          text: run.characters,
          font: `${run.fontName.family} ${run.fontName.style}`,
          fontSize: run.fontSize,
          fills: await paints(run.fills),
          textCase: run.textCase,
          letterSpacing: run.letterSpacing,
          lineHeight: run.lineHeight
        })
    }
    if ('textTruncation' in node) o.textTruncation = node.textTruncation
    if ('maxLines' in node && node.maxLines !== null) o.maxLines = node.maxLines
    const ts = await styleName(seg.textStyleId)
    if (ts) o.textStyle = ts
    o.font = `${seg.fontName.family} ${seg.fontName.style}`
    o.fontSize = seg.fontSize
    if (seg.letterSpacing) o.letterSpacing = seg.letterSpacing
    o.textCase = node.textCase === figma.mixed ? seg.textCase : node.textCase
  }
  if (node.type === 'INSTANCE') {
    const main = await node.getMainComponentAsync()
    if (main) {
      o.component =
        main.parent && main.parent.type === 'COMPONENT_SET' ? main.parent.name : main.name
      o.componentId = main.id
      if (main.parent && main.parent.type === 'COMPONENT_SET') o.variant = main.name
    }
    if (node.componentProperties) {
      const cp = {}
      for (const [k, v] of Object.entries(node.componentProperties)) cp[k] = v.value
      if (Object.keys(cp).length) o.props = cp
    }
    const own = await paints(node.fills)
    if (Array.isArray(own) && own.length) o.fills = own
    o.tint = await tintOf(node)
  }
  if ('children' in node && depth < maxDepth) {
    o.children = []
    const hiddenNames = []
    const parentBox = node.absoluteBoundingBox
    const positioned = !('layoutMode' in node) || node.layoutMode === 'NONE'
    for (const child of node.children) {
      if (child.visible === false) {
        hiddenNames.push(child.name)
        continue
      }
      const built = await serialize(child, depth + 1, maxDepth)
      // Offsets come from the bounding boxes because x/y of a group's children are relative to the group's
      // parent, not to the group.
      const box = child.absoluteBoundingBox
      if (parentBox && box && (positioned || child.layoutPositioning === 'ABSOLUTE')) {
        built.x = r2(box.x - parentBox.x)
        built.y = r2(box.y - parentBox.y)
        if (!positioned) built.absolute = true
      }
      o.children.push(built)
    }
    if (hiddenNames.length) o.hiddenChildren = hiddenNames
  }
  return o
}
function parseVariantName(name) {
  const props = {}
  for (const part of name.split(',')) {
    const i = part.indexOf('=')
    if (i > 0) props[part.slice(0, i).trim()] = part.slice(i + 1).trim()
  }
  return props
}
// Patch paths index the *visible* children, so base and variant stay aligned after hidden layers are dropped.
function diffTree(a, b, at, out) {
  const isRoot = at.length === 0
  const same =
    a.type === b.type &&
    (isRoot || a.name === b.name) &&
    (a.children ? a.children.length : 0) === (b.children ? b.children.length : 0)
  if (!same) {
    out.push({ at, replace: b })
    return
  }
  const set = {}
  const unset = []
  for (const k of new Set([...Object.keys(a), ...Object.keys(b)])) {
    if (k === 'children' || (isRoot && k === 'name')) continue
    if (JSON.stringify(a[k]) !== JSON.stringify(b[k])) {
      if (b[k] === undefined) unset.push(k)
      else set[k] = b[k]
    }
  }
  if (Object.keys(set).length || unset.length) {
    const patch = { at }
    if (Object.keys(set).length) patch.set = set
    if (unset.length) patch.unset = unset
    out.push(patch)
  }
  if (a.children) a.children.forEach((child, i) => diffTree(child, b.children[i], [...at, i], out))
}
// Preserve component structure even when it contains many descendants.
async function serializeVariant(node, maxDepth) {
  return serialize(node, 0, maxDepth)
}
const ROOT_KEYS = [
  'w',
  'h',
  'opacity',
  'layout',
  'sizing',
  'radius',
  'fills',
  'strokes',
  'strokeWeight',
  'strokeWeights',
  'strokeAlign',
  'effects'
]
function rootPatch(base, other) {
  const set = {}
  const unset = []
  for (const k of ROOT_KEYS) {
    if (JSON.stringify(base[k]) === JSON.stringify(other[k])) continue
    if (other[k] === undefined) unset.push(k)
    else set[k] = other[k]
  }
  const patch = { at: [] }
  if (Object.keys(set).length) patch.set = set
  if (unset.length) patch.unset = unset
  return Object.keys(set).length || unset.length ? [patch] : []
}
async function build(node, maxDepth, external) {
  const entry = {
    kind: node.type === 'COMPONENT_SET' ? 'set' : 'component',
    id: node.id,
    name: node.name
  }
  if (external) entry.external = true
  if (node.description) entry.description = node.description
  const frames = []
  for (let p = node.parent; p && p.type !== 'PAGE'; p = p.parent) frames.push(p.name)
  entry.section = frames.reverse()
  entry.x = r2(node.absoluteBoundingBox ? node.absoluteBoundingBox.x : node.x)
  entry.y = r2(node.absoluteBoundingBox ? node.absoluteBoundingBox.y : node.y)
  entry.properties = {}
  for (const [k, d] of Object.entries(node.componentPropertyDefinitions)) {
    entry.properties[k] = { type: d.type, default: d.defaultValue }
    if (d.variantOptions) entry.properties[k].options = d.variantOptions
  }
  if (node.type === 'COMPONENT_SET') {
    const variants = node.children.filter((v) => v.type === 'COMPONENT')
    const others =
      VARIANT_LIMIT > 0
        ? variants.slice(1).slice(VARIANT_START, VARIANT_START + VARIANT_LIMIT)
        : variants.slice(1)
    const trees = []
    for (const [index, v] of [variants[0], ...others].entries()) {
      trees.push(await serializeVariant(v, OTHERS_ROOT_ONLY && index > 0 ? 0 : maxDepth))
    }
    delete trees[0].name
    entry.base = {
      id: variants[0].id,
      name: variants[0].name,
      props: parseVariantName(variants[0].name),
      tree: trees[0]
    }
    entry.others = others.map((v, i) => {
      const patches = []
      if (OTHERS_ROOT_ONLY) patches.push(...rootPatch(trees[0], trees[i + 1]))
      else diffTree({ ...trees[0], name: v.name }, trees[i + 1], [], patches)
      return { id: v.id, name: v.name, props: parseVariantName(v.name), patches }
    })
  } else {
    const tree = await serializeVariant(node, maxDepth)
    delete tree.name
    entry.base = { id: node.id, name: node.name, props: {}, tree }
    entry.others = []
  }
  return entry
}

const nested = (n) => n.type === 'COMPONENT' && n.parent && n.parent.type === 'COMPONENT_SET'
async function syncStateOf(page) {
  const markers = page.children.filter((node) => node.name === 'Sync status')
  if (markers.length !== 1 || markers[0].type !== 'INSTANCE') return 'missing-sync-status'
  const marker = markers[0]
  const main = await marker.getMainComponentAsync()
  if (main?.parent?.id !== '15973:15') return 'invalid-sync-status'
  const ready = marker.componentProperties.ready
  if (ready?.type !== 'VARIANT' || !['true', 'false'].includes(ready.value))
    return 'invalid-sync-state'
  return ready.value === 'true' ? null : 'not-ready'
}

async function componentsOf(pageId) {
  if (pageId === 'external') {
    const list = []
    const skipped = []
    for (const id of EXTERNAL_IDS) {
      const node = await figma.getNodeByIdAsync(id)
      if (!node) {
        skipped.push({ id, reason: 'missing-component' })
        continue
      }
      let page = node
      while (page.parent && page.type !== 'PAGE') page = page.parent
      const reason = page.type === 'PAGE' ? await syncStateOf(page) : 'missing-page'
      const target = node.parent && node.parent.type === 'COMPONENT_SET' ? node.parent : node
      const exception = NOT_READY_EXCEPTIONS.some(
        (item) =>
          item.pageId === page.id &&
          item.componentId === target.id &&
          typeof item.reason === 'string' &&
          item.reason.trim()
      )
      if (reason && !(reason === 'not-ready' && exception)) {
        skipped.push({ id, page: page.name, reason })
        continue
      }
      list.push(node.parent && node.parent.type === 'COMPONENT_SET' ? node.parent : node)
    }
    return {
      page: { id: 'external', name: 'External' },
      found: [...new Set(list)],
      external: true,
      skipped
    }
  }
  const page = await figma.getNodeByIdAsync(pageId)
  if (!page || page.type !== 'PAGE')
    return {
      page: { id: pageId, name: pageId },
      found: [],
      skipped: [{ id: pageId, reason: 'missing-page' }]
    }
  await figma.setCurrentPageAsync(page)
  const reason = await syncStateOf(page)
  const exceptions = NOT_READY_EXCEPTIONS.filter(
    (item) => item.pageId === page.id && typeof item.reason === 'string' && item.reason.trim()
  )
  if (reason && !(reason === 'not-ready' && exceptions.length > 0))
    return { page, found: [], skipped: [{ id: page.id, page: page.name.trim(), reason }] }
  let found = page
    .findAllWithCriteria({ types: ['COMPONENT_SET', 'COMPONENT'] })
    .filter((n) => !nested(n))
  let external = false
  if (reason === 'not-ready') {
    const selected = found.filter((node) => exceptions.some((item) => item.componentId === node.id))
    return { page, found: selected, external, labels: false }
  }
  if (found.length === 0) {
    // A page that only instantiates a library component: capture the component behind the instances.
    const mains = []
    for (const child of page.children) {
      if (child.type !== 'INSTANCE' || child.name === 'Sync status') continue
      const main = await child.getMainComponentAsync()
      if (main) mains.push(main.parent && main.parent.type === 'COMPONENT_SET' ? main.parent : main)
    }
    found = [...new Set(mains)]
    external = true
  }
  return { page, found, external, labels: START_COMPONENT === 0 }
}

const jobs = EXTERNAL_IDS.length > 0 ? ['external'] : PAGE_IDS
const pages = []
const hashes = {}
const skipped = []
let size = 0
let resume = null
outer: for (let pageIndex = START_PAGE; pageIndex < jobs.length; pageIndex++) {
  const {
    page,
    found,
    external,
    labels,
    skipped: rejected = []
  } = await componentsOf(jobs[pageIndex])
  skipped.push(...rejected)
  if (rejected.length > 0 && found.length === 0) continue
  const start = pageIndex === START_PAGE ? START_COMPONENT : 0
  const components = []
  let next = null
  for (let index = start; index < found.length; index++) {
    const entry = await build(found[index], FULL_DEPTH, external)
    if (VERIFY) {
      hashes[entry.id] = checksum(JSON.stringify(entry))
      continue
    }
    // Every variant part retains the full-depth base tree; ingest checks contiguous coverage.
    if (VARIANT_LIMIT > 0) {
      const total =
        found[index].type === 'COMPONENT_SET'
          ? found[index].children.filter((node) => node.type === 'COMPONENT').length - 1
          : 0
      entry.variantRange = { start: VARIANT_START, total }
      components.push(entry)
      const nextVariant = VARIANT_START + VARIANT_LIMIT
      next = nextVariant < total ? index : index + 1 < found.length ? index + 1 : null
      if (next === index) resume = { page: pageIndex, component: index, variant: nextVariant }
      break
    }
    const length = JSON.stringify(entry).length
    if ((components.length > 0 || pages.length > 0) && size + length > BUDGET) {
      next = index
      break
    }
    size += length
    components.push(entry)
  }
  const out = {
    page: { id: page.id, name: page.name.trim() },
    total: found.length,
    start,
    components
  }
  if (start === 0 && labels && !external && page.findAllWithCriteria) {
    out.labels = page
      .findAllWithCriteria({ types: ['TEXT'] })
      .filter((t) => {
        for (let p = t.parent; p; p = p.parent)
          if (p.type === 'COMPONENT' || p.type === 'COMPONENT_SET' || p.type === 'INSTANCE')
            return false
        return true
      })
      .slice(0, 40)
      .map((t) => ({
        text: t.characters,
        x: r2(t.absoluteBoundingBox ? t.absoluteBoundingBox.x : t.x),
        y: r2(t.absoluteBoundingBox ? t.absoluteBoundingBox.y : t.y)
      }))
  }
  if (components.length > 0 || start === 0) pages.push(out)
  if (next !== null) {
    if (!resume) resume = { page: pageIndex, component: next, variant: 0 }
    break outer
  }
  if (VARIANT_LIMIT > 0) {
    // One part per run. A finished page hands over to the next page of the job, from its first component and
    // first variant; only the last page ends the job with resume null.
    if (pageIndex + 1 < jobs.length) resume = { page: pageIndex + 1, component: 0, variant: 0 }
    break
  }
  if (size > BUDGET && pageIndex + 1 < jobs.length) {
    resume = { page: pageIndex + 1, component: 0, variant: 0 }
    break outer
  }
}
const result = VERIFY ? { hashes, skipped } : { pages, resume, skipped }
const raw = JSON.stringify(result)
if (raw.length > 22000) {
  const next = OUTPUT_START + 22000
  return {
    transport: {
      offset: OUTPUT_START,
      total: raw.length,
      checksum: checksum(raw),
      part: raw.slice(OUTPUT_START, next),
      next: next < raw.length ? next : null
    }
  }
}
return result
