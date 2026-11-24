// Full-screen viewer of one camera-reel photo: the photo with its actions and an info panel (date,
// owner, place, people in the shot) that the info button slides away. Opened through usePhotoViewer,
// as a popup of its own.

import { useEffect, useRef, useState } from 'react'
import { ControlButton, MaskIcon, ScrollThumb, Skeleton, Spinner, Toggle } from '../../design'
import { useWindowKeyDown } from '../../lib/useWindowKeyDown'
import { track } from '@explorer-core/lib/analytics'
import { photoTime } from '../session/useEngineSession'
import { useOptionalSession } from '../session/SessionContext'
import type { GalleryPhoto, GalleryPhotoMeta, Wearable } from '@explorer-core/engine/protocol'
import type { ChatUser } from '../chat/ProfileCardPresentation'
import { copyReelLink, downloadReel, photoDate, shareReelToX, type PhotoPlace } from './reelActions'
import { PhotoPersonRow } from './PhotoPersonRow'
import shareIcon from '../../assets/camera-reel/share-x.png'
import linkIcon from '../../assets/camera-reel/copy-link.png'
import downloadIcon from '../../assets/camera-reel/download.png'
import deleteIcon from '../../assets/camera-reel/delete.png'
import hideShowIcon from '../../assets/camera-reel/hide-show.png'
import arrowLeft from '../../assets/map/panel/arrow-left.png'
import arrowRight from '../../assets/map/panel/arrow-right.png'
import closeIcon from '../../assets/map/panel/close-thin.png'
import pinIcon from '../../assets/map/panel/pin-outline.png'
import jumpInIcon from '../../assets/friends/jump-in.png'
import styles from './PhotoDetail.module.css'

export function PhotoDetail({
  photos,
  index,
  meta,
  canEdit,
  onIndex,
  onLoadMeta,
  loadWearables,
  onClose,
  onJumpIn,
  onSetPublic,
  onDelete,
  onViewPerson,
  covered = false
}: {
  photos: GalleryPhoto[]
  index: number
  meta: GalleryPhotoMeta | null | undefined
  /** The photos are the viewer's own: the public toggle and Delete show. */
  canEdit: boolean
  onIndex: (i: number) => void
  onLoadMeta: (id: string) => void
  /** The wearables a person had on (urns from the photo's metadata); null when the read fails. */
  loadWearables: (urns: string[]) => Promise<Wearable[] | null>
  onClose: () => void
  /** Travel to where the photo was taken; the caller closes the viewer when it goes. */
  onJumpIn: (place: PhotoPlace) => void
  /** Make the photo public or private; resolves whether the service took it. */
  onSetPublic?: (photo: GalleryPhoto, isPublic: boolean) => Promise<boolean>
  /** Delete the photo (the caller asks to confirm). */
  onDelete?: (photo: GalleryPhoto) => void
  /** Open a person's passport: the one who took it, or one in it. */
  onViewPerson?: (user: ChatUser) => void
  /** Another popup is over the viewer: the arrow keys are that popup's. */
  covered?: boolean
}): React.JSX.Element {
  const photo = photos[index]
  const openExternalUrl = useOptionalSession()?.openExternalUrl
  const hasPrev = index > 0
  const hasNext = index < photos.length - 1
  const [panelOpen, setPanelOpen] = useState(true)
  const [loadedUrl, setLoadedUrl] = useState<string | null>(null)
  // Optimistic: reverts if the service refuses the write.
  const [publicChoice, setPublicChoice] = useState<{ id: string; isPublic: boolean; turn: number } | null>(null)
  const choiceTurn = useRef(0)
  const isPublic = publicChoice?.id === photo.id ? publicChoice.isPublic : photo.isPublic === true

  useEffect(() => {
    track('open_photo')
  }, [])

  useEffect(() => {
    if (meta === undefined) onLoadMeta(photo.id)
  }, [photo.id, meta, onLoadMeta])

  useWindowKeyDown(
    (e) => {
      if (e.key === 'ArrowLeft' && hasPrev) {
        onIndex(index - 1)
      } else if (e.key === 'ArrowRight' && hasNext) {
        onIndex(index + 1)
      }
    },
    { enabled: !covered }
  )

  const setPublic = (next: boolean): void => {
    if (onSetPublic == null) return
    const id = photo.id
    const turn = ++choiceTurn.current
    setPublicChoice({ id, isPublic: next, turn })
    void onSetPublic(photo, next).then(() => setPublicChoice((cur) => (cur?.turn === turn ? null : cur)))
  }

  const loaded = loadedUrl === photo.url
  const showToggle = canEdit && onSetPublic != null
  const showDelete = canEdit && onDelete != null

  return (
    <div className={styles.viewer} role="dialog" aria-modal="true" aria-label="Photo">
      <div className={`${styles.imagePanel} ${panelOpen ? '' : styles.imagePanelFull}`.trim()}>
        <img
          key={photo.url}
          className={`${styles.image} ${loaded ? styles.imageShown : ''}`.trim()}
          src={photo.url}
          alt=""
          onLoad={() => setLoadedUrl(photo.url)}
        />
        {!loaded && (
          <span className={styles.imageSpinner}>
            <Spinner size={60} variant="chase" />
          </span>
        )}
        <span className={styles.gradient} aria-hidden="true" />

        <ControlButton className={`${styles.square} ${styles.close}`} aria-label="Close" onClick={onClose}>
          <MaskIcon src={closeIcon} size={16} />
        </ControlButton>
        {hasPrev && (
          <ControlButton className={`${styles.square} ${styles.nav} ${styles.navPrev}`} aria-label="Previous photo" onClick={() => onIndex(index - 1)}>
            <MaskIcon src={arrowLeft} width={14} height={22} />
          </ControlButton>
        )}
        {hasNext && (
          <ControlButton className={`${styles.square} ${styles.nav} ${styles.navNext}`} aria-label="Next photo" onClick={() => onIndex(index + 1)}>
            <MaskIcon src={arrowRight} width={14} height={22} />
          </ControlButton>
        )}

        <div className={styles.actions}>
          {showToggle && (
            <>
              <label className={styles.publicToggle}>
                <span>Set as Public</span>
                <Toggle checked={isPublic} onChange={setPublic} aria-label="Set as Public" />
              </label>
              <span className={styles.separator} aria-hidden="true" />
            </>
          )}
          <ControlButton
            className={`${styles.square} ${styles.action}`}
            aria-label="Share on X"
            onClick={() => {
              shareReelToX(photo.id, openExternalUrl)
              track('share_photo')
            }}
          >
            <MaskIcon src={shareIcon} size={25} />
          </ControlButton>
          <ControlButton className={`${styles.square} ${styles.action}`} aria-label="Copy Link" onClick={() => copyReelLink(photo.id)}>
            <MaskIcon src={linkIcon} size={25} />
          </ControlButton>
          <ControlButton
            className={`${styles.square} ${styles.action}`}
            aria-label="Download"
            onClick={() =>
              void downloadReel(photo.url).then((saved) => {
                if (saved) track('download_photo')
              })
            }
          >
            <MaskIcon src={downloadIcon} size={25} />
          </ControlButton>
          {showDelete && (
            <ControlButton className={`${styles.square} ${styles.action}`} aria-label="Delete" onClick={() => onDelete(photo)}>
              <MaskIcon src={deleteIcon} size={25} />
            </ControlButton>
          )}
          <span className={styles.separator} aria-hidden="true" />
          <ControlButton
            className={`${styles.square} ${styles.infoToggle}`}
            aria-label={panelOpen ? 'Hide photo info' : 'Show photo info'}
            aria-expanded={panelOpen}
            onClick={() => setPanelOpen((v) => !v)}
          >
            <MaskIcon src={hideShowIcon} width={22} height={16} className={panelOpen ? styles.infoIcon : styles.infoIconFlipped} />
          </ControlButton>
        </div>

        <PhotoInfo
          key={photo.id}
          photo={photo}
          meta={meta}
          loadWearables={loadWearables}
          onJumpIn={(place) => {
            track('photo_jump_to')
            onJumpIn(place)
          }}
          onViewPerson={onViewPerson}
          openExternalUrl={openExternalUrl}
          hidden={!panelOpen}
        />
      </div>
    </div>
  )
}

function PhotoInfo({
  photo,
  meta,
  loadWearables,
  onJumpIn,
  onViewPerson,
  openExternalUrl,
  hidden
}: {
  photo: GalleryPhoto
  meta: GalleryPhotoMeta | null | undefined
  loadWearables: (urns: string[]) => Promise<Wearable[] | null>
  onJumpIn: (place: PhotoPlace) => void
  onViewPerson?: (user: ChatUser) => void
  openExternalUrl?: (url: string) => Promise<boolean>
  hidden: boolean
}): React.JSX.Element {
  const scroller = useRef<HTMLDivElement>(null)
  const date = photoDate(photoTime(meta?.dateTime ?? photo.dateTime))
  const owner = meta?.userName
  const ownerAddress = meta?.userAddress
  const x = meta?.x
  const y = meta?.y
  const people = meta?.people ?? []
  return (
    <aside className={styles.info} aria-label="Photo info" inert={hidden}>
      <div className={styles.dateRow}>
        <span>{date}</span>
        {owner != null && owner !== '' && (
          <>
            <span className={styles.takenBy}>&nbsp;- Taken by&nbsp;</span>
            <button
              type="button"
              className={styles.owner}
              disabled={onViewPerson == null || ownerAddress == null}
              onClick={() => ownerAddress != null && onViewPerson?.({ address: ownerAddress, name: owner })}
            >
              {owner}
            </button>
          </>
        )}
      </div>
      <span className={`${styles.label} ${styles.placeLabel}`}>PLACE</span>
      {x != null && y != null && (
        <button type="button" className={styles.jumpIn} aria-label={`Jump In to ${meta?.sceneName ?? ''} ${x}, ${y}`.replace(/\s+/g, ' ')} onClick={() => onJumpIn({ realm: meta?.realm, x, y })}>
          <MaskIcon src={pinIcon} size={24} />
          <span className={styles.sceneText}>{`${meta?.sceneName ?? ''}, ${x}, ${y}`}</span>
          <img className={styles.jumpIcon} src={jumpInIcon} alt="" width={24} height={24} />
        </button>
      )}
      <span className={styles.divider} aria-hidden="true" />
      <div className={styles.peoplePanel}>
        <span className={`${styles.label} ${styles.peopleLabel}`}>PEOPLE</span>
        <div className={styles.peopleList}>
          <div ref={scroller} className={styles.peopleScroll}>
            {people.map((person) => (
              <PhotoPersonRow key={person.address} person={person} loadWearables={loadWearables} onViewPerson={onViewPerson} openExternalUrl={openExternalUrl} />
            ))}
          </div>
          <ScrollThumb target={scroller} tone="solid" className={styles.peopleThumb} />
        </div>
      </div>
      <InfoLoading done={meta !== undefined} />
    </aside>
  )
}

/** Kept mounted after the metadata lands so it can fade out. */
function InfoLoading({ done }: { done: boolean }): React.JSX.Element | null {
  const [gone, setGone] = useState(done)
  if (gone) return null
  return (
    <div className={`${styles.loading} ${done ? styles.loadingDone : ''}`.trim()} aria-hidden="true" onTransitionEnd={() => done && setGone(true)}>
      <Skeleton tone="band" className={styles.skel} style={{ left: 23, top: 33, width: 294 }} />
      <Skeleton tone="band" className={styles.skel} style={{ left: 23, top: 72, width: 206 }} />
      <Skeleton tone="band" className={styles.skel} style={{ left: 23, top: 102, width: 100 }} />
      <span className={styles.skelLine} style={{ top: 142, left: 0, width: 480 }} />
      <Skeleton tone="band" className={styles.skel} style={{ left: 23.5, top: 163, width: 100 }} />
      {[200, 280, 359].map((top, i) => (
        <span key={top}>
          <Skeleton tone="band" className={styles.skelCircle} style={{ left: 27.5, top }} />
          <Skeleton tone="band" className={styles.skel} style={{ left: 75.5, top: top + 11, width: 140 }} />
          {i < 2 && <span className={styles.skelLine} style={{ top: top + 60, left: 15, width: 450 }} />}
        </span>
      ))}
    </div>
  )
}
