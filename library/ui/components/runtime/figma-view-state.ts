import type { ComponentType } from 'react'

type ForcedState = 'hover' | 'pressed'

type Selection = {
  // Undefined only for a responsive axis the caller left unset, which FigmaView resolves before picking a view.
  values: Record<string, string | undefined>
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

function describeSelection(selection: Selection): string {
  const axes = [
    ...Object.entries(selection.values).map(([name, value]) => `${name}=${JSON.stringify(value)}`),
    ...Object.entries(selection.booleans).map(([name, on]) => `${name}=${JSON.stringify(on)}`)
  ]
  return axes.join(', ') || 'no variant axes'
}

// Interaction flags Figma did not draw in combination (for example "selected" + "disabled") fall back to the
// closest drawn view by dropping flags from the end. Any other selection Figma never drew throws, in every
// environment and during server rendering. The generator types a sparse component's axes as a union of the drawn
// combinations (its variant contract) and only emits defaults that are drawn, so only an untyped caller (plain JS,
// spread props) reaches here; rendering another view would silently change the viewport, state or theme asked for.
function pickView<T>(
  views: Record<string, T>,
  selection: Selection,
  component = 'A generated component'
): T {
  // A boolean axis is true or false; any other value (a string from an untyped caller) selects nothing drawn.
  if (Object.values(selection.booleans).some((on) => typeof on !== 'boolean'))
    throw new Error(`${component} has no captured view for ${describeSelection(selection)}`)
  const flagCount = FLAG_ORDER.filter((flag) => selection.flags[flag]).length
  for (let dropped = 0; dropped <= flagCount; dropped += 1) {
    const view = views[viewKey(selection, dropped)]
    if (view) return view
  }
  throw new Error(`${component} has no captured view for ${describeSelection(selection)}`)
}

export { pickView, registerComponent, registry, viewKey }
export type { ForcedState, Selection }
