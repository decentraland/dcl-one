import type { Loop, Motion, ViewNode, ViewSpec } from '../ir/components.js'
import type { RawInteraction } from '../ir/raw-components.js'
import { loopPayloadChange } from './views.js'

// "While hovering" and "While pressing" revert on their own, which is what a CSS state transition does.
const TRIGGERS = { hover: 'ON_HOVER', pressed: 'ON_PRESS' } as const

// Figma's standard curves are the CSS keyword curves; back and spring curves have no CSS equivalent here.
const EASINGS: Record<string, string> = {
  LINEAR: 'linear',
  EASE_IN: 'ease-in',
  EASE_OUT: 'ease-out',
  EASE_IN_AND_OUT: 'ease-in-out'
}

type MotionResult = { motion?: Motion; issue?: string }

// CSS rejects a cubic-bezier whose x control points fall outside [0, 1], and any non-finite point.
const validBezier = (points: number[]) =>
  points.length === 4 &&
  points.every((value) => Number.isFinite(value)) &&
  [points[0]!, points[2]!].every((x) => x >= 0 && x <= 1)

function curve(transition: NonNullable<RawInteraction['transition']>): string | undefined {
  if (transition.easing !== 'CUSTOM_CUBIC_BEZIER' || !transition.bezier)
    return EASINGS[transition.easing]
  if (!validBezier(transition.bezier)) return undefined
  return `cubic-bezier(${transition.bezier.map((value) => Number(value.toFixed(4))).join(', ')})`
}

function stateMotion(
  interactions: RawInteraction[] | undefined,
  state: 'hover' | 'pressed',
  destinationId: string
): MotionResult {
  const interaction = interactions?.find(
    (candidate) =>
      candidate.trigger === TRIGGERS[state] && candidate.destinationId === destinationId
  )
  const transition = interaction?.transition
  if (!transition) return {}
  if (transition.type !== 'SMART_ANIMATE')
    return { issue: `unsupported-transition:${transition.type}` }
  const easing = curve(transition)
  if (!easing) return { issue: `unsupported-easing:${transition.easing}` }
  return { motion: { duration: Math.round(transition.duration * 1000), easing } }
}

// The unit of an AFTER_TIMEOUT wait is not established: Figma documents milliseconds, while the Skeleton's
// "immediately" reads 0.001. A raw wait at or below 0.016 is immediate under either unit (at most one 60Hz
// frame). Any longer wait is reported, never converted, until a known wait is measured end to end.
const IMMEDIATE_RAW = 0.016
// Properties a loop may move. Anything else (a fill, a layer that appears) is not a sweep and stays static.
const LOOP_PROPERTIES = new Set([
  'left',
  'top',
  'right',
  'bottom',
  'inset',
  'width',
  'height',
  'transform',
  'opacity'
])

type LoopVariant = { figmaId: string; view: ViewSpec; interactions?: RawInteraction[] }
type LoopResult = { view: ViewSpec; loop?: Loop; issue?: string }

const timed = (interactions: RawInteraction[] | undefined) =>
  (interactions ?? []).filter((interaction) => interaction.trigger === 'AFTER_TIMEOUT')
// Why a wait cannot be generated, or undefined for an immediate one.
function waitIssue(timeout: number | undefined): string | undefined {
  if (timeout === undefined) return 'missing-timeout'
  if (!Number.isFinite(timeout) || timeout < 0) return 'invalid-timeout'
  if (timeout > IMMEDIATE_RAW) return 'unverified-wait'
  return undefined
}

function nodeAt(root: ViewNode, path: number[]): ViewNode | undefined {
  let node: ViewNode | undefined = root
  for (const index of path) node = node?.children?.find((child) => child.path.at(-1) === index)
  return node
}

function selection(view: ViewSpec): Record<string, string> {
  const entries: [string, string][] = [
    ...Object.entries(view.values),
    ...Object.entries(view.booleans).map(([key, value]): [string, string] => [key, String(value)]),
    ...Object.entries(view.flags).map(([key, value]): [string, string] => [key, String(value)])
  ]
  return Object.fromEntries(entries)
}

// The variant properties that differ between two views. A loop toggles exactly one.
function differingProperties(a: ViewSpec, b: ViewSpec): string[] {
  const left = selection(a)
  const right = selection(b)
  return [...new Set([...Object.keys(left), ...Object.keys(right)])].filter(
    (key) => left[key] !== right[key]
  )
}

// Figma's timed loop: variant A waits, Smart-Animates to B, B waits and changes back to A with no
// transition. That is a CSS animation on A with an instant restart. Only that two-variant shape, moving
// only geometry or opacity, is generated; every other timed change is reported, never guessed.
function variantLoops(
  variants: LoopVariant[],
  diff: (
    rest: ViewNode,
    other: ViewNode
  ) => {
    rules: { path: number[]; decls: [string, string][] }[]
    addsLayers: boolean
  }
): LoopResult[] {
  const byId = new Map(variants.map((variant) => [variant.figmaId, variant]))
  const results: LoopResult[] = []
  const consumed = new Set<string>()
  for (const a of variants) {
    const outgoing = timed(a.interactions)
    if (outgoing.length === 0 || consumed.has(a.figmaId)) continue
    // The instant change back is the reset half of a loop; its partner (the animated side) reports it.
    const partner = outgoing.length === 1 ? byId.get(outgoing[0]!.destinationId) : undefined
    const partnerTimed = timed(partner?.interactions)
    if (
      !outgoing[0]!.transition &&
      partnerTimed.some((change) => change.destinationId === a.figmaId && change.transition)
    )
      continue
    const fail = (issue: string) => {
      consumed.add(a.figmaId)
      if (partner) consumed.add(partner.figmaId)
      results.push({ view: a.view, issue })
    }
    if (outgoing.length > 1) {
      fail('several-timed-changes')
      continue
    }
    const forward = outgoing[0]!
    const b = byId.get(forward.destinationId)
    if (!b) {
      fail('destination-outside-set')
      continue
    }
    const back = timed(b.interactions)
    if (back.length !== 1 || back[0]!.destinationId !== a.figmaId) {
      fail(back.length === 0 ? 'no-return' : 'more-than-two-variants')
      continue
    }
    if (back[0]!.transition) {
      fail(`reset-not-instant:${back[0]!.transition.type}`)
      continue
    }
    const wait = waitIssue(forward.timeout) ?? waitIssue(back[0]!.timeout)
    if (wait) {
      fail(wait)
      continue
    }
    const transition = forward.transition
    if (!transition || transition.type !== 'SMART_ANIMATE') {
      fail(`unsupported-transition:${transition?.type ?? 'INSTANT'}`)
      continue
    }
    const duration = Math.round(transition.duration * 1000)
    if (!Number.isFinite(duration) || duration < 1) {
      fail('invalid-duration')
      continue
    }
    const easing = curve(transition)
    if (!easing) {
      fail(`unsupported-easing:${transition.easing}`)
      continue
    }
    if (differingProperties(a.view, b.view).length !== 1) {
      fail('not-sibling-variants')
      continue
    }
    const { rules, addsLayers } = diff(a.view.root, b.view.root)
    const payload = loopPayloadChange(a.view.root, b.view.root)
    if (
      addsLayers ||
      rules.some((rule) => rule.decls.some(([property]) => property === 'display'))
    ) {
      fail('layers-change')
      continue
    }
    if (payload) {
      fail(payload)
      continue
    }
    const moved = rules.flatMap((rule) => rule.decls.map(([property]) => property))
    const unsupported = moved.find((property) => !LOOP_PROPERTIES.has(property))
    if (unsupported) {
      fail(`non-geometry-change:${unsupported}`)
      continue
    }
    if (rules.length === 0) {
      fail('no-change')
      continue
    }
    // Each moved property needs a concrete starting value on A; `auto` cannot be interpolated.
    const implicit: Record<string, string> = { opacity: '1', transform: 'none' }
    let missing: string | undefined
    const tracks = rules.map((rule) => {
      const node = nodeAt(a.view.root, rule.path)
      const from = rule.decls.map(([property]): [string, string] => {
        const value = node?.css.decls.find(([name]) => name === property)?.[1] ?? implicit[property]
        if (value === undefined) missing ??= property
        return [property, value ?? '']
      })
      return { path: rule.path, from, to: rule.decls }
    })
    if (missing) {
      fail(`no-start-value:${missing}`)
      continue
    }
    consumed.add(a.figmaId).add(b.figmaId)
    results.push({
      view: a.view,
      loop: { to: b.view.key, duration, easing, tracks }
    })
  }
  return results
}

export { stateMotion, variantLoops }
