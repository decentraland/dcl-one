type DesignVariant = { properties: Record<string, string> }

function propertyOptions(variants: DesignVariant[]): [string, string[]][] {
  const options = new Map<string, Set<string>>()
  for (const variant of variants)
    for (const [name, value] of Object.entries(variant.properties)) {
      if (!options.has(name)) options.set(name, new Set())
      options.get(name)!.add(value)
    }
  return [...options].map(([name, values]) => [name, [...values]])
}

// Keep the chosen property and preserve as many other values as the captured combinations allow.
function selectVariant(
  variants: DesignVariant[],
  current: number,
  property: string,
  value: string
): number {
  let best = current
  let bestScore = -1
  const previous = variants[current]?.properties ?? {}
  variants.forEach((variant, index) => {
    if (variant.properties[property] !== value) return
    const score = Object.entries(previous).filter(
      ([name, item]) => name !== property && variant.properties[name] === item
    ).length
    if (score > bestScore) {
      best = index
      bestScore = score
    }
  })
  return best
}

export { propertyOptions, selectVariant }
