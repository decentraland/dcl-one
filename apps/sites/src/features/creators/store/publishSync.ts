// Collections whose chain→server publish sync is running in this tab, so a page mount doesn't start a second one
// and the publishing tab doesn't offer to publish again while its own transaction is still pending.
import { create } from 'zustand'

type PublishSyncState = {
  syncing: ReadonlySet<string>
  start: (collectionId: string) => void
  end: (collectionId: string) => void
}

export const usePublishSyncStore = create<PublishSyncState>(set => ({
  syncing: new Set(),
  start: collectionId => set(state => ({ syncing: new Set(state.syncing).add(collectionId) })),
  end: collectionId =>
    set(state => {
      const syncing = new Set(state.syncing)
      syncing.delete(collectionId)
      return { syncing }
    })
}))

export const isPublishSyncing = (collectionId: string) => usePublishSyncStore.getState().syncing.has(collectionId)

/** Whether this tab is still waiting for the collection's publish transaction or its server sync. */
export function usePublishInFlight(collectionId: string | undefined): boolean {
  return usePublishSyncStore(state => !!collectionId && state.syncing.has(collectionId))
}
