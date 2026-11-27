import {
  useState,
  type ComponentType,
  type AnchorHTMLAttributes,
  type Ref,
  type ReactNode
} from 'react'
import PreviewFrame, { UNITY_LOAD_TIMEOUT_MS, type PreviewFrameOptions } from './PreviewFrame'
import styles from './LivePromo.module.css'

// Upstream's close-up framing, passed as the preview's own zoom so it survives the camera framing the avatar again as
// its items stream in. Its wheel range is widened to match; the tile takes no pointer input, so it only sets the range.
const ZOOM = 100
const WHEEL_ZOOM = 2.5

/**
 * The tile's avatar, as upstream draws it: Babylon whatever the Unity flag says (the Bevy avatar preview where its own
 * flag is on, in upstream's marketplace mode), a default body in the promo's items doing
 * PreviewEmote.FASHION (an emote item among the urns, listed first, plays instead). The urns are mainnet's, so the iframe
 * reads mainnet's peer and marketplace whatever the Shop's environment (previewBase).
 */
function previewOptions(urns: string[], bevy: boolean): PreviewFrameOptions {
  const options: PreviewFrameOptions = {
    urns,
    profile: 'default',
    type: 'avatar',
    emote: 'fashion',
    zoom: ZOOM,
    wheelZoom: WHEEL_ZOOM,
    disableBackground: true
  }
  return bevy ? { ...options, bevy: true, unityMode: 'marketplace' } : options
}

export type LivePromoProps = {
  Link: ComponentType<
    AnchorHTMLAttributes<HTMLAnchorElement> & { to: string; ref?: Ref<HTMLAnchorElement> }
  >
  backdrop: ReactNode
  linkRef: Ref<HTMLAnchorElement>
  onScreen: boolean
  allowed: boolean
  bevyFlag: boolean | null
  to: string
  urns: string[]
  title: string
  cta: string
  ariaLabel: string
  fallback: string
  fallbackAlt: string
  testId?: string
}

export function LivePromo({
  Link,
  backdrop,
  linkRef,
  onScreen,
  allowed,
  bevyFlag,
  to,
  urns,
  title,
  cta,
  ariaLabel,
  fallback,
  fallbackAlt,
  testId
}: LivePromoProps) {
  const [ready, setReady] = useState(false)
  const [failed, setFailed] = useState(false)
  // Bevy failing gives way to Babylon for the rest of the tile's life.
  const [bevyFailed, setBevyFailed] = useState(false)
  const bevy = bevyFlag === true && !bevyFailed

  const live = allowed && !failed
  // The iframe lives only while the tile is on screen: off screen its render loop runs at full speed for nobody. A hidden
  // tab keeps it, since the browser all but stops its frames there.
  // Mounted once the flag has said which renderer, so the tile never loads one and then the other.
  const mounted = live && onScreen && bevyFlag !== null
  // A new iframe has drawn nothing: back on screen, the picture stays until it has.
  if (!mounted && ready) setReady(false)
  const show = mounted && ready

  return (
    <Link
      ref={linkRef}
      className={styles.root}
      to={to}
      aria-label={ariaLabel}
      data-testid={testId}
      data-crest={show || undefined}
    >
      <span className={styles.card}>
        {/* The fitting room's animated backdrop under the avatar, as upstream's tile draws it: only while the tile is
            on screen, with larger drifting tiles (tiling 1, against the fitting room's 1.66) for the smaller card. */}
        {mounted ? backdrop : null}
        <span className={styles.inner} data-ready={show || undefined}>
          <span className={styles.title}>{title}</span>
          <span className={styles.cta}>{cta}</span>
        </span>
        <img
          className={styles.fallback}
          src={fallback}
          alt={fallbackAlt}
          data-hidden={show || undefined}
          decoding="async"
        />
      </span>
      {mounted ? (
        <span className={styles.avatar} data-ready={show || undefined} aria-hidden>
          <PreviewFrame
            key={bevy ? 'bevy' : 'babylon'}
            options={previewOptions(urns, bevy)}
            ambient
            loadTimeoutMs={bevy ? UNITY_LOAD_TIMEOUT_MS : undefined}
            onLoad={() => setReady(true)}
            // A fresh iframe has drawn nothing yet (Bevy booting again): the picture covers it until it has.
            onReplaced={() => setReady(false)}
            onError={() => {
              setReady(false)
              if (bevy) setBevyFailed(true)
              else setFailed(true)
            }}
          />
        </span>
      ) : null}
    </Link>
  )
}
