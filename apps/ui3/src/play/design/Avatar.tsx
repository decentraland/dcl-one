// Avatar — circular profile picture with a colored-initial fallback and optional
// status dot. Shared across chat bubbles, member rows, friends, and the profile.

import { useSyncExternalStore } from 'react'
import styles from './Avatar.module.css'

/** A failed picture is probed again after each of these: a fresh profile's snapshot URL is published
 *  before the server has rendered it (~10-15 s), and nothing says when it lands. */
const RETRY_DELAYS_MS = [2_000, 4_000, 8_000, 16_000, 32_000]

// Failed pictures, shared by every Avatar showing the same URL: one probe per retry, and a load
// (the probe's, or an <img> that was still in flight) puts the picture back up everywhere at once.
// Each failure is its own object, so a probe from an earlier failure of the same URL stands down.
interface Failure {
  attempt: number
}
const failedAttempts = new Map<string, Failure>()
const listeners = new Set<() => void>()

function subscribe(listener: () => void): () => void {
  listeners.add(listener)
  return () => listeners.delete(listener)
}

function notify(): void {
  for (const listener of listeners) listener()
}

function loaded(src: string): void {
  if (failedAttempts.delete(src)) notify()
}

function failed(src: string): void {
  if (failedAttempts.has(src)) return
  const failure = { attempt: 0 }
  failedAttempts.set(src, failure)
  notify()
  scheduleProbe(src, failure)
}

function scheduleProbe(src: string, failure: Failure): void {
  if (failure.attempt >= RETRY_DELAYS_MS.length) {
    // out of retries: Avatars showing the initial keep it, the next one to mount tries again
    failedAttempts.delete(src)
    return
  }
  setTimeout(() => {
    if (failedAttempts.get(src) !== failure) return
    const probe = new Image()
    probe.onload = () => loaded(src)
    probe.onerror = () => {
      if (failedAttempts.get(src) !== failure) return
      failure.attempt += 1
      scheduleProbe(src, failure)
    }
    probe.src = src
  }, RETRY_DELAYS_MS[failure.attempt])
}

type Status = 'online' | 'away' | 'offline'

interface AvatarProps {
  /** Face snapshot URL; falls back to the colored initial when absent or it fails. */
  src?: string
  /** Used for the initial fallback + alt text. */
  name: string
  /** Fallback background / ring tint. */
  color?: string
  /** Diameter in px (default 32). */
  size?: number
  status?: Status
  /** The picture inset in its colour disc under a thin ring, as on the sidebar rail. */
  framed?: boolean
  /** Status dot corner: bottom-right (default) or a smaller dot at the top-right (friends list). */
  dotPosition?: 'bottom' | 'top'
  className?: string
}

function initialOf(name: string): string {
  const m = name.replace(/^[@#]/, '').match(/[a-z0-9]/i)
  return (m ? m[0] : name[0] || '?').toUpperCase()
}

export function Avatar({
  src,
  name,
  color = 'var(--fill-4)',
  size = 32,
  status,
  framed = false,
  dotPosition = 'bottom',
  className = ''
}: AvatarProps): React.JSX.Element {
  const isFailed = useSyncExternalStore(subscribe, () => src != null && failedAttempts.has(src))
  const showImg = src && !isFailed
  const dot = Math.max(6, Math.round(size * (dotPosition === 'top' ? 0.15 : 0.18)))
  return (
    <span
      className={`${styles.root} ${framed ? styles.framed : ''} ${className}`.trim()}
      style={{ width: size, height: size, background: color, fontSize: Math.round(size * 0.42) }}
    >
      {showImg ? (
        <img className={styles.img} src={src} alt={name} onLoad={() => loaded(src)} onError={() => failed(src)} />
      ) : (
        initialOf(name)
      )}
      {status && (
        <span
          className={`${styles.dot} ${styles[status]} ${dotPosition === 'top' ? styles.dotTop : ''}`.trim()}
          style={{ width: dot, height: dot }}
        />
      )}
    </span>
  )
}
