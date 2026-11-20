import {
  useId,
  useLayoutEffect,
  useRef,
  useState,
  type ComponentType,
  type KeyboardEvent,
  type ReactNode
} from 'react'

import { Button, JumpInCTA, Modal, type PlaceCardLinkProps } from '../../../index'
import {
  GeneralClose,
  GeneralInfoButton,
  GeneralLeftArrow,
  GeneralMapPin,
  GeneralRightArrow,
  MenusDownload
} from '../../../foundations/icons'

import styles from './PhotoViewer.module.css'
import { useBrokenImage } from '../useBrokenImage'

export type PhotoViewerPerson = {
  name: string
  /** Their profile page; without one the name is plain text. */
  href?: string
  faceUrl?: string
  isGuest: boolean
}

export type PhotoViewerDetails = {
  /** Already formatted; left out when the service's date cannot be read. */
  date?: string
  takenBy?: { name: string; href: string }
  place: { label: string; jumpInHref?: string }
  people: PhotoViewerPerson[]
}

export type PhotoViewerProps = {
  imageUrl: string
  /** The image's alt text. */
  alt: string
  /** The dialog's accessible name, e.g. "Photo at Genesis Plaza, November 08, 2026, 3 of 24". */
  label: string
  details: PhotoViewerDetails
  /**
   * Where Download saves the full-size image from, and its file name. The node serves reel images
   * from the page's origin; a browser opens a cross-origin image instead of saving it.
   */
  download?: { href: string; fileName: string }
  /** Without one, its button is marked unavailable and its arrow key does nothing. */
  onPrevious?: () => void
  onNext?: () => void
  onClose: () => void
  linkComponent: ComponentType<PlaceCardLinkProps>
}

/** Profile photo and its information panel inside the shared gallery modal. */
export function PhotoViewer({
  imageUrl,
  alt,
  label,
  details,
  download,
  onPrevious,
  onNext,
  onClose,
  linkComponent: Link
}: PhotoViewerProps) {
  const closeRef = useRef<HTMLButtonElement>(null)
  const [detailsVisible, setDetailsVisible] = useState(true)
  const titleId = useId()

  // Stepping to another photo or hiding details can remove the focused link.
  useLayoutEffect(() => {
    const close = closeRef.current
    const dialog = close?.closest('dialog')
    if (dialog?.open && !dialog.contains(document.activeElement)) close?.focus()
  }, [imageUrl, detailsVisible])

  function onKeyDown(event: KeyboardEvent<HTMLDialogElement>) {
    if (event.key === 'ArrowLeft' && onPrevious) {
      event.preventDefault()
      onPrevious()
    } else if (event.key === 'ArrowRight' && onNext) {
      event.preventDefault()
      onNext()
    }
  }

  return (
    <Modal
      layout="gallery"
      title={label}
      closeButton={false}
      initialFocusRef={closeRef}
      onClose={onClose}
      onKeyDown={onKeyDown}
    >
      <Button
        ref={closeRef}
        iconOnly
        variant="dark"
        size="sm"
        className={styles.close}
        aria-label="Close"
        onClick={onClose}
      >
        <GeneralClose size={24} aria-hidden="true" />
      </Button>
      <div className={detailsVisible ? `${styles.body} ${styles.withDetails}` : styles.body}>
        <div className={styles.media}>
          <div className={styles.toolbar}>
            {download ? (
              <Button
                iconOnly
                variant="dark"
                size="sm"
                href={download.href}
                download={download.fileName}
                aria-label="Download photo"
              >
                <MenusDownload size={22} aria-hidden="true" />
              </Button>
            ) : null}
            <Button
              iconOnly
              variant="dark"
              size="sm"
              aria-label="Information"
              aria-pressed={detailsVisible}
              onClick={() => setDetailsVisible((visible) => !visible)}
            >
              <GeneralInfoButton size={22} aria-hidden="true" />
            </Button>
          </div>
          <img
            className={styles.image}
            src={imageUrl}
            alt={alt}
            onMouseDown={(event) => event.preventDefault()}
          />
          {onPrevious || onNext ? (
            <>
              <StepButton
                label="Previous photo"
                className={styles.previous}
                onStep={onPrevious}
                icon={<GeneralLeftArrow size={24} aria-hidden="true" />}
              />
              <StepButton
                label="Next photo"
                className={styles.next}
                onStep={onNext}
                icon={<GeneralRightArrow size={24} aria-hidden="true" />}
              />
            </>
          ) : null}
        </div>
        {detailsVisible ? (
          <div className={styles.details}>
            {/* A tab stop, so the keyboard can scroll a long panel that has no link in it. */}
            <section className={styles.detailsScroll} aria-labelledby={titleId} tabIndex={0}>
              <div className={styles.header}>
                <h2 id={titleId} className={styles.sectionTitle}>
                  information
                </h2>
                {details.date ? <p className={styles.date}>{details.date}</p> : null}
                {details.takenBy ? (
                  <p className={styles.takenBy}>
                    Photo taken by{' '}
                    <Link to={details.takenBy.href} className={styles.link}>
                      {details.takenBy.name}
                    </Link>
                  </p>
                ) : null}
              </div>
              <h3 className={`${styles.sectionTitle} ${styles.padded}`}>place</h3>
              <div className={styles.place}>
                <span className={styles.placeLabel}>
                  <GeneralMapPin size={20} aria-hidden="true" />
                  <span>{details.place.label}</span>
                </span>
                {details.place.jumpInHref ? (
                  <JumpInCTA
                    href={details.place.jumpInHref}
                    size="sm"
                    aria-label={`Jump in to ${details.place.label}`}
                  />
                ) : null}
              </div>
              {details.people.length > 0 ? (
                <>
                  <h3 className={`${styles.sectionTitle} ${styles.padded}`}>people</h3>
                  <ul className={styles.people}>
                    {details.people.map((person, index) => (
                      <li key={`${person.href ?? person.name}-${index}`} className={styles.person}>
                        <PersonFace name={person.name} faceUrl={person.faceUrl} />
                        {person.href ? (
                          <Link to={person.href} className={styles.personName}>
                            {person.name}
                          </Link>
                        ) : (
                          <span className={styles.personName}>{person.name}</span>
                        )}
                        {person.isGuest ? <span className={styles.guest}>guest</span> : null}
                      </li>
                    ))}
                  </ul>
                </>
              ) : null}
            </section>
          </div>
        ) : null}
      </div>
    </Modal>
  )
}

/** A person's face, or their initial when there is none or it fails to load. Decoration: the name follows it. */
function PersonFace({ name, faceUrl }: { name: string; faceUrl?: string | undefined }) {
  const { broken, ref, onError } = useBrokenImage(faceUrl)
  const showImage = Boolean(faceUrl) && !broken
  return (
    <span className={styles.face} aria-hidden="true">
      {showImage ? (
        <img ref={ref} className={styles.faceImage} src={faceUrl} alt={name} onError={onError} />
      ) : (
        <span>{(Array.from(name)[0] ?? '?').toUpperCase()}</span>
      )}
    </span>
  )
}

/**
 * Previous or Next. At either end it stays mounted and is only marked unavailable (Button keeps an
 * `aria-disabled` button focusable and ignores its clicks): removing or disabling the button that
 * holds focus would drop focus out of the dialog.
 */
function StepButton({
  label,
  className,
  onStep,
  icon
}: {
  label: string
  className: string | undefined
  onStep?: () => void
  icon: ReactNode
}) {
  return (
    <Button
      iconOnly
      variant="dark"
      size="sm"
      className={`${styles.step} ${className ?? ''}`}
      aria-label={label}
      aria-disabled={!onStep}
      onClick={onStep}
    >
      {icon}
    </Button>
  )
}
