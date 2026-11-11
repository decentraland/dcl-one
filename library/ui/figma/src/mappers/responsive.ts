import type { ComponentSpec, ResponsiveConfig } from '../ir/components.js'

// Resolves a configured responsive axis against the foundations breakpoints. Every option of the prop names one
// breakpoint, no two share a minimum width, the narrowest starts at 0 so every width shows a view, and every other
// axis combination is drawn for every option: a caller that omits the prop then gets a view at any width, and the
// generated type, which makes the prop optional on every drawn combination, stays true.
function resolveResponsive(
  spec: ComponentSpec,
  config: ResponsiveConfig,
  breakpoints: ReadonlyMap<string, number>
): NonNullable<ComponentSpec['responsive']> {
  const fail = (message: string): never => {
    throw new Error(`${spec.name} responsive: ${message}`)
  }
  const prop = spec.props.find((item) => item.name === config.prop)
  if (prop?.kind !== 'enum') fail(`${config.prop} is not a variant prop`)
  const options = prop!.values
  for (const value of Object.keys(config.breakpoints))
    if (!options.includes(value)) fail(`${value} is not an option of ${config.prop}`)
  const resolved = options.map((value) => {
    const name = config.breakpoints[value]
    if (name === undefined) fail(`option ${value} names no breakpoint`)
    const min = breakpoints.get(name!)
    if (min === undefined)
      fail(
        `${value} names the unknown breakpoint ${name} (foundations: ${[...breakpoints.keys()].join(', ') || 'none'})`
      )
    return { value, min: min! }
  })
  resolved.sort((a, b) => a.min - b.min)
  resolved.forEach((item, index) => {
    const next = resolved[index + 1]
    if (next && next.min === item.min)
      fail(`${item.value} and ${next.value} share the minimum width ${item.min}px`)
  })
  if (resolved[0]!.min !== 0)
    fail(`the narrowest option ${resolved[0]!.value} must start at 0px, not ${resolved[0]!.min}px`)

  const rest = (view: ComponentSpec['views'][number]) =>
    JSON.stringify({
      values: Object.entries(view.values).filter(([name]) => name !== config.prop),
      booleans: view.booleans,
      flags: view.flags
    })
  const drawn = new Map(options.map((value) => [value, new Set<string>()]))
  for (const view of spec.views) drawn.get(view.values[config.prop]!)?.add(rest(view))
  const all = new Set([...drawn.values()].flatMap((rows) => [...rows]))
  for (const [value, rows] of drawn)
    for (const row of all)
      if (!rows.has(row)) fail(`${config.prop}=${value} has no view for ${row}`)
  return { prop: config.prop, breakpoints: resolved }
}

export { resolveResponsive }
