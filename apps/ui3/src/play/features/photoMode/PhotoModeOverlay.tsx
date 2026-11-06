// Photo mode's HUD: the screen darkened outside the photo frame, the rule-of-thirds grid, the
// Gallery / Shoot / camera-controls row under the frame, the close button, the camera controls
// panel, the storage-full notice, and the capture effect (a white flash, then the photo flying
// into the Gallery button). The engine owns the camera and the capture; this reads its stream
// and sends it the buttons. Sizes are reference pixels (1080p canvas) scaled by --ui-scale.
import { useCallback, useEffect, useLayoutEffect, useRef, useState } from 'react'
import { Close, MaskIcon, MouseIcon, ScrollIcon } from '../../design'
import { keyHintFor, useBindingsSnapshot, type BindingsSnapshot } from '../../lib/bindingLabels'
import { registerCancelLayer } from '../../lib/cancelLayers'
import type { PhotoModeState } from '../session/useEngineSession'
import { normalizedFrameSize } from './frame'
import shootArt from '../../assets/photo-mode/shoot.png'
import keyboardArt from '../../assets/photo-mode/keyboard.png'
import galleryArt from '../../assets/photo-mode/gallery.png'
import styles from './PhotoModeOverlay.module.css'

const NO_STORAGE_TEXT =
  "You can't take more photos because you have reached the Camera Reel storage limit. To make room we recommend you to download and then delete some items."

const SPLASH_MS = 500
const PAUSE_MS = 100
const FLY_MS = 500

function key(snap: BindingsSnapshot, action: string, fallback: string): string {
  return keyHintFor(snap, action) ?? fallback
}

type Row = { keys?: string[]; icon?: 'mouse' | 'scroll'; text: string }

function controlRows(snap: BindingsSnapshot): Row[] {
  return [
    { icon: 'mouse', text: 'Click and Drag to Move' },
    { icon: 'scroll', text: 'Zoom In/Out' },
    { keys: [key(snap, 'IaForward', 'W'), key(snap, 'IaBackward', 'S')], text: 'Move Forward/Backward' },
    { keys: [key(snap, 'IaLeft', 'A'), key(snap, 'IaRight', 'D')], text: 'Pan Left/Right' },
    { keys: [key(snap, 'PhotoCameraUp', 'R'), key(snap, 'PhotoCameraDown', 'F')], text: 'Pan Down/Up' },
    { keys: [key(snap, 'PhotoTiltLeft', 'Q'), key(snap, 'PhotoTiltRight', 'E')], text: 'Rotate Left/Right' },
    { keys: [key(snap, 'IaModifier', 'Shift')], text: 'Increase Move/Pan Speed' },
    { keys: [key(snap, 'HideUi', 'U')], text: 'Show/Hide UI' },
    { keys: [key(snap, 'HideNames', 'N')], text: 'Show/Hide Name tags' },
    { keys: [key(snap, 'PhotoMode', 'C')], text: 'Open/Exit Camera' }
  ]
}

function uiScale(): number {
  const value = parseFloat(getComputedStyle(document.documentElement).getPropertyValue('--ui-scale'))
  return Number.isFinite(value) && value > 0 ? value : 1
}

interface Props {
  photoMode: PhotoModeState
  onOpenGallery: () => void
}

export function PhotoModeOverlay({ photoMode, onOpenGallery }: Props): React.JSX.Element {
  const snap = useBindingsSnapshot()
  const state = photoMode.snapshot
  const [viewport, setViewport] = useState(() => ({ w: window.innerWidth, h: window.innerHeight, s: uiScale() }))
  useEffect(() => {
    const onResize = (): void => setViewport({ w: window.innerWidth, h: window.innerHeight, s: uiScale() })
    window.addEventListener('resize', onResize)
    return () => window.removeEventListener('resize', onResize)
  }, [])
  const frame = normalizedFrameSize(viewport.w / Math.max(1, viewport.h))
  const frameRect = {
    left: ((1 - frame.x) / 2) * viewport.w,
    top: ((1 - frame.y) / 2) * viewport.h,
    width: frame.x * viewport.w,
    height: frame.y * viewport.h
  }
  const s = viewport.s

  // Escape closes photo mode engine-side; the layer keeps the HUD's own cancel dispatch from
  // closing something underneath on the same press.
  useEffect(() => registerCancelLayer(() => photoMode.set(false)), [photoMode])

  const [controlsOpen, setControlsOpen] = useState(false)
  const controlsRef = useRef<HTMLDivElement>(null)
  useEffect(() => {
    if (!controlsOpen) return
    const onDown = (e: MouseEvent): void => {
      if (!controlsRef.current?.contains(e.target as Node)) setControlsOpen(false)
    }
    document.addEventListener('mousedown', onDown, true)
    return () => document.removeEventListener('mousedown', onDown, true)
  }, [controlsOpen])

  const galleryIcon = useRef<HTMLSpanElement>(null)
  const [effect, setEffect] = useState<{ seq: number; preview: string } | null>(null)
  const [fly, setFly] = useState<React.CSSProperties | null>(null)
  const seenSeq = useRef(0)
  useEffect(() => {
    if (state == null || state.event !== 'captured' || state.eventSeq === seenSeq.current) return
    seenSeq.current = state.eventSeq
    setEffect({ seq: state.eventSeq, preview: state.previewJpegBase64 })
    setFly(null)
  }, [state])
  useLayoutEffect(() => {
    if (effect == null) return
    const target = galleryIcon.current?.getBoundingClientRect()
    const flyTimer = window.setTimeout(() => {
      if (target != null) setFly({ left: target.left, top: target.top, width: target.width, height: target.height })
    }, SPLASH_MS + PAUSE_MS)
    const endTimer = window.setTimeout(() => {
      setEffect(null)
      setFly(null)
    }, SPLASH_MS + PAUSE_MS + FLY_MS)
    return () => {
      window.clearTimeout(flyTimer)
      window.clearTimeout(endTimer)
    }
  }, [effect])

  const close = useCallback(() => photoMode.set(false), [photoMode])
  const storageFull = state != null && state.storageLoaded && state.storageCurrent >= state.storageMax
  const hidden = state?.hudHidden ?? false
  const bottom = frameRect.top + frameRect.height

  return (
    <section
      className={styles.root}
      aria-label="Photo mode"
      data-hidden={hidden || undefined}
      style={{ '--s': s } as React.CSSProperties}
    >
      {!hidden && (
        <div className={styles.hud}>
          <div className={styles.mask} style={{ left: 0, top: 0, right: 0, height: frameRect.top }} />
          <div className={styles.mask} style={{ left: 0, right: 0, top: bottom, bottom: 0 }} />
          <div className={styles.mask} style={{ left: 0, top: frameRect.top, width: frameRect.left, height: frameRect.height }} />
          <div className={styles.mask} style={{ left: frameRect.left + frameRect.width, right: 0, top: frameRect.top, height: frameRect.height }} />
          <div className={styles.grid} style={frameRect} aria-hidden="true">
            <div className={styles.gridV} style={{ left: '33.333%' }} />
            <div className={styles.gridV} style={{ left: '66.667%' }} />
            <div className={styles.gridH} style={{ top: '33.333%' }} />
            <div className={styles.gridH} style={{ top: '66.667%' }} />
          </div>

          <button type="button" className={styles.close} aria-label="Close camera" onClick={close}>
            <Close size={12 * s} strokeWidth={2} />
          </button>

          <div className={styles.row} style={{ left: frameRect.left, width: frameRect.width, top: bottom }}>
            <button type="button" className={`${styles.button} ${styles.gallery}`} aria-label="Gallery" onClick={onOpenGallery}>
              <span ref={galleryIcon} className={styles.galleryIcon}>
                <MaskIcon src={galleryArt} size={28 * s} />
              </span>
              <span className={styles.label}>
                Gallery <span className={styles.key}>[{key(snap, 'Gallery', 'G')}]</span>
              </span>
            </button>
            {storageFull ? (
              <div className={styles.notice} role="alert">
                <span className={styles.noticeText}>{NO_STORAGE_TEXT}</span>
              </div>
            ) : (
              <button
                type="button"
                className={`${styles.button} ${styles.shoot}`}
                aria-label="Shoot"
                disabled={state?.phase === 'capturing'}
                onClick={photoMode.take}
              >
                <MaskIcon src={shootArt} size={25 * s} />
                <span className={styles.shootText}>
                  <span className={styles.label}>Shoot</span>
                  <span className={styles.shootKey}>[{key(snap, 'TakePhoto', 'Space')}]</span>
                </span>
              </button>
            )}
            <div ref={controlsRef} className={styles.controlsAnchor}>
              <button
                type="button"
                className={`${styles.button} ${styles.controls}`}
                aria-expanded={controlsOpen}
                onClick={() => setControlsOpen((open) => !open)}
              >
                <MaskIcon src={keyboardArt} size={24 * s} />
                <span className={styles.controlsLabel}>camera controls</span>
              </button>
            </div>
          </div>

          {controlsOpen && (
            <div className={styles.panel} role="dialog" aria-label="Camera controls">
              <h2 className={styles.panelTitle}>Camera Controls</h2>
              <ul className={styles.rows}>
                {controlRows(snap).map((row) => (
                  <li key={row.text} className={styles.hint}>
                    <span className={styles.hintKeys}>
                      {row.icon === 'mouse' && <MouseIcon size={20 * s} />}
                      {row.icon === 'scroll' && <ScrollIcon size={20 * s} />}
                      {row.keys?.map((k) => (
                        <span key={k} className={styles.keyCap}>
                          {k}
                        </span>
                      ))}
                    </span>
                    <span className={styles.hintText}>{row.text}</span>
                  </li>
                ))}
              </ul>
            </div>
          )}
        </div>
      )}
      {effect != null && (
        <>
          <div key={`flash-${effect.seq}`} className={styles.flash} data-testid="photo-flash" />
          {effect.preview !== '' && (
            <img
              key={`fly-${effect.seq}`}
              className={styles.fly}
              alt=""
              src={`data:image/jpeg;base64,${effect.preview}`}
              style={fly ?? frameRect}
            />
          )}
        </>
      )}
    </section>
  )
}
