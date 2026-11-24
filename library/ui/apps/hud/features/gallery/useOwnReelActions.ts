import { useCallback, useRef } from 'react'
import type { GalleryPhoto } from '@explorer-core/engine/protocol'
import type { ReelWrite } from '../session/useEngineSession'
import type { ReelList } from './useReelList'
import { REEL_MESSAGES, confirmReelDelete, reelToast } from './reelActions'

export interface ReelWrites {
  setPublic: (id: string, isPublic: boolean) => Promise<ReelWrite>
  remove: (id: string) => Promise<ReelWrite>
}

interface OwnReelActions {
  /** Resolves whether the service took the change. */
  setPublic: (photo: GalleryPhoto, isPublic: boolean) => Promise<boolean>
  /** Asks first; resolves whether the photo was deleted. */
  remove: (photo: GalleryPhoto) => Promise<boolean>
}

/** `publicOnly`: the list holds only public photos (a passport). A photo made private leaves it at
 *  once and comes back if the write fails, and a delete's storage counts, which include private
 *  photos, are not the list's total. */
export function useOwnReelActions(
  writes: ReelWrites,
  list: Pick<ReelList, 'remove' | 'restore' | 'setPublic' | 'setStorage'>,
  { publicOnly = false }: { publicOnly?: boolean } = {}
): OwnReelActions {
  const { setPublic: write, remove: del } = writes
  const { remove: drop, restore, setPublic: mark, setStorage } = list

  // A photo's writes go one at a time, so the service takes them in the order asked.
  const lastWrite = useRef(new Map<string, Promise<boolean>>())
  const setPublic = useCallback(
    (photo: GalleryPhoto, isPublic: boolean): Promise<boolean> => {
      const at = publicOnly && !isPublic ? drop(photo.id) : -1
      const run = async (): Promise<boolean> => {
        const r = await write(photo.id, isPublic)
        if (!r.ok) {
          if (at >= 0) restore(photo, at)
          reelToast(REEL_MESSAGES.failure, 'error')
          return false
        }
        mark(photo.id, isPublic)
        reelToast(REEL_MESSAGES.updated)
        return true
      }
      const writes = lastWrite.current
      const result = (writes.get(photo.id) ?? Promise.resolve(true)).then(run, run)
      writes.set(photo.id, result)
      const done = (): void => {
        if (writes.get(photo.id) === result) writes.delete(photo.id)
      }
      void result.then(done, done)
      return result
    },
    [publicOnly, write, drop, restore, mark]
  )

  const remove = useCallback(
    async (photo: GalleryPhoto): Promise<boolean> => {
      if (!(await confirmReelDelete())) return false
      const r = await del(photo.id)
      if (!r.ok) {
        reelToast(REEL_MESSAGES.failure, 'error')
        return false
      }
      drop(photo.id)
      if (!publicOnly && r.current != null && r.max != null) setStorage(r.current, r.max)
      reelToast(REEL_MESSAGES.deleted)
      return true
    },
    [publicOnly, del, drop, setStorage]
  )

  return { setPublic, remove }
}
