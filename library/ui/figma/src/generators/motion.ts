import type { Loop, Motion, ViewNode, ViewSpec } from '../ir/components.js'

type Decl = [string, string]
type State = 'hover' | 'pressed'
type StateExtras = Map<string, { decls: Decl[]; starting?: Decl[] }>
type BaseEntry = {
  properties: Set<string>
  discrete: boolean
  decls: Map<string, string>
  starting: Map<string, string>
}

const STATES: State[] = ['hover', 'pressed']
const pathKey = (path: number[]) => path.join('.')
const declOf = (decls: Decl[], property: string) => decls.find(([name]) => name === property)?.[1]
const timing = (motion: Motion): Decl[] => [
  ['transition-duration', `${motion.duration}ms`],
  ['transition-timing-function', motion.easing]
]
const sameMotion = (a: Motion, b: Motion) => a.duration === b.duration && a.easing === b.easing

function nodesByPath(root: ViewNode): Map<string, ViewNode> {
  const nodes = new Map<string, ViewNode>()
  const visit = (node: ViewNode) => {
    nodes.set(pathKey(node.path), node)
    node.children?.forEach(visit)
  }
  visit(root)
  return nodes
}

function entryOf(base: Map<string, BaseEntry>, key: string): BaseEntry {
  const entry = base.get(key) ?? {
    properties: new Set<string>(),
    discrete: false,
    decls: new Map<string, string>(),
    starting: new Map<string, string>()
  }
  base.set(key, entry)
  return entry
}

// The rest view carries the transition, so leaving a state animates back with the same motion. A layer a
// state shows or hides through `display` cannot interpolate, so it fades: `allow-discrete` keeps it rendered
// while its opacity runs, and `@starting-style` gives the entering side its starting opacity.
function plan(view: ViewSpec) {
  const nodes = nodesByPath(view.root)
  const restMotion = view.motion?.hover ?? view.motion?.pressed
  const base = new Map<string, BaseEntry>()
  const states: Record<State, StateExtras> = { hover: new Map(), pressed: new Map() }
  for (const state of STATES) {
    const motion = view.motion?.[state]
    if (!motion || !restMotion) continue
    for (const rule of view[state] ?? []) {
      const key = pathKey(rule.path)
      // The ring overlay is its own box: its opacity and paint transition on the pseudo-element itself.
      if (rule.after) {
        const ringKey = `${key}::after`
        const ring = entryOf(base, ringKey)
        rule.after.forEach(([property]) => ring.properties.add(property))
        if (!sameMotion(motion, restMotion)) states[state].set(ringKey, { decls: timing(motion) })
      }
      // A rule that only changes the ring leaves the element itself without a transition.
      if (rule.decls.length === 0) continue
      const node = nodes.get(key)
      const entry = entryOf(base, key)
      rule.decls.forEach(([property]) => entry.properties.add(property))
      const decls: Decl[] = sameMotion(motion, restMotion) ? [] : timing(motion)
      let starting: Decl[] | undefined
      const shown = declOf(rule.decls, 'display')
      const restHidden = node ? declOf(node.css.decls, 'display') === 'none' : false
      if (shown !== undefined && (shown === 'none') !== restHidden) {
        entry.discrete = true
        entry.properties.add('opacity')
        const ownOpacity = declOf(rule.decls, 'opacity')
        if (shown === 'none') {
          if (ownOpacity === undefined) decls.push(['opacity', '0'])
          entry.starting.set('opacity', '0')
        } else {
          if (ownOpacity === undefined)
            decls.push(['opacity', (node && declOf(node.css.decls, 'opacity')) ?? '1'])
          entry.decls.set('opacity', '0')
          starting = [['opacity', '0']]
        }
      }
      if (decls.length > 0 || starting) states[state].set(key, { decls, starting })
    }
  }
  return { base, states, restMotion }
}

// Rest-view lines for the root rule, plus the declarations each state rule needs on top of its diff.
function viewMotion(view: ViewSpec): { rest: string[]; states: Record<State, StateExtras> } {
  const { base, states, restMotion } = plan(view)
  if (!restMotion || base.size === 0) return { rest: [], states }
  const body = (entry: BaseEntry, indent: string): string[] => [
    ...[...entry.decls].map(([property, value]) => `${indent}${property}: ${value};`),
    `${indent}transition-property: ${[...entry.properties].join(', ')};`,
    ...timing(restMotion).map(([property, value]) => `${indent}${property}: ${value};`),
    ...(entry.discrete ? [`${indent}transition-behavior: allow-discrete;`] : []),
    ...(entry.starting.size > 0
      ? [
          `${indent}@starting-style {`,
          ...[...entry.starting].map(([property, value]) => `${indent}  ${property}: ${value};`),
          `${indent}}`
        ]
      : [])
  ]
  const root = base.get('')
  const lines = root ? body(root, '') : []
  for (const [key, entry] of base) {
    if (key === '') continue
    const [path, pseudo = ''] = key.split('::')
    const selector = `&${path ? ` [data-n='${path}']` : ''}${pseudo ? `::${pseudo}` : ''}`
    lines.push(`${selector} {`, ...body(entry, '  '), '}')
  }
  const rings = [...base.keys()].some((key) => key.endsWith('::after'))
  lines.push(
    '@media (prefers-reduced-motion: reduce) {',
    `  &, & [data-n]${rings ? ', &::after, & [data-n]::after' : ''} {`,
    '    transition: none;',
    '  }',
    '}'
  )
  return { rest: lines, states }
}

const declBlock = (decls: Decl[], extra: Decl[] = []) =>
  [...decls, ...extra].map(([property, value]) => `${property}: ${value};`).join(' ')

// A loop plays on its rest view: move from A to B over `duration` with the easing, then restart at A with
// no transition, which is what an infinite iteration without `alternate` does. The keyframe names are
// local to the component's CSS module.
function viewLoop(view: ViewSpec, prefix: string): { rest: string[]; keyframes: string[] } {
  const loop: Loop | undefined = view.loop
  if (!loop) return { rest: [], keyframes: [] }
  const rest: string[] = []
  const keyframes: string[] = []
  for (const track of loop.tracks) {
    const key = pathKey(track.path)
    const name = `${prefix}loop${key ? `_${track.path.join('_')}` : ''}`
    keyframes.push(
      [
        `@keyframes ${name} {`,
        `  0% { ${declBlock(track.from, [['animation-timing-function', loop.easing]])} }`,
        `  100% { ${declBlock(track.to)} }`,
        '}'
      ].join('\n')
    )
    const animation = `animation: ${name} ${loop.duration}ms linear infinite;`
    if (key === '') rest.push(animation)
    else rest.push(`& [data-n='${key}'] {`, `  ${animation}`, '}')
  }
  rest.push(
    '@media (prefers-reduced-motion: reduce) {',
    '  &, & [data-n] {',
    '    animation: none;',
    '  }',
    '}'
  )
  return { rest, keyframes }
}

export { viewLoop, viewMotion }
export type { StateExtras }
