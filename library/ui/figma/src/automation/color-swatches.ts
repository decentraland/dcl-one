import type { TokenCollection } from '../ir/schemas.js'
import type { LiveNode } from './live-file.js'

// The Colors page documents each primitive as a "Swatch": a filled rectangle, a label and a hex text.
// The design rule is that the hex text equals the rectangle fill, so a swatch whose text and fill
// disagree is reported instead of read. Gradients have a description instead of a hex and are skipped.
type Swatch = { id: string; column: string; label: string; hex: string; alpha: number }
type SwatchChange =
  | { kind: 'updated'; name: string; from: string; to: string; fromAlpha: number; toAlpha: number }
  | { kind: 'added'; name: string; to: string; toAlpha: number }
  | { kind: 'excluded'; name: string }

const VALUE = /^#([0-9a-f]{6})(?:\s*-\s*(\d{1,3})%)?$/i
const PRIMITIVES = 'Primitives'

type Paint = { type?: string; visible?: boolean; opacity?: number; color?: Record<string, number> }

const channel = (value: number) =>
  Math.round(value * 255)
    .toString(16)
    .padStart(2, '0')
const round = (value: number) => Math.round(value * 100) / 100

const opacity = (node: LiveNode) => (node.opacity as number | undefined) ?? 1

// Figma paints later fills over earlier ones: the visible colour is the topmost paint, and only a
// solid one has a hex. An image or gradient on top makes the swatch unreadable instead of mis-read.
function fillOf(node: LiveNode, swatch: LiveNode): { hex: string; alpha: number } | undefined {
  const paint = ((node.fills as Paint[] | undefined) ?? [])
    .filter((item) => item.visible !== false)
    .at(-1)
  if (paint?.type !== 'SOLID' || !paint.color) return undefined
  const { r = 0, g = 0, b = 0, a = 1 } = paint.color
  return {
    hex: `#${channel(r)}${channel(g)}${channel(b)}`,
    alpha: round(a * (paint.opacity ?? 1) * opacity(node) * opacity(swatch))
  }
}

function texts(node: LiveNode): string[] {
  if (node.type === 'TEXT') return [String(node.characters ?? '').trim()]
  return (node.children ?? []).flatMap(texts)
}

function findSwatches(page: LiveNode, errors: string[]): Swatch[] {
  const swatches: Swatch[] = []
  const visit = (node: LiveNode, column: string) => {
    if (node.visible === false) return
    if (node.name === 'Swatch') {
      const [label, value] = texts(node)
      const match = value?.match(VALUE)
      if (!label || !match) return
      // Transparency swatches stack the colour over a checkerboard: the topmost rectangle is the colour.
      const top = (node.children ?? [])
        .filter((child) => child.visible !== false && child.type === 'RECTANGLE')
        .at(-1)
      const fill = top && fillOf(top, node)
      const hex = `#${match[1]!.toLowerCase()}`
      const alpha = match[2] === undefined ? 1 : round(Number(match[2]) / 100)
      if (!fill || fill.hex !== hex || fill.alpha !== alpha)
        errors.push(
          `Swatch "${label}" (${node.id}) says ${value} but is filled with ${fill ? `${fill.hex} at ${fill.alpha}` : 'no solid colour'}`
        )
      else swatches.push({ id: node.id, column, label, hex, alpha })
      return
    }
    const nested = (node.children ?? []).some((child) => child.name === 'Swatch')
    for (const child of node.children ?? []) visit(child, nested ? node.id : column)
  }
  visit(page, page.id)
  return swatches
}

type Primitive = { name: string; hex: string; alpha: number }

// An excluded primitive keeps its saved variable: its swatch is reported, never read into the capture.
function readSwatches(
  page: LiveNode,
  collections: TokenCollection[],
  excludedPrimitives: readonly string[] = []
) {
  const errors: string[] = []
  const swatches = findSwatches(page, errors)
  const collection = collections.find((item) => item.name === PRIMITIVES)
  if (!collection)
    return { collections, changes: [], notes: [], errors: [`Missing ${PRIMITIVES} collection`] }
  const mode = collection.modes[0]!
  const primitives: Primitive[] = collection.variables.flatMap((variable) => {
    const value = variable.values[mode] as { hex?: string; alpha?: number } | undefined
    return variable.type === 'COLOR' && value?.hex
      ? [{ name: variable.name, hex: value.hex, alpha: round(value.alpha ?? 1) }]
      : []
  })
  const group = (name: string) => name.slice(0, name.lastIndexOf('/'))
  const leaf = (name: string) => name.slice(name.lastIndexOf('/') + 1)
  const sameValue = (swatch: Swatch, primitive: Primitive) =>
    primitive.hex === swatch.hex && primitive.alpha === swatch.alpha

  // A column is one variable group: learn it from the labels that name exactly one primitive.
  const columnGroups = new Map<string, Set<string>>()
  for (const swatch of swatches) {
    const named = primitives.filter((primitive) => leaf(primitive.name) === swatch.label)
    if (named.length !== 1) continue
    const groups = columnGroups.get(swatch.column) ?? new Set<string>()
    groups.add(group(named[0]!.name))
    columnGroups.set(swatch.column, groups)
  }
  const changes: SwatchChange[] = []
  // Nothing is removed or skipped silently: these go to the sync report for review.
  const notes: string[] = []
  const shown = new Set<string>()
  const next = structuredClone(collections)
  const variables = next.find((item) => item.name === PRIMITIVES)!.variables
  for (const swatch of swatches) {
    const groups = columnGroups.get(swatch.column)
    if (!groups || groups.size !== 1) {
      if (!primitives.some((primitive) => sameValue(swatch, primitive)))
        errors.push(
          `Swatch "${swatch.label}" (${swatch.id}) is in a column without one variable group`
        )
      continue
    }
    const name = `${[...groups][0]!}/${swatch.label}`
    const existing = primitives.find((primitive) => primitive.name === name)
    shown.add(name)
    if (excludedPrimitives.includes(`${PRIMITIVES}/${name}`)) continue
    if (existing && sameValue(swatch, existing)) continue
    // Documentation swatches labelled differently (Transparencies) are unchanged when their value matches.
    const twin =
      !existing &&
      primitives.find(
        (primitive) => group(primitive.name) === group(name) && sameValue(swatch, primitive)
      )
    if (twin) {
      shown.add(twin.name)
      notes.push(
        `Swatch "${swatch.label}" (${swatch.id}) shows the value of ${twin.name} and was not added`
      )
      continue
    }
    const value = { hex: swatch.hex, alpha: swatch.alpha }
    const variable = variables.find((item) => item.name === name)
    if (variable) {
      variable.values[mode] = value
      changes.push({
        kind: 'updated',
        name,
        from: existing!.hex,
        to: swatch.hex,
        fromAlpha: existing!.alpha,
        toAlpha: swatch.alpha
      })
    } else {
      variables.push({ name, type: 'COLOR', values: { [mode]: value } })
      changes.push({ kind: 'added', name, to: swatch.hex, toAlpha: swatch.alpha })
    }
  }
  // One diagnostic per excluded primitive, whether its swatch is shown, repeated or missing.
  const excluded = excludedPrimitives
    .filter((entry) => entry.startsWith(`${PRIMITIVES}/`))
    .map((entry) => entry.slice(PRIMITIVES.length + 1))
  for (const name of excluded) changes.push({ kind: 'excluded', name })
  const groupsShown = new Set([...columnGroups.values()].flatMap((groups) => [...groups]))
  for (const primitive of primitives)
    if (
      groupsShown.has(group(primitive.name)) &&
      !shown.has(primitive.name) &&
      !excluded.includes(primitive.name)
    )
      notes.push(`${primitive.name} has no swatch on the Colors page and was kept`)
  return { collections: next, changes, notes, errors }
}

export { readSwatches }
export type { SwatchChange }
