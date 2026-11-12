import { useEffect, useRef, useState, type MouseEvent } from 'react'
import { GeneralClose, GeneralCopy, GeneralJumpIn, GeneralMapPin } from '../../../foundations/icons'
import dclLogo from '../assets/dcl-logo.svg'
import { Dialog } from './Dialog'
import type { PlaceCardCreator } from './cards/types'
import styles from './JumpInModal.module.css'

type JumpInModalPlace = {
  title: string
  image: string | null
  description?: string
  creator?: PlaceCardCreator
  coords?: string
  jumpHref?: string
  /** The canonical place page, absolute: the address bar still reads /places while the modal is open. */
  shareUrl: string
}

type JumpInModalLabels = {
  close: string
  jumpIn: string
  copyLink: string
  copied: string
  by: string
  whatToExpect: string
}

type JumpInModalProps = {
  place: JumpInModalPlace
  labels: JumpInModalLabels
  onClose: () => void
  /** Call `preventDefault()` to launch another way than following the link. */
  onJumpIn?: (event: MouseEvent<HTMLAnchorElement>) => void
}

const COPIED_MS = 2000

/** Source's JUMP IN modal for an empty scene: cover, creator, location, Jump In, copy link and the description. */
function JumpInModal({ place, labels, onClose, onJumpIn }: JumpInModalProps) {
  const [copied, setCopied] = useState(false)
  const timer = useRef<ReturnType<typeof setTimeout> | undefined>(undefined)
  useEffect(() => () => clearTimeout(timer.current), [])

  const copy = () => {
    // An insecure context or a denied permission shows nothing rather than claiming a copy.
    navigator.clipboard
      ?.writeText(place.shareUrl)
      .then(() => {
        setCopied(true)
        clearTimeout(timer.current)
        timer.current = setTimeout(() => setCopied(false), COPIED_MS)
      })
      .catch(() => undefined)
  }

  const avatar = place.creator?.foundation ? dclLogo : place.creator?.avatarUrl
  return (
    <Dialog label={place.title} onClose={onClose} className={styles.modal}>
      <button type="button" className={styles.close} aria-label={labels.close} onClick={onClose}>
        <GeneralClose size={24} aria-hidden="true" />
      </button>
      <div className={styles.heroWrap}>
        <div className={styles.hero}>
          {place.image ? <img className={styles.cover} src={place.image} alt="" /> : null}
          <div className={styles.fade} />
        </div>
        <div className={styles.heroText}>
          <h2 className={styles.title}>{place.title}</h2>
          <div className={styles.meta}>
            {place.creator ? (
              <span className={styles.creatorRow}>
                {avatar ? <img className={styles.avatar} src={avatar} alt="" /> : null}
                <span className={styles.by}>
                  {labels.by} <span className={styles.creator}>{place.creator.name}</span>
                </span>
              </span>
            ) : null}
            {place.coords ? (
              <span className={styles.location}>
                <GeneralMapPin size={16} aria-hidden="true" />
                {place.coords}
              </span>
            ) : null}
          </div>
          <div className={styles.ctaRow}>
            {place.jumpHref ? (
              <a className={styles.jumpIn} href={place.jumpHref} onClick={onJumpIn}>
                {labels.jumpIn}
                <GeneralJumpIn size={24} aria-hidden="true" />
              </a>
            ) : null}
            <span className={styles.copyWrap}>
              <button
                type="button"
                className={styles.copy}
                aria-label={labels.copyLink}
                onClick={copy}
              >
                <GeneralCopy size={20} aria-hidden="true" />
              </button>
              <span role="status" className={copied ? styles.copied : styles.visuallyHidden}>
                {copied ? labels.copied : ''}
              </span>
            </span>
          </div>
        </div>
      </div>
      {place.description ? (
        <div className={styles.about}>
          <h3 className={styles.aboutLabel}>{labels.whatToExpect}</h3>
          <p className={styles.aboutText}>{place.description}</p>
        </div>
      ) : null}
    </Dialog>
  )
}

export { JumpInModal }
export type { JumpInModalLabels, JumpInModalPlace, JumpInModalProps }
