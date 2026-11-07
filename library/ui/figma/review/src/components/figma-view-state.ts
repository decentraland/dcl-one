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

// What a generated component does with a selection of its variant axes that Figma never drew. The typed props
// already reject one; this decides for untyped callers (plain JS, spread props). 'throw' fails loudly, naming the
// component and the axis values. 'default' renders the component's default view, which silently changes what the
// caller asked for (a viewport, a signed-in state, a theme). One setting for every environment, server rendering
// included; the decided policy is 'throw'.
const UNCAPTURED_SELECTION: 'throw' | 'default' = 'throw'

function describeSelection(selection: Selection): string {
  const axes = [
    ...Object.entries(selection.values).map(([name, value]) => `${name}=${JSON.stringify(value)}`),
    ...Object.entries(selection.booleans).map(([name, on]) => `${name}=${Boolean(on)}`)
  ]
  return axes.join(', ') || 'no variant axes'
}

// Interaction flags Figma did not draw in combination (for example "selected" + "disabled") fall back to the
// closest drawn view by dropping flags from the end. Any other undrawn combination follows UNCAPTURED_SELECTION.
function pickView<T>(
  views: Record<string, T>,
  defaultKey: string,
  selection: Selection,
  component = 'A generated component',
  policy: 'throw' | 'default' = UNCAPTURED_SELECTION
): T {
  const flagCount = FLAG_ORDER.filter((flag) => selection.flags[flag]).length
  for (let dropped = 0; dropped <= flagCount; dropped += 1) {
    const view = views[viewKey(selection, dropped)]
    if (view) return view
  }
  if (policy === 'throw')
    throw new Error(`${component} has no captured view for ${describeSelection(selection)}`)
  const fallback = views[defaultKey] ?? Object.values(views)[0]
  if (!fallback) throw new Error('A generated component must have at least one view')
  return fallback
}

export { UNCAPTURED_SELECTION, pickView, registerComponent, registry, viewKey }
export type { ForcedState, Selection }
