import { EmptyState, Spinner } from '@dcl/one-ui'
import { useState } from 'react'
import { Icon } from '../../design'
import { MainMenuShell } from '../menu/MainMenuShell'
import { MenuBackdrop } from '../menu/MenuBackdrop'
import { usePhotoViewer, type PhotoViewerProps } from './usePhotoViewer'
import { ReelGrid, type ReelGridLayout, type ReelGridMenu } from './ReelGrid'
import { useReelList } from './useReelList'
import { useOwnReelActions } from './useOwnReelActions'
import type { GalleryState, ProfileState, TravelOutcome } from '../session/useEngineSession'
import type { PhotoPlace } from './reelActions'
import type { ChatUser } from '../chat/ProfileCardPresentation'
import styles from './GalleryPage.module.css'

// Six columns of 306x230 fill the 1920 canvas with its 30 px padding.
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
const COMPACT_GALLERY_LAYOUT: ReelGridLayout = { ...GALLERY_LAYOUT, columns: 5, tileWidth: 292, tileHeight: 219 }

export function GalleryPage({
  gallery,
  profile,
  onNavigate,
  onJumpIn,
  onViewProfile,
  compact
}: {
  gallery: GalleryState
  profile: ProfileState
  onNavigate: (page: string) => void
  /** Travel to where a photo was taken (closes the gallery). */
  onJumpIn: (place: PhotoPlace) => TravelOutcome
  /** Open a captured person's passport. */
  onViewProfile?: (user: ChatUser) => void
  /** Hosted in the lobby's section modal. */
  compact?: { onTravel: () => void }
}): React.JSX.Element | null {
  const active = gallery.open || compact != null
  const [index, setIndex] = useState<number | null>(null)
  // The page stays mounted while closed, so each open resets it and reads the reel from the top:
  // photos taken since the last open show up.
  const [opened, setOpened] = useState(0)
  const [wasOpen, setWasOpen] = useState(active)
  if (active !== wasOpen) {
    setWasOpen(active)
    if (active) {
      setOpened((n) => n + 1)
      setIndex(null)
    }
  }
  const list = useReelList(active ? `own:${opened}` : null, gallery.fetchPage)
  const actions = useOwnReelActions(gallery, list)
  const menu: ReelGridMenu = { showDelete: true, onSetPublic: actions.setPublic, onDelete: (photo) => void actions.remove(photo) }

  const selected = active && index != null ? (list.photos[index] ?? null) : null
  const detail: PhotoViewerProps | null =
    selected != null && index != null
      ? {
          photos: list.photos,
          index,
          meta: gallery.metas[selected.id],
          canEdit: true,
          onIndex: (i) => {
            setIndex(i)
            list.loadMore(i)
          },
          onLoadMeta: gallery.loadPhoto,
          loadWearables: gallery.loadWearables,
          onClose: () => setIndex(null),
          onSetPublic: actions.setPublic,
          onJumpIn: (place) => {
            const outcome = onJumpIn(place)
            if (outcome === 'refused') return
            setIndex(null)
            if (compact != null) compact.onTravel()
            // over the startup lobby the pick has closed the gallery already
            else if (outcome === 'sent') gallery.toggle()
          },
          onDelete: (photo) =>
            void actions.remove(photo).then((removed) => {
              if (removed) setIndex(null)
            }),
          onViewPerson: onViewProfile
        }
      : null

  usePhotoViewer(detail)

  if (!active) return null

  const p = profile.data
  const pct = list.max > 0 ? Math.min(100, (list.total / list.max) * 100) : 0

  const body = (
    <>
      <div className={styles.head}>
        <h1 className={styles.title}>Gallery</h1>
        {list.loaded && list.max > 0 && (
          <div className={styles.storage} title={`${list.total} of ${list.max} photos used`}>
            <span className={styles.storageText}>
              Storage {list.total}/{list.max} photos taken
            </span>
            <div className={styles.storageBar}>
              <div className={styles.storageFill} style={{ '--fill': `${pct}%` } as React.CSSProperties} />
            </div>
          </div>
        )}
      </div>

      {!list.loaded ? (
        <div className={styles.center}>
          <Spinner variant="thin" />
        </div>
      ) : list.error && list.photos.length === 0 ? (
        <EmptyState
          variant="screen"
          tone="error"
          title="Your photos couldn't be loaded"
          subtitle="Check your connection and try again."
          actions={[{ label: 'RETRY', onClick: list.retry }]}
        />
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
        <ReelGrid
          photos={list.photos}
          layout={compact != null ? COMPACT_GALLERY_LAYOUT : GALLERY_LAYOUT}
          menu={menu}
          onOpen={setIndex}
          onVisibleIndex={list.loadMore}
          onRetry={list.error ? list.retry : undefined}
          aria-label="Photos"
        />
      )}
    </>
  )
  if (compact != null) return <div className={styles.compact}>{body}</div>
  return (
    <MainMenuShell
      active="gallery"
      profileName={p?.name}
      profilePicture={p?.picture}
      profileAddress={p?.address}
      profileClaimed={p?.hasClaimedName}
      onNavigate={onNavigate}
      onClose={gallery.toggle}
      transparentBody
    >
      <div className={styles.page}>
        <MenuBackdrop />
        {body}
      </div>
    </MainMenuShell>
  )
}
