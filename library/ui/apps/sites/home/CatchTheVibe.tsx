import { useCallback, useEffect, useId, useRef, useState } from 'react'
import { Button } from '../shared/Button/Button'
import {
  EditorPause,
  GeneralAudioOff,
  GeneralAudioOn,
  GeneralJumpIn,
  GeneralPlay
} from '../../../foundations/icons'
import avatarFace from '../assets/avatar_face.webp'
import glow from '../assets/ellipse_gradient.webp'
import { Carousel, type CarouselLabels } from '../shared/Carousel'
import styles from './CatchTheVibe.module.css'

type VibeItem = {
  id: string
  image: string
  video: string
  creator: { name: string; href?: string; avatarUrl?: string }
}

type VibeLabels = { play: string; pause: string; mute: string; unmute: string }

type CatchTheVibeProps = {
  title: string
  items: readonly VibeItem[]
  /** Accessible names; play and pause name the clip, e.g. "Play: AlanHowick". */
  labels: VibeLabels
  /** The phone and tablet carousel's controls. */
  carouselLabels: CarouselLabels
  /** The "Hang out now" call under the clips. */
  hangOut?: { label: string; onClick: () => void }
}

// Hover previews only where a pointer can hover and the visitor accepts motion.
const HOVER_PREVIEW = '(hover: hover) and (prefers-reduced-motion: no-preference)'
const REDUCED_MOTION = '(prefers-reduced-motion: reduce)'

/** "m:ss", as source's duration chip. */
function formatDuration(seconds: number): string {
  const whole = Math.floor(seconds)
  return `${Math.floor(whole / 60)}:${String(whole % 60).padStart(2, '0')}`
}

function VibeCard({
  item,
  labels,
  as: Root = 'li',
  enabled = true
}: {
  item: VibeItem
  labels: VibeLabels
  as?: 'li' | 'div'
  enabled?: boolean
}) {
  const video = useRef<HTMLVideoElement | null>(null)
  const playIntent = useRef(0)
  const wanted = useRef(false)
  const [playing, setPlaying] = useState(false)
  const [muted, setMuted] = useState(true)
  const [duration, setDuration] = useState<string | null>(null)

  // The chip shows the clip's length once its metadata is in, "0:00" until then, as source.
  useEffect(() => {
    const element = video.current
    if (!element) return
    const read = () => {
      if (Number.isFinite(element.duration) && element.duration > 0)
        setDuration(formatDuration(element.duration))
    }
    read()
    element.addEventListener('loadedmetadata', read)
    return () => element.removeEventListener('loadedmetadata', read)
  }, [])

  const stop = useCallback(() => {
    ++playIntent.current
    wanted.current = false
    const element = video.current
    if (element) {
      element.pause()
      element.currentTime = 0
      element.muted = true
    }
    setPlaying(false)
    setMuted(true)
  }, [])

  const play = useCallback(
    (withSound: boolean) => {
      const element = video.current
      if (!element || !enabled) return
      const intent = ++playIntent.current
      wanted.current = true
      const current = () => wanted.current && playIntent.current === intent
      element.muted = !withSound
      setMuted(!withSound)
      setPlaying(true)
      element.play()?.catch((error: unknown) => {
        if (!current()) return
        if (
          !withSound ||
          typeof error !== 'object' ||
          error === null ||
          !('name' in error) ||
          error.name !== 'NotAllowedError'
        ) {
          wanted.current = false
          setPlaying(false)
          return
        }
        element.muted = true
        setMuted(true)
        element.play()?.catch(() => {
          if (current()) {
            wanted.current = false
            setPlaying(false)
          }
        })
      })
    },
    [enabled]
  )

  useEffect(() => {
    if (!enabled) stop()
  }, [enabled, stop])
  useEffect(() => {
    const element = video.current
    return () => {
      ++playIntent.current
      wanted.current = false
      element?.pause()
    }
  }, [])

  // A clip that scrolls away stops, so nothing plays off screen.
  useEffect(() => {
    const element = video.current
    if (!element || typeof IntersectionObserver === 'undefined') return
    const observer = new IntersectionObserver(
      ([entry]) => {
        if (entry && !entry.isIntersecting && wanted.current) stop()
      },
      { threshold: 0.3 }
    )
    observer.observe(element)
    return () => observer.disconnect()
  }, [stop])

  // A preview the pointer started stops as soon as the visitor turns reduced motion on; a clip
  // played with the button keeps playing, since the visitor asked for it.
  const hoverStarted = useRef(false)
  useEffect(() => {
    const reduced = window.matchMedia(REDUCED_MOTION)
    const onChange = () => {
      if (reduced.matches && hoverStarted.current) {
        hoverStarted.current = false
        stop()
      }
    }
    reduced.addEventListener('change', onChange)
    return () => reduced.removeEventListener('change', onChange)
  }, [stop])

  const hoverPreview = () => window.matchMedia(HOVER_PREVIEW).matches

  return (
    <Root
      className={styles.card}
      onMouseEnter={() => {
        if (!enabled || !hoverPreview() || playing) return
        hoverStarted.current = true
        play(false)
      }}
      onMouseLeave={() => {
        if (!hoverStarted.current) return
        hoverStarted.current = false
        stop()
      }}
    >
      <div className={styles.media}>
        <video
          ref={video}
          className={styles.video}
          src={enabled ? item.video : undefined}
          poster={item.image}
          loop
          muted
          playsInline
          preload={enabled ? 'metadata' : 'none'}
          aria-hidden="true"
        />
        <div className={styles.controls}>
          {/* Stopped, the play control is source's duration chip: play mark and the clip length. */}
          <Button
            variant="quaternary"
            size="sm"
            aria-label={`${playing ? labels.pause : labels.play}: ${item.creator.name}`}
            aria-pressed={playing}
            onClick={() => {
              // From here the visitor owns the clip: leaving or reduced motion no longer stops it.
              hoverStarted.current = false
              if (playing) stop()
              else play(true)
            }}
          >
            {playing ? (
              <EditorPause size={16} aria-hidden="true" />
            ) : (
              <>
                <GeneralPlay size={16} aria-hidden="true" />
                <span className={styles.duration}>{duration ?? '0:00'}</span>
              </>
            )}
          </Button>
          {playing ? (
            <Button
              variant="quaternary"
              size="sm"
              aria-label={muted ? labels.unmute : labels.mute}
              onClick={() => {
                const element = video.current
                if (!element) return
                element.muted = !element.muted
                setMuted(element.muted)
              }}
            >
              {muted ? (
                <GeneralAudioOff size={16} aria-hidden="true" />
              ) : (
                <GeneralAudioOn size={16} aria-hidden="true" />
              )}
            </Button>
          ) : null}
        </div>
      </div>
      <div className={styles.footer}>
        {/* Source's stand-in face until the creator's profile picture is known. */}
        <img className={styles.avatar} src={item.creator.avatarUrl || avatarFace} alt="" />
        {item.creator.href ? (
          <a className={styles.creator} href={item.creator.href}>
            {item.creator.name}
          </a>
        ) : (
          <span className={styles.creator}>{item.creator.name}</span>
        )}
      </div>
    </Root>
  )
}

/**
 * "Catch the Vibe": creator clips that play on demand, previewing on hover where motion is welcome.
 * Desktop shows them in a row; narrower screens in a carousel, as source.
 */
function CatchTheVibe({ title, items, labels, carouselLabels, hangOut }: CatchTheVibeProps) {
  const titleId = useId()
  const [layout, setLayout] = useState<'wide' | 'narrow' | null>(null)
  useEffect(() => {
    const media = window.matchMedia('(min-width: 1024px)')
    const update = () => setLayout(media.matches ? 'wide' : 'narrow')
    update()
    media.addEventListener('change', update)
    return () => media.removeEventListener('change', update)
  }, [])
  return (
    <section className={styles.root} aria-labelledby={titleId}>
      <img className={styles.glow} src={glow} alt="" loading="lazy" />
      <h2 id={titleId} className={styles.title}>
        {title}
      </h2>
      <ul className={styles.cards}>
        {items.map((item) => (
          <VibeCard key={item.id} item={item} labels={labels} enabled={layout === 'wide'} />
        ))}
      </ul>
      {/* Below breakpoints.desktop the clips are source's carousel instead of a row. */}
      <Carousel
        className={styles.carousel}
        items={items}
        getKey={(item) => item.id}
        labelledBy={titleId}
        labels={carouselLabels}
        renderSlide={(item, peek) => (
          <VibeCard
            key={item.id}
            item={item}
            labels={labels}
            as="div"
            enabled={layout === 'narrow' && !peek}
          />
        )}
      />
      {hangOut ? (
        <Button onClick={hangOut.onClick}>
          {hangOut.label}
          <GeneralJumpIn size={32} aria-hidden="true" />
        </Button>
      ) : null}
    </section>
  )
}

export { CatchTheVibe }
export type { CatchTheVibeProps, VibeItem }
