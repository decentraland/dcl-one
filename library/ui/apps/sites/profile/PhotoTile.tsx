import { Button } from '../../../index'
import { GeneralImage, GeneralLock } from '../../../foundations/icons'
import { useBrokenImage } from '../useBrokenImage'
import styles from './PhotoTile.module.css'

/** A snapshot in the grid; the whole tile opens the viewer. */
export function PhotoTile({
  image,
  label,
  isPrivate,
  onOpen
}: {
  image: string
  label: string
  isPrivate: boolean
  onOpen: () => void
}) {
  const { broken, ref, onError } = useBrokenImage(image)
  return (
    <button
      type="button"
      className={styles.tile}
      aria-label={isPrivate ? `${label}, private` : label}
      onClick={onOpen}
    >
      {broken ? (
        <span className={styles.tileBroken} aria-hidden="true">
          <GeneralImage size={40} />
        </span>
      ) : (
        <img
          ref={ref}
          className={styles.tileImage}
          src={image}
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

/** The public gallery stays visible if the signed read for private photos fails. */
export function PrivatePhotosNotice({ onRetry }: { onRetry: () => void }) {
  return (
    <span role="status">
      Your private photos could not be loaded.{' '}
      <Button variant="text" size="sm" onClick={onRetry}>
        Try again
      </Button>
    </span>
  )
}
