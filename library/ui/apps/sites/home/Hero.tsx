import { useEffect, useState, type ReactNode } from 'react'
import heroDesktop from '../assets/hero_desktop.webp'
import heroMobile from '../assets/hero_mobile.webp'
import heroTablet from '../assets/hero_tablet.webp'
import videoMp4 from '../assets/video_landing.mp4'
import videoWebm from '../assets/video_landing.webm'
import {
  DownloadActions,
  StoreBadge,
  type DownloadActionsProps,
  type StoreBadgeProps
} from '../download/DownloadActions'
import styles from './Hero.module.css'

type HeroProps = {
  /** The page heading on a wide screen. */
  title: ReactNode
  /** What the background shows, for screen readers. */
  imageAlt: string
  /** Wide screens: the download actions. */
  actions?: DownloadActionsProps
  /** Narrow screens: their own heading, a subtitle and one store badge instead of the actions. Both
      layouts render and the viewport width picks one, as source does, so SSR markup is stable. */
  mobile?: { title: ReactNode; subtitle: ReactNode; store: StoreBadgeProps }
  /** Dialogs the actions open (the phone QR codes). */
  children?: ReactNode
}

const REDUCED_MOTION = '(prefers-reduced-motion: reduce)'
// breakpoints.desktop in foundations/layout: the ambient video only plays in the wide layout.
const WIDE = '(min-width: 1024px)'
// The video waits for an idle moment so it never competes with the hero image (the LCP).
const VIDEO_IDLE_TIMEOUT_MS = 4000

/** The home hero: background image, an ambient video on desktop, the title and the download CTAs. */
function Hero({ title, imageAlt, actions, mobile, children }: HeroProps) {
  const [playVideo, setPlayVideo] = useState(false)

  // Follows the motion preference and the viewport live: reduced motion or a narrow viewport cancels
  // a pending start and removes a playing video; turning back schedules the video again.
  useEffect(() => {
    const reduced = window.matchMedia(REDUCED_MOTION)
    const wide = window.matchMedia(WIDE)
    let cancel = () => {}
    const schedule = () => {
      cancel()
      cancel = () => {}
      if (reduced.matches || !wide.matches) {
        setPlayVideo(false)
        return
      }
      const start = () => setPlayVideo(true)
      if (typeof window.requestIdleCallback === 'function') {
        const id = window.requestIdleCallback(start, { timeout: VIDEO_IDLE_TIMEOUT_MS })
        cancel = () => window.cancelIdleCallback(id)
      } else {
        const timer = setTimeout(start, VIDEO_IDLE_TIMEOUT_MS)
        cancel = () => clearTimeout(timer)
      }
    }
    schedule()
    reduced.addEventListener('change', schedule)
    wide.addEventListener('change', schedule)
    return () => {
      cancel()
      reduced.removeEventListener('change', schedule)
      wide.removeEventListener('change', schedule)
    }
  }, [])

  return (
    <section className={styles.root}>
      <div className={styles.background}>
        <img
          className={styles.media}
          src={heroMobile}
          srcSet={`${heroMobile} 800w, ${heroTablet} 1200w, ${heroDesktop} 1920w`}
          sizes="100vw"
          alt={imageAlt}
          width={1920}
          height={1080}
          fetchPriority="high"
        />
        {playVideo ? (
          <video
            className={`${styles.media} ${styles.video}`}
            autoPlay
            loop
            muted
            playsInline
            preload="none"
            aria-hidden="true"
          >
            <source src={videoWebm} type="video/webm" />
            <source src={videoMp4} type="video/mp4" />
          </video>
        ) : null}
      </div>
      <div className={styles.shadeTop} aria-hidden="true" />
      <div className={styles.shadeBottom} aria-hidden="true" />

      <div className={styles.content}>
        <div className={mobile ? `${styles.layout} ${styles.wideOnly}` : styles.layout}>
          <h1 className={styles.title}>{title}</h1>
          {actions ? <DownloadActions {...actions} align="center" appearance="source" /> : null}
        </div>
        {mobile ? (
          <div className={`${styles.layout} ${styles.narrowOnly}`}>
            <h1 className={styles.title}>{mobile.title}</h1>
            <p className={styles.subtitle}>{mobile.subtitle}</p>
            <StoreBadge {...mobile.store} appearance="source" />
          </div>
        ) : null}
      </div>
      {children}
    </section>
  )
}

export { Hero }
export type { HeroProps }
