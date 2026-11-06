import type { ComponentType } from 'react'

type ForcedState = 'hover' | 'pressed'

type Selection = {
  values: Record<string, string>
  booleans: Record<string, boolean>
  flags: { selected?: boolean; disabled?: boolean; loading?: boolean }
}

const FLAG_ORDER = ['selected', 'disabled', 'loading'] as const

const registry = new Map<string, ComponentType<Record<string, unknown>>>()

// Generated components register themselves so that views can embed each other without import cycles.
function registerComponent(name: string, component: ComponentType<never>): void {
  registry.set(name, component as unknown as ComponentType<Record<string, unknown>>)
}

function viewKey(selection: Selection, dropped = 0): string {
  const flags = FLAG_ORDER.filter((flag) => selection.flags[flag])
  const parts = [
    ...Object.entries(selection.values).map(([name, value]) => `${name}:${value}`),
    ...Object.entries(selection.booleans)
      .filter(([, on]) => on)
      .map(([name]) => name),
    ...flags.slice(0, flags.length - dropped)
  ]
  return parts.join('|') || 'default'
}

// A variant combination that Figma never drew (for example "selected" + "disabled") falls back to the
// closest drawn one by dropping flags from the end, then to the component's default view.
function pickView<T>(views: Record<string, T>, defaultKey: string, selection: Selection): T {
  const flagCount = FLAG_ORDER.filter((flag) => selection.flags[flag]).length
  for (let dropped = 0; dropped <= flagCount; dropped += 1) {
    const view = views[viewKey(selection, dropped)]
    if (view) return view
  }
  const fallback = views[defaultKey] ?? Object.values(views)[0]
  if (!fallback) throw new Error('A generated component must have at least one view')
  return fallback
}

export { pickView, registerComponent, registry, viewKey }
export type { ForcedState, Selection }
