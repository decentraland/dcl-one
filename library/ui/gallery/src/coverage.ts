import type { Exclusion, GalleryEntry } from './registry'

const FORWARD_REF = Symbol.for('react.forward_ref')
const MEMO = Symbol.for('react.memo')

/** Whether an export is a React component: a PascalCase function, `forwardRef` or `memo` value. */
export function isComponentExport(name: string, value: unknown): boolean {
  if (!/^[A-Z]/.test(name)) return false
  if (typeof value === 'function') return true
  if (typeof value !== 'object' || value === null) return false
  const type = (value as { $$typeof?: unknown }).$$typeof
  return type === FORWARD_REF || type === MEMO
}

/**
 * The React components a package entry exports (`isComponentExport`). Names the foundations entry also exports (the
 * icons) are foundations, which the gallery shows on its own pages.
 */
export function componentExports(
  entry: Readonly<Record<string, unknown>>,
  foundations: Readonly<Record<string, unknown>>
): string[] {
  return Object.keys(entry)
    .filter((name) => !(name in foundations) && isComponentExport(name, entry[name]))
    .sort()
}

/** Every reason the gallery does not account for each public component exactly once; empty when it does. */
export function coverageProblems(
  components: readonly string[],
  entries: readonly Pick<GalleryEntry, 'name' | 'components' | 'examples'>[],
  exclusions: readonly Exclusion[]
): string[] {
  const problems: string[] = []
  const exported = new Set(components)
  const shown = new Set<string>()
  const pages = new Set<string>()
  for (const entry of entries) {
    if (pages.has(entry.name)) problems.push(`${entry.name}: the page name is repeated`)
    pages.add(entry.name)
    if (!entry.examples.length) problems.push(`${entry.name}: the page has no example`)
    for (const name of entry.components) {
      if (!exported.has(name))
        problems.push(`${entry.name}: lists ${name}, which is not a public component export`)
      shown.add(name)
    }
  }
  const excluded = new Set<string>()
  for (const { name, reason } of exclusions) {
    if (!reason.trim()) problems.push(`${name}: the exclusion gives no reason`)
    if (!exported.has(name)) problems.push(`${name}: excluded, but not a public component export`)
    if (shown.has(name)) problems.push(`${name}: both excluded and shown`)
    excluded.add(name)
  }
  for (const name of components)
    if (!shown.has(name) && !excluded.has(name))
      problems.push(`${name}: no example renders it and no exclusion explains why`)
  return problems
}

/**
 * The exports a page claims in `components` that none of its examples mounted. `mounted` holds the public exports
 * that rendered while the page's examples were drawn, as the instrumented render test records them.
 */
export function claimProblems(
  entry: Pick<GalleryEntry, 'name' | 'components'>,
  mounted: ReadonlySet<string>
): string[] {
  return entry.components
    .filter((name) => !mounted.has(name))
    .map((name) => `${entry.name}: claims ${name}, but no example mounts it`)
}
