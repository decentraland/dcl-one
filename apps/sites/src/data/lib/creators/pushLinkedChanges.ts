// Sends already-reviewed linked items' changes for review again, one item curation per item, the way the legacy
// builder's pushChangesToThirdPartyItems did: a few requests at a time, and one failure never stops the rest.
import { CurationStatus } from './collections'
import { type Item } from './items'
import { type ItemCuration } from './linkedItemStatus'

export type PushLinkedFailure = { item: Item; message: string }

export type PushLinkedResult = { pushed: Item[]; failed: PushLinkedFailure[] }

export type PushLinkedDeps = {
  pushItemCuration: (itemId: string) => Promise<ItemCuration>
  updateItemCuration: (itemId: string, status: 'pending') => Promise<ItemCuration>
}

export const DEFAULT_PUSH_CONCURRENCY = 3

function toMessage(error: unknown): string {
  if (error instanceof Error) return error.message
  return typeof error === 'string' ? error : 'Push failed'
}

async function runPool(tasks: Array<() => Promise<void>>, concurrency: number): Promise<void> {
  let next = 0
  const worker = async () => {
    while (next < tasks.length) {
      const task = tasks[next++]
      await task()
    }
  }
  const workers = Math.max(1, Math.min(concurrency, tasks.length))
  await Promise.all(Array.from({ length: workers }, worker))
}

/**
 * Pushes every item's changes. `curations` holds each item's latest item curation: a pending one is refreshed
 * with the item's current hash (PATCH), any other opens a new request (POST). Results keep the input order.
 */
export async function pushLinkedChanges(
  items: Item[],
  curations: Map<string, ItemCuration>,
  deps: PushLinkedDeps,
  onProgress?: (done: number, total: number) => void,
  concurrency = DEFAULT_PUSH_CONCURRENCY
): Promise<PushLinkedResult> {
  const failures: Array<string | undefined> = new Array<string | undefined>(items.length)
  let done = 0
  const tasks = items.map((item, index) => async () => {
    try {
      if (curations.get(item.id)?.status === CurationStatus.PENDING) {
        await deps.updateItemCuration(item.id, 'pending')
      } else {
        await deps.pushItemCuration(item.id)
      }
    } catch (error) {
      failures[index] = toMessage(error)
    }
    done++
    onProgress?.(done, items.length)
  })
  await runPool(tasks, concurrency)

  const result: PushLinkedResult = { pushed: [], failed: [] }
  items.forEach((item, index) => {
    const message = failures[index]
    if (message === undefined) result.pushed.push(item)
    else result.failed.push({ item, message })
  })
  return result
}
