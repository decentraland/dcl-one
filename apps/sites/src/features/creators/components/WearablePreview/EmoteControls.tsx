import { useEffect, useRef, useState } from 'react'
import { PreviewEmoteEventType } from '@dcl/schemas/dist/dapps/preview/preview-emote-event-type'
import { PauseIcon, PlayArrowIcon, VolumeOffIcon, VolumeUpIcon } from '../Icons'
import { useTranslation } from '@/data/lib/creators/intl'
import { cx } from '@/data/lib/creators/cx'
import { usePreviewController } from './usePreviewController'
import styles from './PreviewControls.module.css'
import type { EmoteControlsProps } from './types'

// The slider counts hundredths of a second.
const STEPS_PER_SECOND = 100

/** Play/pause, a seek bar, the current second and a mute button for the emote a preview is playing. */
export function EmoteControls({ previewId, controller: shared, className, hideFrameInput, hideSoundButton }: EmoteControlsProps) {
  const { t } = useTranslation()
  const { controller, isReady } = usePreviewController(previewId, shared)
  const [position, setPosition] = useState(0)
  const [isPlaying, setIsPlaying] = useState(false)
  const [length, setLength] = useState<number>()
  const [isSoundEnabled, setIsSoundEnabled] = useState(true)
  const [hasSound, setHasSound] = useState<boolean>()
  // While the creator drags the seek bar the emote is paused; it resumes on release if it was playing.
  const seeking = useRef({ active: false, resume: false })
  const lengthRef = useRef(length)
  lengthRef.current = length

  useEffect(() => {
    if (!controller) return
    const { events } = controller.emote
    const onPlay = async () => {
      if (seeking.current.active) return
      const emoteLength = lengthRef.current || (await controller.emote.getLength())
      setIsPlaying(true)
      setLength(emoteLength)
    }
    const onPlaying = ({ length: played }: { length?: number }) => setPosition(Math.ceil((played ?? 0) * STEPS_PER_SECOND))
    const onEnd = () => {
      setIsPlaying(false)
      setPosition(current => (current >= Math.floor((lengthRef.current ?? 0) * STEPS_PER_SECOND) ? 0 : current))
    }
    const onPause = () => setIsPlaying(false)
    const onLoop = () => setPosition(0)
    events.on(PreviewEmoteEventType.ANIMATION_PLAY, onPlay)
    events.on(PreviewEmoteEventType.ANIMATION_PLAYING, onPlaying)
    events.on(PreviewEmoteEventType.ANIMATION_END, onEnd)
    events.on(PreviewEmoteEventType.ANIMATION_PAUSE, onPause)
    events.on(PreviewEmoteEventType.ANIMATION_LOOP, onLoop)
    return () => {
      events.off(PreviewEmoteEventType.ANIMATION_PLAY, onPlay)
      events.off(PreviewEmoteEventType.ANIMATION_PLAYING, onPlaying)
      events.off(PreviewEmoteEventType.ANIMATION_END, onEnd)
      events.off(PreviewEmoteEventType.ANIMATION_PAUSE, onPause)
      events.off(PreviewEmoteEventType.ANIMATION_LOOP, onLoop)
    }
  }, [controller])

  useEffect(() => {
    if (hasSound !== undefined || !isReady || !controller) return
    controller.emote.hasSound().then(setHasSound, () => setHasSound(false))
  }, [hasSound, isReady, controller])

  const togglePlay = () => {
    if (!controller) return
    void (isPlaying ? controller.emote.pause() : controller.emote.play()).catch(() => undefined)
  }

  const seek = (value: number) => {
    if (Number.isNaN(value) || !controller) return
    const target = Math.min(value, (length ?? 0) * STEPS_PER_SECOND)
    setPosition(target)
    seeking.current.active = true
    const paused = isPlaying ? ((seeking.current.resume = true), controller.emote.pause()) : Promise.resolve()
    void paused.then(() => controller.emote.goTo(target / STEPS_PER_SECOND)).catch(() => undefined)
  }

  const release = () => {
    const { resume } = seeking.current
    seeking.current = { active: false, resume: false }
    if (resume) void controller?.emote.play().catch(() => undefined)
  }

  const toggleSound = () => {
    void (isSoundEnabled ? controller?.emote.disableSound() : controller?.emote.enableSound())
    setIsSoundEnabled(!isSoundEnabled)
  }

  return (
    <div className={cx(styles.emote, className)} data-preview-control="emote">
      <button
        type="button"
        className={styles.button}
        data-preview-control="button"
        aria-label={t(isPlaying ? 'preview_controls.pause' : 'preview_controls.play')}
        onClick={togglePlay}
      >
        {isPlaying ? <PauseIcon /> : <PlayArrowIcon />}
      </button>
      <input
        type="range"
        className={styles.progress}
        aria-label={t('preview_controls.progress')}
        value={position}
        min={0}
        max={Math.ceil((length ?? 0) * STEPS_PER_SECOND)}
        step={1}
        onChange={event => seek(Number(event.target.value))}
        onPointerUp={release}
        onKeyUp={release}
      />
      {!hideFrameInput && (
        <input
          type="number"
          className={styles.frame}
          aria-label={t('preview_controls.frame')}
          value={Math.round(position / STEPS_PER_SECOND)}
          onChange={event => seek(Number(event.target.value) * STEPS_PER_SECOND)}
          onBlur={release}
        />
      )}
      {!hideSoundButton && hasSound && (
        <button
          type="button"
          className={styles.button}
          data-preview-control="button"
          data-muted={!isSoundEnabled || undefined}
          aria-label={t(isSoundEnabled ? 'preview_controls.mute' : 'preview_controls.unmute')}
          onClick={toggleSound}
        >
          {isSoundEnabled ? <VolumeUpIcon /> : <VolumeOffIcon />}
        </button>
      )}
    </div>
  )
}
