// Stable config IDs choose which existing Review components may refresh their evidence.
function captureScope(catalog, config, requested) {
  if (requested === undefined) return { selected: catalog.components, frozen: [] }
  const ids = new Set(requested.split(',').filter(Boolean))
  const configured = Object.entries(config.pages).map(([key, page]) => ({
    id: page.id === undefined ? (/^\d+:\d+$/.test(key) ? key : null) : page.id,
    name: page.name ?? key
  }))
  const known = configured.filter((page) => page.id !== null).map((page) => page.id)
  if (new Set(known).size !== known.length) throw new Error('Ambiguous configured page IDs')
  for (const id of ids)
    if (!configured.some((page) => page.id === id))
      throw new Error(`Unknown capture page ID: ${id}`)
  const names = new Set(configured.filter((page) => ids.has(page.id)).map((page) => page.name))
  if (configured.some((page) => !ids.has(page.id) && names.has(page.name)))
    throw new Error('Ambiguous canonical component page owner')
  return {
    selected: catalog.components.filter((component) => names.has(component.page)),
    frozen: catalog.components.filter((component) => !names.has(component.page))
  }
}

function preserveFrozenCases(frozen, previous) {
  const cases = {}
  for (const component of frozen)
    for (const [index, variant] of component.variants.entries()) {
      const key = `${component.name}-${index}`
      const evidence = previous?.cases?.[key]
      if (!evidence || evidence.sourceId !== variant.id)
        throw new Error(`Missing frozen visual evidence: ${key}`)
      // A kept case carries the signature it was captured under (its own, or else the report it came from), so
      // a partial capture cannot pass its old renders off under the signature of the new report. Evidence from a
      // report that had no signature is marked so that it can never match one.
      cases[key] = {
        ...evidence,
        signature: evidence.signature ?? previous.signature ?? 'unsigned'
      }
    }
  return cases
}

// The cases of a report that were not captured under `signature`. A freshly captured case has no signature of
// its own and belongs to the report; a kept one names the capture it came from.
function staleCases(report, signature) {
  return Object.entries(report.cases)
    .filter(([, evidence]) => (evidence.signature ?? report.signature) !== signature)
    .map(([key]) => key)
}
export { captureScope, preserveFrozenCases, staleCases }
