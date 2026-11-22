import { createElement, type ComponentType } from 'react'
import { isComponentExport } from '../src/coverage'

/**
 * A copy of a module's exports in which every component, by the coverage rule's own predicate, records its name when
 * it mounts and renders the original. Other exports are kept as they are.
 */
export function instrumentComponents(
  exports: Readonly<Record<string, unknown>>,
  onMount: (name: string) => void
): Record<string, unknown> {
  return Object.fromEntries(
    Object.entries(exports).map(([name, value]) => {
      if (!isComponentExport(name, value)) return [name, value]
      const Original = value as ComponentType<Record<string, unknown>>
      const Recorded = (props: Record<string, unknown>) => {
        onMount(name)
        return createElement(Original, props)
      }
      return [name, Recorded]
    })
  )
}
