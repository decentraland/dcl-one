import { assertIdentifier } from '../generators/naming.js'
import { expandVariants } from '../capture/expand-variants.js'
import type { FigmaConfig } from '../config.js'
import { excludePrimitives, TokenIndex } from '../generators/tokens.js'
import { splitWords } from './variants.js'
import type { ComponentSpec, SlotSpec, TextRole, ViewNode, ViewSpec } from '../ir/components.js'
import type { RawComponent, RawComponentPage } from '../ir/raw-components.js'
import type { TextStyle, TokenCollection } from '../ir/schemas.js'
import { ColorResolver } from './colors.js'
import { stateMotion, variantLoops } from './motion.js'
import { applyTextRoles } from './text-roles.js'
import { mapVariants, viewKey, type MappedVariant, type VariantMapping } from './variants.js'
import {
  buildView,
  graftAddedLayers,
  stateRules,
  type NumberTokens,
  type RefTarget,
  type Report,
  type ViewEnv
} from './views.js'

type ComponentPageInput = { name: string; category: string; data: RawComponentPage }

type SpecInput = {
  pages: ComponentPageInput[]
  reservedNames?: string[]
  config: FigmaConfig
  collections: TokenCollection[]
  textStyles: TextStyle[]
  icons: Map<string, string>
}

type Pending = {
  raw: RawComponent
  page: ComponentPageInput
  name: string
  mapping: VariantMapping
}

const capitalize = (word: string) => word.charAt(0).toUpperCase() + word.slice(1)

function componentName(raw: RawComponent, config: FigmaConfig): string {
  const configured = config.componentNames[raw.name]
  if (configured) return assertIdentifier(configured)
  if (/^[A-Z][A-Za-z0-9]*$/.test(raw.name)) return raw.name
  return assertIdentifier(splitWords(raw.name).map(capitalize).join(''))
}

function numberTokens(collections: TokenCollection[]): NumberTokens {
  const spacing = collections.find((collection) => collection.name === 'Spacing')
  const tokens: NumberTokens = { space: new Map(), radius: new Map() }
  if (!spacing) return tokens
  const index = new TokenIndex(collections)
  for (const token of spacing.variables) {
    const value = index.resolve(token, spacing.modes[0]!) as number
    const variable = `--dcl-${token.name.replace('/', '-')}`
    if (token.name.startsWith('space/')) tokens.space.set(value, variable)
    else if (token.name.startsWith('radius/')) tokens.radius.set(value, variable)
  }
  return tokens
}

function refTargets(pending: Pending[]): Map<string, RefTarget> {
  const targets = new Map<string, RefTarget>()
  for (const entry of pending) {
    for (const variant of entry.mapping.variants) {
      const props: Record<string, string | boolean> = { ...variant.values, ...variant.booleans }
      for (const [flag, on] of Object.entries(variant.flags)) if (on) props[flag] = true
      targets.set(variant.figmaId, { name: entry.name, props })
    }
  }
  return targets
}

function collectSlots(views: ViewSpec[]): SlotSpec[] {
  const slots = new Map<string, SlotSpec>()
  const visit = (node: ViewNode) => {
    if (node.slot && !slots.has(node.slot)) {
      slots.set(node.slot, {
        name: node.slot,
        kind: node.kind === 'text' ? 'text' : 'icon',
        default: node.kind === 'text' ? node.text : undefined
      })
    }
    node.children?.forEach(visit)
  }
  views.forEach((view) => visit(view.root))
  return [...slots.values()]
}

function defaultVariant(raw: RawComponent, variants: MappedVariant[]): MappedVariant | undefined {
  const defaults = Object.entries(raw.properties).filter(
    ([, definition]) => definition.type === 'VARIANT'
  )
  return (
    variants.find((variant) =>
      defaults.every(([name, definition]) => {
        const expected = definition.default === undefined ? undefined : String(definition.default)
        return expected === undefined || variant.figmaName.includes(`${name}=${expected}`)
      })
    ) ?? variants[0]
  )
}

function buildSpec(pending: Pending, env: Omit<ViewEnv, 'report'>): ComponentSpec {
  const { raw, page, name, mapping } = pending
  const report: Report = { notes: new Set(mapping.notes), manual: new Set() }
  const viewEnv: ViewEnv = { ...env, report, images: raw.images }
  const interactions = new Map(
    [raw.base, ...raw.others].map((variant) => [variant.id, variant.interactions])
  )
  const groups = new Map<string, Partial<Record<'base' | 'hover' | 'pressed', MappedVariant>>>()
  for (const variant of mapping.variants) {
    const key = viewKey(variant)
    const group = groups.get(key) ?? {}
    if (!group[variant.interaction]) group[variant.interaction] = variant
    groups.set(key, group)
  }

  const views: ViewSpec[] = []
  const loopCandidates: { figmaId: string; view: ViewSpec }[] = []
  for (const [key, group] of groups) {
    const restVariant = group.base ?? group.hover ?? group.pressed!
    if (!group.base) report.manual.add(`state-without-base-variant:${key}`)
    let root = buildView(restVariant.tree, viewEnv)
    const states: { name: 'hover' | 'pressed'; root: ViewNode }[] = []
    for (const name of ['hover', 'pressed'] as const) {
      const variant = group[name]
      if (!group.base || !variant) continue
      const grafted = graftAddedLayers(root, buildView(variant.tree, viewEnv))
      root = grafted.rest
      states.push({ name, root: grafted.other })
      if (grafted.grafted.length > 0) {
        report.notes.add(`state-reveals-layers:${key}:${name}: ${grafted.grafted.join(', ')}`)
      }
    }
    const view: ViewSpec = {
      key,
      figmaName: restVariant.figmaName,
      figmaId: restVariant.figmaId,
      values: restVariant.values,
      booleans: restVariant.booleans,
      flags: restVariant.flags,
      root
    }
    for (const state of states) {
      view[state.name] = stateRules(root, state.root, report, `${key}:${state.name}`)
      const { motion, issue } = stateMotion(
        interactions.get(group.base!.figmaId),
        state.name,
        group[state.name]!.figmaId
      )
      // A motion CSS cannot reproduce keeps the instant state change, so it is reported without blocking promotion.
      if (issue) report.notes.add(`motion-not-generated:${issue}:${key}:${state.name}`)
      if (motion && view[state.name]!.length > 0)
        view.motion = { ...view.motion, [state.name]: motion }
    }
    views.push(view)
    if (group.base) loopCandidates.push({ figmaId: group.base.figmaId, view })
  }

  const loops = variantLoops(
    loopCandidates.map((candidate) => ({
      ...candidate,
      interactions: interactions.get(candidate.figmaId)
    })),
    (rest, other) => {
      const scratch: Report = { notes: new Set(), manual: new Set() }
      const rules = stateRules(rest, other, scratch, 'loop')
      return { rules, addsLayers: scratch.manual.size > 0 }
    }
  )
  for (const { view, loop, issue } of loops) {
    // Like a state motion, a loop CSS cannot reproduce stays at its rest variant and is reported.
    if (issue) report.notes.add(`motion-not-generated:unsupported-loop:${issue}:${view.key}`)
    if (loop) view.loop = loop
  }

  if (raw.hiddenVariants?.length)
    report.manual.add(
      `hidden-variants-excluded: ${raw.hiddenVariants.map(({ id }) => id).join(', ')}`
    )
  if (raw.truncated)
    report.manual.add('truncated-capture: nested layers beyond the capture budget were dropped')
  if (Object.values(raw.properties).some((property) => property.type === 'SLOT')) {
    report.manual.add('slot-property: Figma slots have no generated equivalent')
  }
  for (const issue of mapping.issues) report.manual.add(issue)

  const fallback = defaultVariant(raw, mapping.variants)
  const defaultKey = fallback ? viewKey(fallback) : (views[0]?.key ?? 'default')
  const hasInteraction = mapping.variants.some((variant) => variant.interaction !== 'base')
  return {
    name,
    figmaName: raw.name,
    figmaId: raw.id,
    page: page.name,
    category: page.category,
    kind: raw.kind,
    description: raw.description,
    tag: hasInteraction || /button|btn|cta/i.test(raw.name) ? 'button' : 'div',
    props: mapping.props,
    slots: collectSlots(views),
    views,
    defaultKey: views.some((view) => view.key === defaultKey)
      ? defaultKey
      : (views[0]?.key ?? 'default'),
    needsManual: [...report.manual].sort(),
    notes: [...report.notes].sort(),
    size: { w: raw.base.tree.w, h: raw.base.tree.h }
  }
}

type NameCollision = {
  name: string
  components: { id: string; figmaName: string; page: string }[]
}

const compareIds = (a: string, b: string) => a.localeCompare(b, 'en', { numeric: true })

// Two live components that generate the same name (case-insensitively, so their files cannot clash
// either) are left out entirely instead of numbered by capture order: a numbered name would swap when
// Figma reorders them. Distinct `componentNames` aliases resolve the collision. A name kept by a retired
// component is different: the survivor deliberately takes the next free number, preserving the public
// name it was already published under.
function resolveNames(input: SpecInput) {
  const groups = new Map<string, { raw: RawComponent; page: ComponentPageInput; name: string }[]>()
  // The same Figma id on two pages is one component captured twice, whatever name or alias each copy
  // carries: it is generated once, at its first page in config order, and later copies are dropped.
  const seen = new Set<string>()
  for (const page of input.pages)
    for (const raw of page.data.components) {
      if (seen.has(raw.id)) continue
      seen.add(raw.id)
      const name = componentName(raw, input.config)
      const key = name.toLowerCase()
      groups.set(key, [...(groups.get(key) ?? []), { raw, page, name }])
    }
  // A survivor's numbered name must also skip every name another live component already holds.
  const used = new Set<string>([
    ...(input.reservedNames ?? []),
    ...[...groups.values()].filter((members) => members.length === 1).map(([only]) => only!.name)
  ])
  const reserved = new Set(input.reservedNames)
  const named = new Map<RawComponent, string>()
  const collisions: NameCollision[] = []
  for (const members of groups.values()) {
    if (members.length > 1) {
      collisions.push({
        name: [...members.map((member) => member.name)].sort()[0]!,
        components: members
          .map(({ raw, page }) => ({ id: raw.id, figmaName: raw.name, page: page.name }))
          .sort((a, b) => compareIds(a.id, b.id))
      })
      continue
    }
    const { raw, name: base } = members[0]!
    let name = base
    if (reserved.has(base)) {
      for (let counter = 2; used.has(name); counter += 1) name = `${base}${counter}`
      used.add(name)
    }
    named.set(raw, name)
  }
  collisions.sort((a, b) => a.name.localeCompare(b.name))
  return { named, collisions }
}

function collisionReason(collision: NameCollision): string {
  const members = collision.components.map((item) => `${item.id} "${item.figmaName}"`).join(', ')
  return `name-collision:${collision.name}: ${members}`
}

// Every captured `token` binding in a component, at any depth: fills, strokes, gradient stops, tints.
function boundTokens(value: unknown, found = new Set<string>()): Set<string> {
  if (Array.isArray(value)) for (const item of value) boundTokens(item, found)
  else if (value && typeof value === 'object')
    for (const [key, item] of Object.entries(value)) {
      if (key === 'token' && typeof item === 'string') found.add(item)
      else boundTokens(item, found)
    }
  return found
}

// An excluded primitive has no CSS variable, so a component bound to it fails instead of emitting an
// undefined var() or silently falling back to a literal colour.
function assertNoExcludedBindings(pending: Pending[], excluded: readonly string[]) {
  for (const { raw, page, name } of pending) {
    const bound = [...boundTokens(raw)].filter((token) => excluded.includes(token))
    if (bound.length)
      throw new Error(
        `Component ${name} (${raw.id}) on page ${page.name} is bound to excluded primitive ${bound.join(', ')}`
      )
  }
}

function buildComponentSpecsWithCollisions(input: SpecInput): {
  specs: ComponentSpec[]
  collisions: NameCollision[]
} {
  const wordAliases = Object.fromEntries(
    Object.entries(input.config.variantAliases).map(([word, alias]) => [word.toLowerCase(), alias])
  )
  const { named, collisions } = resolveNames(input)
  const pending: Pending[] = []
  for (const page of input.pages) {
    for (const raw of page.data.components) {
      const name = named.get(raw)
      if (!name) continue
      const override = input.config.components[raw.name] ?? {}
      const mapping = mapVariants(raw, expandVariants(raw), { wordAliases, ...override })
      pending.push({ raw, page, name, mapping })
    }
  }

  const excluded = input.config.excludedPrimitives ?? []
  assertNoExcludedBindings(pending, excluded)
  // Value matching only sees the tokens that are emitted, so it never picks an excluded primitive.
  const collections = excludePrimitives(input.collections, excluded)
  const refsById = refTargets(pending)
  const env: Omit<ViewEnv, 'report'> = {
    colors: new ColorResolver(collections),
    textStyles: new Map(input.textStyles.map((style) => [style.name, style])),
    textStyleAliases: input.config.textStyleAliases,
    numbers: numberTokens(collections),
    icons: input.icons,
    iconAliases: input.config.iconAliases,
    refsById
  }
  const specs = pending.map((entry) => {
    const spec = buildSpec(entry, env)
    const control = input.config.components[entry.raw.name]?.control
    if (control) {
      if (spec.tag !== 'button') throw new Error(`${spec.name}: button control needs a button root`)
      spec.control = control
    }
    const exposeTextSlots = input.config.components[entry.raw.name]?.exposeTextSlots
    if (exposeTextSlots !== undefined) spec.exposeTextSlots = exposeTextSlots
    const textRoles = input.config.components[entry.raw.name]?.textRoles
    if (textRoles) withTextRoles(spec, textRoles)
    // Node ids only bind roles: they never reach the spec, its hash, generated code or comparisons.
    spec.views.forEach((view) => dropFigmaIds(view.root))
    const semantics = input.config.components[entry.raw.name]?.semantics
    if (!semantics) return spec
    const names = new Set<string>()
    const boxes = (node: ViewNode) => {
      if (node.kind === 'box') names.add(node.name)
      node.children?.forEach(boxes)
    }
    spec.views.forEach((view) => boxes(view.root))
    // Figma names are not unique across pages, so a configured name a same-named component lacks
    // is reported in its notes instead of failing the whole generation.
    const unknown = Object.keys(semantics).filter((name) => name !== '@root' && !names.has(name))
    const notes = unknown.length
      ? [...spec.notes, `semantics: no box named ${unknown.join(', ')}`]
      : spec.notes
    return { ...spec, notes, semantics }
  })
  return { specs, collisions }
}

function dropFigmaIds(node: ViewNode): void {
  delete node.figmaId
  node.children?.forEach(dropFigmaIds)
}

// Binds the configured text roles and recollects the slots, since bound nodes now carry the role as their slot.
function withTextRoles(spec: ComponentSpec, roles: Record<string, TextRole>): ComponentSpec {
  spec.textRoles = applyTextRoles(spec, roles)
  spec.slots = collectSlots(spec.views)
  return spec
}

function buildComponentSpecs(input: SpecInput): ComponentSpec[] {
  return buildComponentSpecsWithCollisions(input).specs
}

export { buildComponentSpecs, buildComponentSpecsWithCollisions, collisionReason, withTextRoles }
export type { ComponentPageInput, NameCollision, SpecInput }
