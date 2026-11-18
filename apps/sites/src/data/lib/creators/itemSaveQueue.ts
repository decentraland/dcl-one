import { type Item } from './items'

export type SaveItemPatch = (item: Item, patch: Partial<Item>) => Promise<Item>

/**
 * builder-server saves an item whole (PUT), so two overlapping saves of one item can undo each other: a URN edit built
 * from a list that hasn't caught up with a mapping save, or two mapping saves applied out of order. Saves of the same
 * item run one after another here, each built from the newest copy known: the one last saved or the one passed in.
 */
export function createItemSaveQueue(save: (item: Item) => Promise<Item>): SaveItemPatch {
  const latest = new Map<string, Item>()
  const tails = new Map<string, Promise<unknown>>()
  return (item, patch) => {
    const run = (tails.get(item.id) ?? Promise.resolve())
      .catch(() => undefined)
      .then(async () => {
        const known = latest.get(item.id)
        const base = known && known.updatedAt >= item.updatedAt ? known : item
        const saved = await save({ ...base, ...patch })
        latest.set(item.id, saved)
        return saved
      })
    tails.set(item.id, run)
    return run
  }
}
