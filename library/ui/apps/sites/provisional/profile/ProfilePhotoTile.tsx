// TODO(figma-ds): replace with the Figma-generated photo tile from library/ui/components.
import { GeneralImage, GeneralLock } from '../../../../foundations/icons'
import { useBrokenImage } from '../useBrokenImage'
import styles from './ProfilePhotoTile.module.css'

type ProfilePhotoTileProps = {
  imageUrl: string
  /** Names the button, e.g. "Photo at Genesis Plaza, November 08, 2026". */
  label: string
  /** Marks a snapshot only its owner can see. */
  isPrivate?: boolean
  onOpen: () => void
}

/** A snapshot in the profile's photo grid; the whole tile opens the viewer. */
function ProfilePhotoTile({ imageUrl, label, isPrivate = false, onOpen }: ProfilePhotoTileProps) {
  const { broken, ref, onError } = useBrokenImage(imageUrl)
  const name = isPrivate ? `${label}, private` : label

  return (
    <button type="button" className={styles.root} aria-label={name} onClick={onOpen}>
      {broken ? (
        <span className={styles.broken} aria-hidden="true">
          <GeneralImage size={40} />
        </span>
      ) : (
        <img
          ref={ref}
          className={styles.image}
          src={imageUrl}
          alt=""
          loading="lazy"
          onError={onError}
        />
      )}
      {isPrivate ? (
        <span className={styles.privateBadge} aria-hidden="true">
          <GeneralLock size={16} />
        </span>
      ) : null}
    </button>
  )
}

export { ProfilePhotoTile }
export type { ProfilePhotoTileProps }
