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
  GeneralCopy,
  GeneralInfoButton,
  GeneralImage,
  GeneralLeftArrow,
  GeneralMapPin,
  GeneralRightArrow,
  MenusDownload,
  SocialX
} from '../../../foundations/icons'

import styles from './PhotoViewer.module.css'
import { CardImage } from '../../../components/CardImage/CardImage'
import { useBrokenImage } from '../useBrokenImage'

type PhotoWearable = { id: string; label: string; image?: string; href?: string }

export type PhotoViewerPerson = {
  name: string
  /** Their profile page; without one the name is plain text. */
  href?: string
  faceUrl?: string
  isGuest: boolean
  wearables?: PhotoWearable[]
}

export type PhotoViewerDetails = {
  /** Already formatted; left out when the service's date cannot be read. */
  date?: string
  takenBy?: { name: string; href?: string }
  place: { label: string; href?: string; jumpInHref?: string }
  people: PhotoViewerPerson[]
}

export type PhotoViewerActions = {
  shareHref?: string
  onCopy?: () => void
  onDownload?: () => void
  downloading?: boolean
  announcement?: string
  copyFallbackHref?: string
  announcementVersion?: number
}

/** What the viewer reports of its controls, for a host that counts them. Each runs before the control's own action. */
export type PhotoViewerEvents = {
  onShare?: () => void
  /** The details panel was shown (`true`) or hidden. */
  onDetailsToggle?: (visible: boolean) => void
  onTakenByClick?: () => void
  onPlaceClick?: () => void
  onJumpInClick?: () => void
  /** A person's profile link, by their index in `details.people`. */
  onPersonClick?: (person: number) => void
  /** A person's wearables list was opened (`true`) or closed. */
  onWearablesToggle?: (person: number, open: boolean) => void
  /** A wearable's link, by the person's index and the wearable's index in their list. */
  onWearableClick?: (person: number, wearable: number) => void
}

export type PhotoViewerProps = {
  imageUrl: string
  /** The image's alt text. */
  alt: string
  /** The page heading or dialog's accessible name. */
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
  actions?: PhotoViewerActions
  events?: PhotoViewerEvents
  /** Shows the first person's wearables list open at first, as decentraland.org's Reels page did. */
  firstWearablesOpen?: boolean
  linkComponent: ComponentType<PlaceCardLinkProps>
} & ({ variant: 'page'; onClose?: never } | { variant: 'dialog'; onClose: () => void })

/** Profile photo and its information panel inside the shared gallery modal. */
export function PhotoViewer({
  variant,
  imageUrl,
  alt,
  label,
  details,
  download,
  actions,
  events,
  firstWearablesOpen = false,
  onPrevious,
  onNext,
  onClose,
  linkComponent: Link
}: PhotoViewerProps) {
  const closeRef = useRef<HTMLButtonElement>(null)
  const [detailsVisible, setDetailsVisible] = useState(true)
  const titleId = useId()
  const detailsId = useId()
  const { broken, ref, onError } = useBrokenImage(imageUrl)

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

  const content = (
    <div className={[styles.body, detailsVisible && styles.withDetails].filter(Boolean).join(' ')}>
      <div className={variant === 'page' ? `${styles.media} ${styles.pageMedia}` : styles.media}>
        <div className={styles.toolbar}>
          {actions?.shareHref ? (
            <Button
              iconOnly
              variant="dark"
              size="sm"
              href={actions.shareHref}
              target="_blank"
              rel="noopener noreferrer"
              aria-label="Share to X"
              onClick={events?.onShare}
            >
              <SocialX size={22} aria-hidden="true" />
            </Button>
          ) : null}
          {actions?.onCopy ? (
            <Button
              iconOnly
              variant="dark"
              size="sm"
              aria-label="Copy link"
              onClick={actions.onCopy}
            >
              <GeneralCopy size={22} aria-hidden="true" />
            </Button>
          ) : null}
          {actions?.onDownload ? (
            <Button
              iconOnly
              variant="dark"
              size="sm"
              aria-label="Download photo"
              aria-disabled={actions.downloading || broken}
              onClick={actions.onDownload}
            >
              <MenusDownload size={22} aria-hidden="true" />
            </Button>
          ) : download ? (
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
            aria-expanded={detailsVisible}
            aria-controls={detailsId}
            onClick={() => {
              const next = !detailsVisible
              setDetailsVisible(next)
              events?.onDetailsToggle?.(next)
            }}
          >
            <GeneralInfoButton size={22} aria-hidden="true" />
          </Button>
        </div>
        {actions ? (
          <div
            className={
              actions.announcement
                ? `${styles.announcement} ${styles.announcementVisible}`
                : styles.announcement
            }
          >
            <span role="status">
              {actions.announcement ? (
                <span key={actions.announcementVersion}>{actions.announcement}</span>
              ) : null}
            </span>
            {actions.copyFallbackHref ? (
              <code className={styles.copyFallback}>{actions.copyFallbackHref}</code>
            ) : null}
          </div>
        ) : null}
        {broken ? (
          <div className={styles.photoUnavailable} role="status">
            <GeneralImage size={32} aria-hidden="true" />
            Photo could not be loaded.
          </div>
        ) : (
          <img
            ref={ref}
            onError={onError}
            className={variant === 'page' ? `${styles.image} ${styles.pageImage}` : styles.image}
            src={imageUrl}
            alt={alt}
          />
        )}
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
      <div
        id={detailsId}
        hidden={!detailsVisible}
        className={
          detailsVisible
            ? variant === 'page'
              ? `${styles.details} ${styles.pageDetails}`
              : styles.details
            : undefined
        }
      >
        {detailsVisible ? (
          <section
            className={
              variant === 'page'
                ? `${styles.detailsScroll} ${styles.pageDetailsScroll}`
                : styles.detailsScroll
            }
            aria-labelledby={titleId}
            tabIndex={0}
          >
            <div className={styles.header}>
              <h2 id={titleId} className={styles.sectionTitle}>
                information
              </h2>
              {details.date ? <p className={styles.date}>{details.date}</p> : null}
              {details.takenBy ? (
                <p className={styles.takenBy}>
                  Photo taken by{' '}
                  {details.takenBy.href ? (
                    <Link
                      to={details.takenBy.href}
                      className={styles.link}
                      onClick={events?.onTakenByClick}
                    >
                      {details.takenBy.name}
                    </Link>
                  ) : (
                    details.takenBy.name
                  )}
                </p>
              ) : null}
            </div>
            <h3 className={`${styles.sectionTitle} ${styles.padded}`}>place</h3>
            <div className={styles.place}>
              <span className={styles.placeLabel}>
                <GeneralMapPin size={20} aria-hidden="true" />
                {details.place.href ? (
                  <Link
                    to={details.place.href}
                    className={styles.link}
                    onClick={events?.onPlaceClick}
                  >
                    {details.place.label}
                  </Link>
                ) : (
                  <span>{details.place.label}</span>
                )}
              </span>
              {details.place.jumpInHref ? (
                <JumpInCTA
                  href={details.place.jumpInHref}
                  size="sm"
                  aria-label={`Jump in to ${details.place.label}`}
                  onClick={events?.onJumpInClick}
                />
              ) : null}
            </div>
            {details.people.length > 0 ? (
              <>
                <h3 className={`${styles.sectionTitle} ${styles.padded}`}>people</h3>
                <ul className={styles.people}>
                  {details.people.map((person, index) => (
                    <li key={`${person.href ?? person.name}-${index}`} className={styles.person}>
                      <div className={styles.personIdentity}>
                        <PersonFace name={person.name} faceUrl={person.faceUrl} />
                        {person.href ? (
                          <Link
                            to={person.href}
                            className={styles.personName}
                            onClick={() => events?.onPersonClick?.(index)}
                          >
                            {person.name}
                          </Link>
                        ) : (
                          <span className={styles.personName}>{person.name}</span>
                        )}
                        {person.isGuest ? <span className={styles.guest}>guest</span> : null}
                      </div>
                      {person.wearables ? (
                        <details open={(firstWearablesOpen && index === 0) || undefined}>
                          <summary
                            className={styles.wearableSummary}
                            onClick={(event) => {
                              // The click toggles the list after this runs, so it is about to flip.
                              const open = !event.currentTarget.parentElement?.hasAttribute('open')
                              events?.onWearablesToggle?.(index, open)
                            }}
                          >
                            Collectible wearables
                          </summary>
                          <ul className={styles.wearables}>
                            {person.wearables.map((item, wearable) => (
                              <li key={item.id} className={styles.wearable}>
                                {item.image ? (
                                  <CardImage
                                    src={item.image}
                                    alt=""
                                    className={styles.wearableImage}
                                  />
                                ) : null}
                                {item.href ? (
                                  <Link
                                    to={item.href}
                                    className={styles.link}
                                    onClick={() => events?.onWearableClick?.(index, wearable)}
                                  >
                                    {item.label}
                                  </Link>
                                ) : (
                                  item.label
                                )}
                              </li>
                            ))}
                          </ul>
                        </details>
                      ) : null}
                    </li>
                  ))}
                </ul>
              </>
            ) : null}
          </section>
        ) : null}
      </div>
    </div>
  )

  if (variant === 'page') return content
  return (
    <Modal
      layout="gallery"
      title={label}
      closeButton={false}
      initialFocusRef={closeRef}
      onClose={onClose}
      onKeyDown={onKeyDown}
      onMouseDown={(event) => {
        // Clicking blank gallery space keeps the current keyboard control focused.
        const control =
          event.target instanceof Element
            ? event.target.closest('button, a[href], [tabindex], code')
            : null
        if (!event.currentTarget.contains(control)) event.preventDefault()
      }}
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
      {content}
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
