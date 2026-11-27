// The order of every list the gallery shows: by label, whatever order its entries are declared in.

const collator = new Intl.Collator('en', { sensitivity: 'base', numeric: true })

/**
 * Two labels in the gallery's order: alphabetical whatever the case, digits by their value (Item2 before Item10).
 * Labels that differ by case alone fall back to their code units, so the order never depends on the declared one.
 */
export function compareLabels(a: string, b: string): number {
  return collator.compare(a, b) || (a < b ? -1 : a > b ? 1 : 0)
}

/** A copy of a list in the gallery's order, by the label each item is shown under. */
export function sortByLabel<T>(items: readonly T[], label: (item: T) => string): T[] {
  return [...items].sort((a, b) => compareLabels(label(a), label(b)))
}
