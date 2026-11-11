// TODO(figma-ds): replace with the Figma-generated photo viewer dialog from library/ui/components.
import {
  useEffect,
  useId,
  useRef,
  useState,
  type ComponentType,
  type KeyboardEvent,
  type ReactNode
} from 'react'
import { createPortal } from 'react-dom'
import {
  GeneralClose,
  GeneralInfoButton,
  GeneralLeftArrow,
  GeneralMapPin,
  GeneralRightArrow,
  MenusDownload
} from '../../../../foundations/icons'
import { Button } from '../../shared/Button/Button'
import { ProfileAvatar } from '../ProfileAvatar/ProfileAvatar'
import styles from './PhotoViewer.module.css'

/** The app's router link, so following it is a client navigation; a plain anchor otherwise. */
type PhotoViewerLinkProps = { to: string; className: string; children: ReactNode }

type PhotoViewerPerson = {
  name: string
  /** Their profile page; without one the name is plain text. */
  href?: string
  faceUrl?: string
  isGuest: boolean
}

type PhotoViewerDetails = {
  /** Already formatted; left out when the service's date cannot be read. */
  date?: string
  takenBy?: { name: string; href: string }
  place: { label: string; jumpInHref?: string }
  people: PhotoViewerPerson[]
}

type PhotoViewerProps = {
  imageUrl: string
  /** The image's alt text. */
  alt: string
  /** The dialog's accessible name, e.g. "Photo 3 of 24". */
  label: string
  details: PhotoViewerDetails
  /**
   * Where Download saves the full-size image from, and its file name. The node serves reel images
   * from the page's origin; a browser opens a cross-origin image instead of saving it.
   */
  download?: { href: string; fileName: string }
  /** Without one, its button is not shown and its arrow key does nothing. */
  onPrevious?: () => void
  onNext?: () => void
  onClose: () => void
  LinkComponent?: ComponentType<PhotoViewerLinkProps>
}

function AnchorLink({ to, ...props }: PhotoViewerLinkProps) {
  return <a href={to} {...props} />
}

const FOCUSABLE = 'a[href], button:not([disabled]), [tabindex]:not([tabindex="-1"])'

/**
 * A full-screen dialog with one photo and its information panel. It traps focus, closes on Escape
 * or a click on the backdrop, and moves between photos with the arrow keys and the Previous and
 * Next buttons; focus returns to where it was when it closes.
 */
function PhotoViewer(props: PhotoViewerProps) {
  // The dialog is portalled to <body>, which only exists in the browser.
  const [mounted, setMounted] = useState(false)
  useEffect(() => setMounted(true), [])
  return mounted ? createPortal(<ViewerDialog {...props} />, document.body) : null
}

function ViewerDialog({
  imageUrl,
  alt,
  label,
  details,
  download,
  onPrevious,
  onNext,
  onClose,
  LinkComponent = AnchorLink
}: PhotoViewerProps) {
  const dialogRef = useRef<HTMLDivElement>(null)
  const closeRef = useRef<HTMLButtonElement>(null)
  const [detailsVisible, setDetailsVisible] = useState(true)
  const titleId = useId()

  useEffect(() => {
    const opener = document.activeElement instanceof HTMLElement ? document.activeElement : null
    closeRef.current?.focus()
    const { overflow } = document.body.style
    document.body.style.overflow = 'hidden'
    return () => {
      document.body.style.overflow = overflow
      if (opener?.isConnected) opener.focus()
    }
  }, [])

  function onKeyDown(event: KeyboardEvent<HTMLDivElement>) {
    if (event.key === 'Escape') {
      event.stopPropagation()
      onClose()
    } else if (event.key === 'ArrowLeft' && onPrevious) {
      event.preventDefault()
      onPrevious()
    } else if (event.key === 'ArrowRight' && onNext) {
      event.preventDefault()
      onNext()
    } else if (event.key === 'Tab') {
      const focusable = [...(dialogRef.current?.querySelectorAll<HTMLElement>(FOCUSABLE) ?? [])]
      const first = focusable[0]
      const last = focusable[focusable.length - 1]
      if (!first || !last) return
      const active = document.activeElement
      if (event.shiftKey && (active === first || !dialogRef.current?.contains(active))) {
        event.preventDefault()
        last.focus()
      } else if (!event.shiftKey && (active === last || !dialogRef.current?.contains(active))) {
        event.preventDefault()
        first.focus()
      }
    }
  }

  return (
    <div className={styles.backdrop} onClick={onClose}>
      <div
        ref={dialogRef}
        className={styles.dialog}
        role="dialog"
        aria-modal="true"
        // A click inside on something that cannot take focus keeps focus in the dialog, so its keys still work.
        tabIndex={-1}
        aria-label={label}
        onKeyDown={onKeyDown}
        onClick={(event) => event.stopPropagation()}
      >
        <button
          ref={closeRef}
          type="button"
          className={styles.close}
          aria-label="Close"
          onClick={onClose}
        >
          <GeneralClose size={24} aria-hidden="true" />
        </button>
        <div className={detailsVisible ? `${styles.body} ${styles.withDetails}` : styles.body}>
          <div className={styles.media}>
            <div className={styles.toolbar}>
              {download ? (
                <a
                  className={styles.tool}
                  href={download.href}
                  download={download.fileName}
                  aria-label="Download photo"
                >
                  <MenusDownload size={22} aria-hidden="true" />
                </a>
              ) : null}
              <button
                type="button"
                className={detailsVisible ? `${styles.tool} ${styles.toolOn}` : styles.tool}
                aria-label="Information"
                aria-pressed={detailsVisible}
                onClick={() => setDetailsVisible((visible) => !visible)}
              >
                <GeneralInfoButton size={22} aria-hidden="true" />
              </button>
            </div>
            <img className={styles.image} src={imageUrl} alt={alt} />
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
            <section className={styles.details} aria-labelledby={titleId}>
              <div className={styles.header}>
                <h2 id={titleId} className={styles.sectionTitle}>
                  information
                </h2>
                {details.date ? <p className={styles.date}>{details.date}</p> : null}
                {details.takenBy ? (
                  <p className={styles.takenBy}>
                    Photo taken by{' '}
                    <LinkComponent to={details.takenBy.href} className={styles.link}>
                      {details.takenBy.name}
                    </LinkComponent>
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
                  <Button
                    href={details.place.jumpInHref}
                    size="sm"
                    aria-label={`Jump in to ${details.place.label}`}
                  >
                    jump in
                  </Button>
                ) : null}
              </div>
              {details.people.length > 0 ? (
                <>
                  <h3 className={`${styles.sectionTitle} ${styles.padded}`}>people</h3>
                  <ul className={styles.people}>
                    {details.people.map((person, index) => (
                      <li key={`${person.href ?? person.name}-${index}`} className={styles.person}>
                        <span aria-hidden="true">
                          <ProfileAvatar
                            imageUrl={person.faceUrl}
                            name={person.name}
                            color="var(--dcl-neutrals-pale-black)"
                            size={40}
                          />
                        </span>
                        {person.href ? (
                          <LinkComponent to={person.href} className={styles.personName}>
                            {person.name}
                          </LinkComponent>
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
          ) : null}
        </div>
      </div>
    </div>
  )
}

/**
 * Previous or Next. At either end it stays mounted and is only marked unavailable: removing or
 * disabling the button that holds focus would drop focus out of the dialog.
 */
function StepButton({
  label,
  className,
  onStep,
  icon
}: {
  label: string
  className: string
  onStep?: () => void
  icon: ReactNode
}) {
  return (
    <button
      type="button"
      className={`${styles.step} ${className}`}
      aria-label={label}
      aria-disabled={!onStep}
      onClick={onStep}
    >
      {icon}
    </button>
  )
}

export { PhotoViewer }
export type { PhotoViewerDetails, PhotoViewerLinkProps, PhotoViewerPerson, PhotoViewerProps }
