// The passport's Photos tab — unity-explorer's PassportController gallery: the same paged reel grid
// as the Gallery, with the player's PUBLIC photos (read unsigned, so your own passport too), 4 columns
// of 272x201 with no month headers. On your own passport each photo has the options menu without
// Delete, and making a photo private takes it off the grid at once (HideReelOnPrivateSet). A
// thumbnail opens the photo viewer over the passport.

import { useCallback, useState } from 'react'
import { EmptyState, Spinner } from '../../design'
import type { GalleryPhoto, GalleryPhotoMeta } from '../../engine/protocol'
import type { ReelWrite } from '../session/useEngineSession'
import type { ChatUser } from '../chat/ProfileCardPresentation'
import { PhotoDetail } from './PhotoDetail'
import { fetchPublicReelPage } from './publicReels'
import { ReelGrid, type ReelGridLayout, type ReelGridMenu } from './ReelGrid'
import { useReelList } from './useReelList'
import { REEL_MESSAGES, confirmReelDelete, reelToast } from './reelActions'
import styles from './PassportPhotos.module.css'

// Plugin Settings' passport gallery (4 x 272x201) and the Passport prefab's paddings (2 / 12 / 0).
// The HUD's passport panel is narrower than Unity's full-screen one, so the tiles keep 272:201 and
// share the width.
const PASSPORT_LAYOUT: ReelGridLayout = {
  columns: 4,
  tileWidth: 272,
  tileHeight: 201,
  fitWidth: true,
  groupByMonth: false,
  paddingTop: 2,
  paddingBottom: 12,
  paddingLeft: 0,
  monthSpacing: 20,
  optionButtonHeight: 40
}

/** What your own passport's photos can do: the signed writes and the viewer's metadata. */
export interface PassportPhotoActions {
  setPublic: (id: string, isPublic: boolean) => Promise<ReelWrite>
  remove: (id: string) => Promise<ReelWrite>
  metas: Record<string, GalleryPhotoMeta | null | undefined>
  loadPhoto: (id: string) => void
  onTeleport: (x: number, y: number) => void
  onViewPerson?: (user: ChatUser) => void
}

export function PassportPhotos({ address, isSelf, actions }: { address: string; isSelf: boolean; actions?: PassportPhotoActions }): React.JSX.Element {
  const key = address.toLowerCase()
  const fetchPage = useCallback((offset: number, limit: number) => fetchPublicReelPage(key, offset, limit), [key])
  const list = useReelList(key, fetchPage)
  const [index, setIndex] = useState<number | null>(null)
  const { remove: drop, restore, setPublic: mark } = list
  const write = actions?.setPublic

  const onSetPublic = useCallback(
    async (photo: GalleryPhoto, isPublic: boolean): Promise<boolean> => {
      if (write == null) return false
      // Off the grid at once, as Unity's HideReelOnPrivateSet; back in its place if the write fails.
      const at = isPublic ? -1 : drop(photo.id)
      const r = await write(photo.id, isPublic)
      if (!r.ok) {
        if (at >= 0) restore(photo, at)
        reelToast(REEL_MESSAGES.failure, 'error')
        return false
      }
      mark(photo.id, isPublic)
      reelToast(REEL_MESSAGES.updated)
      return true
    },
    [write, drop, restore, mark]
  )
  const menu: ReelGridMenu | null = isSelf && write != null ? { showDelete: false, onSetPublic } : null

  const selected = index != null ? list.photos[index] : null

  return (
    <section className={styles.section}>
      {!list.loaded ? (
        <div className={styles.center}>
          <Spinner size={34} />
        </div>
      ) : list.photos.length === 0 ? (
        <EmptyState
          variant="inline"
          title="No photos here yet!"
          subtitle="✨ Set your photos as public to make them visible in your Profile & Communities ✨"
        />
      ) : (
        <ReelGrid photos={list.photos} layout={PASSPORT_LAYOUT} menu={menu} onOpen={setIndex} onVisibleIndex={list.loadMore} aria-label="Photos" />
      )}
      {selected != null && index != null && (
        <PhotoDetail
          photos={list.photos}
          index={index}
          meta={actions?.metas[selected.id]}
          isSelf={isSelf}
          onIndex={(i) => {
            setIndex(i)
            list.loadMore(i)
          }}
          onLoadMeta={actions?.loadPhoto ?? noop}
          onClose={() => setIndex(null)}
          onTeleport={(x, y) => {
            setIndex(null)
            actions?.onTeleport(x, y)
          }}
          onDelete={
            isSelf && actions != null
              ? (photo) =>
                  void (async () => {
                    if (!(await confirmReelDelete())) return
                    const r = await actions.remove(photo.id)
                    if (!r.ok) {
                      reelToast(REEL_MESSAGES.failure, 'error')
                      return
                    }
                    drop(photo.id)
                    setIndex(null)
                    reelToast(REEL_MESSAGES.deleted)
                  })()
              : undefined
          }
          onViewPerson={actions?.onViewPerson}
        />
      )}
    </section>
  )
}

const noop = (): void => {}
