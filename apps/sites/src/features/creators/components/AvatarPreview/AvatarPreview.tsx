import { useEffect, useMemo, useRef, useState, type ReactNode } from 'react'
import { PreviewEmote, PreviewEmoteEventType, PreviewRenderer, PreviewUnityMode } from '@dcl/schemas'
import { Env } from '@dcl/ui-env'
import {
  WearablePreview,
  createWearablePreviewController,
  type WearablePreviewController
} from '@/features/creators/components/WearablePreview'
import { config } from '@/data/lib/creators/config'
import { usePreviewRenderer } from '@/features/creators/hooks/usePreviewRenderer'
import { type AvatarAttributes } from '@/data/lib/creators/avatar'
import {
  PREVIEW_WHEEL_START,
  PREVIEW_WHEEL_ZOOM,
  buildPreviewOptions,
  getPreviewZoom,
  isEmoteSubject,
  type AvatarPreviewSource
} from '@/data/lib/creators/preview'
import { createPreviewBridge, type PreviewBridge } from '@/data/lib/creators/previewBridge'
import { useAvatarPreview } from '@/features/creators/store/avatarPreview'
import styles from './AvatarPreview.module.css'

export type { AvatarPreviewSource }

type Props = {
  id: string
  source: AvatarPreviewSource
  avatar: AvatarAttributes
  /** Ignored while an emote item is the subject: the item owns the animation then. */
  emote?: PreviewEmote
  onLoad?: (renderer?: PreviewRenderer) => void
  onError?: (error: Error) => void
  onController?: (controller: WearablePreviewController) => void
  /** Overlay slot along the bottom (playback, customizer, badges). */
  children?: ReactNode
  testId?: string
}

/**
 * The shared avatar preview. Picks the renderer once (`usePreviewRenderer`), then mounts the
 * WearablePreview once with a frozen snapshot of the URL-bound options and pushes every later change
 * through `lib/previewBridge`, so avatar and item changes never reload the iframe.
 */
export function AvatarPreview(props: Props) {
  const renderer = usePreviewRenderer()
  const testId = props.testId ?? 'avatar-preview'
  if (renderer === undefined) {
    return (
      <div className={styles.wrap} data-testid={testId} data-loading>
        <div className={styles.loader} aria-busy="true" data-testid={`${testId}-loading`}>
          <span className="spinner" aria-hidden />
        </div>
      </div>
    )
  }
  return <MountedPreview {...props} unity={renderer === PreviewRenderer.UNITY} />
}

function MountedPreview({
  id,
  source,
  avatar,
  emote = PreviewEmote.IDLE,
  unity,
  onLoad,
  onError,
  onController,
  children,
  testId = 'avatar-preview'
}: Props & { unity: boolean }) {
  const [snapshot] = useState(() => ({ ...avatar, emote, unity, emoteSubject: isEmoteSubject(source) }))
  const [isLoading, setLoading] = useState(true)
  const bridgeRef = useRef<PreviewBridge | null>(null)
  const showSceneBoundaries = useAvatarPreview(state => state.showSceneBoundaries)
  const options = useMemo(
    () => buildPreviewOptions(source, avatar, emote, showSceneBoundaries),
    [source, avatar, emote, showSceneBoundaries]
  )
  const optionsRef = useRef(options)
  optionsRef.current = options
  const onLoadRef = useRef(onLoad)
  onLoadRef.current = onLoad
  const onErrorRef = useRef(onError)
  onErrorRef.current = onError
  const onControllerRef = useRef(onController)
  onControllerRef.current = onController

  // The iframe exists once the preview has committed; the bridge and the controller attach to it by id.
  useEffect(() => {
    const iframe = document.getElementById(id)
    if (!(iframe instanceof HTMLIFrameElement)) return
    // The iframe app draws its own loader once it has booted (READY); ours only covers the blank gap before that.
    const bridge = createPreviewBridge({ iframe, onBoot: () => setLoading(false) })
    bridgeRef.current = bridge
    bridge.update(optionsRef.current)

    // Created at mount, not in onLoad: Babylon fires PLAY right after LOAD and a late subscriber misses it.
    const controller = createWearablePreviewController(id)
    const { setPlaying, endEmote } = useAvatarPreview.getState()
    const onPlay = () => setPlaying(true)
    const onPause = () => setPlaying(false)
    const onEnd = () => endEmote()
    controller.emote.events.on(PreviewEmoteEventType.ANIMATION_PLAY, onPlay)
    controller.emote.events.on(PreviewEmoteEventType.ANIMATION_PAUSE, onPause)
    controller.emote.events.on(PreviewEmoteEventType.ANIMATION_END, onEnd)
    onControllerRef.current?.(controller)

    return () => {
      bridge.dispose()
      bridgeRef.current = null
      controller.emote.events.off(PreviewEmoteEventType.ANIMATION_PLAY, onPlay)
      controller.emote.events.off(PreviewEmoteEventType.ANIMATION_PAUSE, onPause)
      controller.emote.events.off(PreviewEmoteEventType.ANIMATION_END, onEnd)
      setPlaying(false)
    }
  }, [id])

  useEffect(() => {
    bridgeRef.current?.update(options)
  }, [options])

  return (
    <div className={styles.wrap} data-testid={testId} data-loading={isLoading || undefined}>
      <WearablePreview
        id={id}
        bodyShape={snapshot.bodyShape}
        skin={snapshot.skin}
        eyes={snapshot.eyes}
        hair={snapshot.hair}
        urns={snapshot.baseWearableUrns}
        disableDefaultWearables={snapshot.hasOutfit}
        emote={snapshot.emoteSubject ? undefined : snapshot.emote}
        disableDefaultEmotes={snapshot.emoteSubject || undefined}
        disableAutoRotate
        disableBackground
        zoom={getPreviewZoom(snapshot.emoteSubject ? undefined : snapshot.emote)}
        wheelZoom={PREVIEW_WHEEL_ZOOM}
        wheelStart={PREVIEW_WHEEL_START}
        unity={snapshot.unity}
        unityMode={PreviewUnityMode.BUILDER}
        dev={config.is(Env.DEVELOPMENT)}
        onLoad={renderer => {
          setLoading(false)
          onLoadRef.current?.(renderer)
        }}
        onError={error => {
          setLoading(false)
          onErrorRef.current?.(error)
        }}
      />
      {isLoading && (
        <div className={styles.loader} aria-busy="true" data-testid={`${testId}-loading`}>
          <span className="spinner" aria-hidden />
        </div>
      )}
      {children && (
        <div
          className={styles.overlay}
          data-testid={`${testId}-overlay`}
          data-emote-subject={isEmoteSubject(source) || undefined}
        >
          {children}
        </div>
      )}
    </div>
  )
}
