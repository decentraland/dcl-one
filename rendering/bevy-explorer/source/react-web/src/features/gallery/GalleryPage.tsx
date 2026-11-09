// React Gallery (camera reel) — a full-screen page inside the shared MainMenuShell, matching
// unity-explorer's Camera Reel: the local player's photos, newest first, under month headers, read 100
// at a time as the grid scrolls (useReelList + ReelGrid), with a storage indicator and the per-photo
// options menu. A thumbnail opens the full-screen PhotoDetail lightbox. Data comes from the bridge's
// signed relay of camera-reel-service (see bridge-scene/domains/gallery).

import { useCallback, useState } from 'react'
import { EmptyState, Icon, Spinner } from '../../design'
import { MainMenuShell } from '../menu/MainMenuShell'
import { PhotoDetail } from './PhotoDetail'
import { ReelGrid, type ReelGridLayout, type ReelGridMenu } from './ReelGrid'
import { useReelList } from './useReelList'
import { REEL_MESSAGES, confirmReelDelete, reelToast } from './reelActions'
import type { GalleryState, ProfileState } from '../session/useEngineSession'
import type { GalleryPhoto } from '../../engine/protocol'
import type { ChatUser } from '../chat/ProfileCardPresentation'
import styles from './GalleryPage.module.css'

// The explore panel's CameraReelGallery: 6 columns of 306x230, 30 px padding (Plugin Settings,
// CameraReelGallery.prefab), month headers, the 16x41 options button.
const GALLERY_LAYOUT: ReelGridLayout = {
  columns: 6,
  tileWidth: 306,
  tileHeight: 230,
  groupByMonth: true,
  paddingTop: 30,
  paddingBottom: 30,
  paddingLeft: 30,
  monthSpacing: 20,
  optionButtonHeight: 41
}

/** The gallery's photo actions that write to the service, shared with the lightbox. */
export function useOwnReelActions(
  gallery: GalleryState,
  list: { remove: (id: string) => void; setPublic: (id: string, isPublic: boolean) => void; setStorage: (total: number, max: number) => void }
): { setPublic: (photo: GalleryPhoto, isPublic: boolean) => Promise<boolean>; remove: (photo: GalleryPhoto) => Promise<boolean> } {
  const { setPublic: write, remove: del } = gallery
  const { remove: drop, setPublic: mark, setStorage } = list
  const setPublic = useCallback(
    (photo: GalleryPhoto, isPublic: boolean): Promise<boolean> =>
      write(photo.id, isPublic).then((r) => {
        if (!r.ok) {
          reelToast(REEL_MESSAGES.failure, 'error')
          return false
        }
        mark(photo.id, isPublic)
        reelToast(REEL_MESSAGES.updated)
        return true
      }),
    [write, mark]
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
      if (r.current != null && r.max != null) setStorage(r.current, r.max)
      reelToast(REEL_MESSAGES.deleted)
      return true
    },
    [del, drop, setStorage]
  )
  return { setPublic, remove }
}

export function GalleryPage({
  gallery,
  profile,
  onNavigate,
  onTeleport,
  onViewProfile
}: {
  gallery: GalleryState
  profile: ProfileState
  onNavigate: (page: string) => void
  /** Jump to a photo's parcel (closes the gallery). */
  onTeleport: (x: number, y: number) => void
  /** Open a captured person's passport. */
  onViewProfile?: (user: ChatUser) => void
}): React.JSX.Element | null {
  const [index, setIndex] = useState<number | null>(null)
  // Every open reads the reel again from the top, as Unity's gallery does when it is shown, with the
  // viewer closed: the page stays mounted while the gallery is closed, so it would keep its index.
  const [opened, setOpened] = useState(0)
  const [wasOpen, setWasOpen] = useState(gallery.open)
  if (gallery.open !== wasOpen) {
    setWasOpen(gallery.open)
    if (gallery.open) {
      setOpened((n) => n + 1)
      setIndex(null)
    }
  }
  const list = useReelList(gallery.open ? `own:${opened}` : null, gallery.fetchPage)
  const actions = useOwnReelActions(gallery, list)
  const menu: ReelGridMenu = { showDelete: true, onSetPublic: actions.setPublic, onDelete: (photo) => void actions.remove(photo) }

  if (!gallery.open) return null

  const p = profile.data
  const selected = index != null ? list.photos[index] : null
  const pct = list.max > 0 ? Math.min(100, (list.total / list.max) * 100) : 0

  return (
    <MainMenuShell
      active="gallery"
      profileName={p?.name}
      profilePicture={p?.picture}
      profileAddress={p?.address}
      profileClaimed={p?.hasClaimedName}
      onNavigate={onNavigate}
      onClose={gallery.toggle}
      flushBody
    >
      <div className={styles.head}>
        <h1 className={styles.title}>Gallery</h1>
        {list.loaded && list.max > 0 && (
          <div className={styles.storage} title={`${list.total} of ${list.max} photos used`}>
            <div className={styles.storageBar}>
              <div className={styles.storageFill} style={{ '--fill': `${pct}%` } as React.CSSProperties} />
            </div>
            <span className={styles.storageText}>
              Storage {list.total}/{list.max} photos taken
            </span>
          </div>
        )}
      </div>

      {!list.loaded ? (
        <div className={styles.center}>
          <Spinner />
        </div>
      ) : list.photos.length === 0 ? (
        <EmptyState
          variant="screen"
          iconWash
          icon={<Icon name="gallery" size={56} />}
          title="There are no photos yet"
          subtitle={
            <span className={styles.emptyBody}>
              Use the <span className={styles.emptyCamera}>camera</span> to save incredible memories with your friends!
            </span>
          }
        />
      ) : (
        <ReelGrid photos={list.photos} layout={GALLERY_LAYOUT} menu={menu} onOpen={setIndex} onVisibleIndex={list.loadMore} aria-label="Photos" />
      )}

      {selected && index != null && (
        <PhotoDetail
          photos={list.photos}
          index={index}
          meta={gallery.metas[selected.id]}
          isSelf
          onIndex={(i) => {
            setIndex(i)
            list.loadMore(i)
          }}
          onLoadMeta={gallery.loadPhoto}
          onClose={() => setIndex(null)}
          onTeleport={(x, y) => {
            setIndex(null)
            gallery.toggle() // close the gallery so the teleport lands in-world
            onTeleport(x, y)
          }}
          onDelete={(photo) =>
            void actions.remove(photo).then((removed) => {
              if (removed) setIndex(null)
            })
          }
          onViewPerson={onViewProfile}
        />
      )}
    </MainMenuShell>
  )
}
