// The reel is read unsigned, so even your own passport lists only PUBLIC photos.

import { useCallback, useState } from 'react'
import { EmptyState, Spinner } from '../../design'
import type { GalleryPhotoMeta, Wearable } from '@explorer-core/engine/protocol'
import type { ReelWrite } from '../session/useEngineSession'
import type { ChatUser } from '../chat/ProfileCardPresentation'
import { usePhotoViewer } from './usePhotoViewer'
import type { PhotoPlace } from './reelActions'
import { fetchPublicReelPage } from './publicReels'
import { ReelGrid, type ReelGridLayout, type ReelGridMenu } from './ReelGrid'
import { useReelList } from './useReelList'
import { useOwnReelActions, type ReelWrites } from './useOwnReelActions'
import styles from './PassportPhotos.module.css'

// The passport panel is narrow, so four tiles share its width and keep a 272:201 shape.
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

export interface PassportPhotoActions {
  setPublic: (id: string, isPublic: boolean) => Promise<ReelWrite>
  remove: (id: string) => Promise<ReelWrite>
  metas: Record<string, GalleryPhotoMeta | null | undefined>
  loadPhoto: (id: string) => void
  loadWearables: (urns: string[]) => Promise<Wearable[] | null>
  /** False when the travel is refused: the photo stays open. */
  onJumpIn: (place: PhotoPlace) => boolean
  onViewPerson?: (user: ChatUser) => void
}

export function PassportPhotos({
  address,
  isSelf,
  actions,
  empty
}: {
  address: string
  isSelf: boolean
  actions?: PassportPhotoActions
  /** Shown for a reel with no public photos. */
  empty: React.ReactNode
}): React.JSX.Element {
  const key = address.toLowerCase()
  const fetchPage = useCallback((offset: number, limit: number) => fetchPublicReelPage(key, offset, limit), [key])
  const list = useReelList(key, fetchPage)
  const [index, setIndex] = useState<number | null>(null)
  const writes = useOwnReelActions(actions ?? NO_WRITES, list, { publicOnly: true })
  const menu: ReelGridMenu | null =
    isSelf && actions != null ? { showDelete: true, onSetPublic: writes.setPublic, onDelete: (photo) => void writes.remove(photo) } : null

  // A photo made private leaves this public-only list: show the next one, wrapping, or close when none are left.
  if (index != null && list.loaded && index >= list.photos.length) setIndex(list.photos.length > 0 ? index % list.photos.length : null)
  const selected = index != null ? list.photos[index] : null
  usePhotoViewer(
    selected != null && index != null
      ? {
          photos: list.photos,
          index,
          meta: actions?.metas[selected.id],
          canEdit: menu != null,
          onIndex: (i) => {
            setIndex(i)
            list.loadMore(i)
          },
          onLoadMeta: actions?.loadPhoto ?? noop,
          loadWearables: actions?.loadWearables ?? noWearables,
          onClose: () => setIndex(null),
          onSetPublic: menu != null ? writes.setPublic : undefined,
          onJumpIn: (place) => {
            if (actions?.onJumpIn(place)) setIndex(null)
          },
          onDelete:
            menu != null
              ? (photo) =>
                  void writes.remove(photo).then((removed) => {
                    if (removed) setIndex(null)
                  })
              : undefined,
          onViewPerson: actions?.onViewPerson
        }
      : null
  )

  return (
    <section className={styles.section}>
      {!list.loaded ? (
        <div className={styles.center}>
          <Spinner size={34} />
        </div>
      ) : list.error && list.photos.length === 0 ? (
        <EmptyState variant="inline" tone="error" title="Photos couldn't be loaded" actions={[{ label: 'RETRY', onClick: list.retry }]} />
      ) : list.photos.length === 0 ? (
        empty
      ) : (
        <ReelGrid
          photos={list.photos}
          layout={PASSPORT_LAYOUT}
          menu={menu}
          onOpen={setIndex}
          onVisibleIndex={list.loadMore}
          onRetry={list.error ? list.retry : undefined}
          aria-label="Photos"
        />
      )}
    </section>
  )
}

const noop = (): void => {}
const noWearables = async (): Promise<null> => null
// Never called: without `actions` there is no menu.
const NO_WRITES: ReelWrites = { setPublic: async () => ({ ok: false }), remove: async () => ({ ok: false }) }
