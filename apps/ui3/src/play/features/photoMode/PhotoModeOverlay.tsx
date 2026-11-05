// Photo mode's HUD: the photo frame with its rule-of-thirds grid, the shoot / gallery / close
// controls, the camera controls hint, the storage-full notice, and the capture effect (a white
// flash, then the photo flying to the gallery button). The engine owns the camera and the
// capture; this reads its stream and sends it the buttons.
import { useEffect, useLayoutEffect, useRef, useState } from 'react'
import { Button, Close, ControlButton, HintsButton, KeyCap, Panel, type Hint } from '../../design'
import { keyHintFor, useBindingsSnapshot, type BindingsSnapshot } from '../../lib/bindingLabels'
import { registerCancelLayer } from '../../lib/cancelLayers'
import type { PhotoModeState } from '../session/useEngineSession'
import { normalizedFrameSize } from './frame'
import styles from './PhotoModeOverlay.module.css'

const NO_STORAGE_TEXT =
  "You can't take more photos because you have reached the Camera Reel storage limit. To make room we recommend you to download and then delete some items."

function key(snap: BindingsSnapshot, action: string, fallback: string): string {
  return keyHintFor(snap, action) ?? fallback
}

function controlHints(snap: BindingsSnapshot): Hint[] {
  const cap = (label: string): React.ReactNode => <KeyCap label={label} />
  return [
    { icon: cap(key(snap, 'PhotoMode', 'C')), text: 'Open / exit camera' },
    { icon: cap('W A S D'), text: 'Move' },
    { icon: cap(`${key(snap, 'PhotoCameraUp', 'R')} ${key(snap, 'PhotoCameraDown', 'F')}`), text: 'Move up / down' },
    { icon: cap('Mouse'), text: 'Rotate' },
    { icon: cap(`${key(snap, 'PhotoTiltLeft', 'Q')} ${key(snap, 'PhotoTiltRight', 'E')}`), text: 'Tilt left / right' },
    { icon: cap('Wheel'), text: 'Zoom in / out' },
    { icon: cap(key(snap, 'IaModifier', 'Shift')), text: 'Increase move speed' },
    { icon: cap('Drag'), text: 'Click and drag to look' },
    { icon: cap(key(snap, 'HideUi', 'U')), text: 'Show / hide UI' },
    { icon: cap(key(snap, 'HideNames', 'N')), text: 'Show / hide name tags' },
    { icon: cap(key(snap, 'TakePhoto', 'Space')), text: 'Take photo' }
  ]
}

interface Props {
  photoMode: PhotoModeState
  onOpenGallery: () => void
}

export function PhotoModeOverlay({ photoMode, onOpenGallery }: Props): React.JSX.Element {
  const snap = useBindingsSnapshot()
  const state = photoMode.snapshot
  const [viewport, setViewport] = useState({ w: window.innerWidth, h: window.innerHeight })
  useEffect(() => {
    const onResize = (): void => setViewport({ w: window.innerWidth, h: window.innerHeight })
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

  // Escape closes photo mode engine-side; registering a layer keeps the HUD's own cancel
  // dispatch from closing something underneath on the same press.
  useEffect(() => registerCancelLayer(() => photoMode.set(false)), [photoMode])

  // the capture effect: a flash, a pause, then the preview flies to the gallery button
  const galleryButton = useRef<HTMLSpanElement>(null)
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
    const target = galleryButton.current?.getBoundingClientRect()
    const flyTimer = window.setTimeout(() => {
      if (target == null) return
      setFly({
        left: target.left,
        top: target.top,
        width: target.width,
        height: target.height,
        opacity: 0.2
      })
    }, 500 + 100)
    const endTimer = window.setTimeout(() => {
      setEffect(null)
      setFly(null)
    }, 500 + 100 + 500)
    return () => {
      window.clearTimeout(flyTimer)
      window.clearTimeout(endTimer)
    }
  }, [effect])

  const storageFull = state != null && state.storageLoaded && state.storageCurrent >= state.storageMax
  const hidden = state?.hudHidden ?? false

  return (
    <section className={styles.root} aria-label="Photo mode" data-hidden={hidden || undefined}>
      {!hidden && (
        <>
          <div className={styles.mask} style={{ left: 0, top: 0, right: 0, height: frameRect.top }} />
          <div className={styles.mask} style={{ left: 0, right: 0, top: frameRect.top + frameRect.height, bottom: 0 }} />
          <div className={styles.mask} style={{ left: 0, top: frameRect.top, width: frameRect.left, height: frameRect.height }} />
          <div
            className={styles.mask}
            style={{ left: frameRect.left + frameRect.width, right: 0, top: frameRect.top, height: frameRect.height }}
          />
          <div className={styles.frame} style={frameRect} aria-hidden="true">
            <div className={styles.gridV} style={{ left: '33.333%' }} />
            <div className={styles.gridV} style={{ left: '66.667%' }} />
            <div className={styles.gridH} style={{ top: '33.333%' }} />
            <div className={styles.gridH} style={{ top: '66.667%' }} />
          </div>
          <div className={styles.topBar}>
            <h1 className={styles.title}>Camera</h1>
            <ControlButton aria-label="Close camera" onClick={() => photoMode.set(false)}>
              <Close />
            </ControlButton>
          </div>
          <div className={styles.bottomBar}>
            <HintsButton hints={controlHints(snap)} label="Camera controls" />
            <Button
              variant="primary"
              size="lg"
              className={styles.shoot}
              disabled={storageFull || state?.phase === 'capturing'}
              onClick={photoMode.take}
            >
              Shoot <KeyCap label={key(snap, 'TakePhoto', 'Space')} />
            </Button>
            <span ref={galleryButton} className={styles.galleryAnchor}>
              <Button variant="secondary" size="lg" onClick={onOpenGallery}>
                Gallery <KeyCap label={key(snap, 'Gallery', 'G')} />
              </Button>
            </span>
          </div>
          {storageFull && (
            <Panel blur className={styles.notice} role="alert">
              {NO_STORAGE_TEXT}
            </Panel>
          )}
        </>
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
              style={fly ?? { ...frameRect, opacity: 1 }}
            />
          )}
        </>
      )}
    </section>
  )
}
