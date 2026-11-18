import { useEffect, useMemo, useRef, useState } from 'react'
import { PreviewProjection, PreviewRenderer } from '@dcl/schemas'
import { WearablePreview, createWearablePreviewController } from '@/features/creators/components/WearablePreview'
import { usePreviewRenderer } from '@/features/creators/hooks/usePreviewRenderer'
import { track } from '@/data/lib/creators/analytics'
import { captureError } from '@/data/lib/creators/monitoring'
import { ItemType, hasEmptyModelMetrics, type ItemMetrics } from '@/data/lib/creators/items'
import { isPreviewEmpty, retryWhilePreviewReloads, toEmoteWithBlobs, toWearableWithBlobs } from '@/data/lib/creators/preview'
import { renderPosedThumbnail } from '@/data/lib/creators/renderPosedThumbnail'
import { getThumbnailPose, isAutoThumbnailStale } from '@/data/lib/creators/thumbnailPose'
import { type PreviewDraft, type PreviewResult } from '@/data/lib/creators/draftPreview'
import styles from './ItemDraftModal.module.css'

const PREVIEW_ID = 'draft-processor'
const THUMBNAIL_SIZE = 1024
// A controller call against a scene being replaced throws ("The scene was disposed, a newer render
// has replaced it"); a few attempts absorb that without hiding a genuinely broken file.
const MAX_ATTEMPTS = 4
const RETRY_DELAY_MS = 750
// A Unity scene that never loads wedges the instance for every later draft, so a run that overruns its budget
// remounts the iframe once and then fails the draft instead of holding the queue. A fresh iframe has to boot first.
export const BOOT_BUDGET_MS = 45_000
export const RUN_BUDGET_MS = 20_000
// Once the model's LOAD is in, the reads may still wait out Unity reloads (up to 30s each) and retries.
export const READ_BUDGET_MS = 75_000

export type PreviewFailure = 'timeout' | 'error'

type Props = {
  draft: PreviewDraft
  /** Tags the Sentry report and the analytics event. */
  flow: 'add_items' | 'item_file_change'
  /** Files in the batch the draft belongs to. */
  batchSize?: number
  /** Apply with `applyPreviewResult`, onto the draft as it is by then. */
  onResult: (id: string, result: PreviewResult) => void
  onError: (id: string, reason: PreviewFailure) => void
}

type Run = {
  runKey: string
  done: boolean
  busy: boolean
  /** A LOAD landed while an attempt was running; that attempt may have read the previous scene. */
  loadPending: boolean
  attempts: number
  retryTimer: number
  startedAt: number
  mounts: number
}

type LoadPhase = 'idle' | 'update-sent' | 'loaded'

/**
 * Off-screen preview that fills a draft's metrics and auto thumbnail, one draft at a
 * time (the parent passes the next draft that still needs preview data). Categories with a
 * thumbnail pose are rendered offscreen instead of screenshotting the preview, and re-rendered
 * when the category change alters the pose.
 */
export function DraftProcessor({ draft, flow, batchSize = 1, onResult, onError }: Props) {
  const renderer = usePreviewRenderer()
  const [mountCount, setMountCount] = useState(0)
  // A controller call can still fail mid-flight when the scene is being replaced, so attempts are
  // retried until one round-trip completes against a live scene.
  const pose = draft.type === ItemType.WEARABLE ? getThumbnailPose(draft.category) : null
  // A pose change on an already-processed draft is a new run against the same iframe.
  const runKey = `${draft.id}:${pose ?? ''}`
  const runRef = useRef<Run>({
    runKey: '',
    done: false,
    busy: false,
    loadPending: false,
    attempts: 0,
    retryTimer: 0,
    startedAt: 0,
    mounts: 1
  })

  if (runRef.current.runKey !== runKey) {
    const previous = runRef.current
    window.clearTimeout(previous.retryTimer)
    runRef.current = {
      runKey,
      done: false,
      busy: false,
      loadPending: false,
      attempts: 0,
      retryTimer: 0,
      startedAt: performance.now(),
      // A pose change on the same draft keeps its remount count, so "remount once, then fail" still holds.
      mounts: previous.runKey.startsWith(`${draft.id}:`) ? previous.mounts : 1
    }
  }

  const isEmote = draft.type === ItemType.EMOTE
  // Emote-only options are URL params, so a type switch is a fresh iframe too.
  const iframeKey = `${isEmote ? 'emote' : 'wearable'}-${mountCount}`
  // Until a fresh iframe's first LOAD, the run's budget also covers the boot.
  const bootedRef = useRef({ iframeKey: '', booted: false })
  if (bootedRef.current.iframeKey !== iframeKey) bootedRef.current = { iframeKey, booted: false }

  const report = (run: Run, outcome: 'ok' | PreviewFailure) => {
    track('Item preview processed', {
      flow,
      renderer: renderer ?? null,
      item_type: draft.type,
      outcome,
      duration_ms: Math.round(performance.now() - run.startedAt),
      attempts: run.mounts,
      batch_size: batchSize
    })
  }

  const fail = (run: Run, reason: PreviewFailure) => {
    run.done = true
    window.clearTimeout(run.retryTimer)
    report(run, reason)
    onError(draft.id, reason)
  }

  // The iframe is kept across drafts (a remount reboots the whole preview app), so a new draft's
  // model travels as an UPDATE message. Until that message is out, any LOAD still belongs to the
  // previous scene and must not trigger an attempt.
  const loadRef = useRef<{ draftId: string; phase: LoadPhase }>({ draftId: '', phase: 'idle' })
  if (loadRef.current.draftId !== draft.id) {
    loadRef.current = { draftId: draft.id, phase: 'idle' }
  }

  const watchdogRef = useRef(0)
  const armWatchdog = (budget: number) => {
    window.clearTimeout(watchdogRef.current)
    const run = runRef.current
    watchdogRef.current = window.setTimeout(() => {
      if (runRef.current !== run || run.done) return
      if (run.mounts > 1) {
        captureError(new Error('The preview did not process the model in time'), {
          flow,
          step: 'preview_timeout',
          renderer,
          itemType: draft.type
        })
        fail(run, 'timeout')
        // The wedged instance would otherwise make the next draft wait out a whole budget too.
        setMountCount(count => count + 1)
        return
      }
      // A fresh run object orphans whatever the wedged iframe still owes the old one.
      window.clearTimeout(run.retryTimer)
      runRef.current = {
        ...run,
        busy: false,
        loadPending: false,
        attempts: 0,
        retryTimer: 0,
        mounts: run.mounts + 1
      }
      loadRef.current = { draftId: draft.id, phase: 'idle' }
      setMountCount(count => count + 1)
    }, budget)
  }

  const attempt = () => {
    const run = runRef.current
    if (run.done) return
    if (run.busy) {
      run.loadPending = true
      return
    }
    run.busy = true

    void (async () => {
      try {
        const controller = createWearablePreviewController(PREVIEW_ID)

        if (draft.type === ItemType.EMOTE) {
          // Screenshot a random frame, like the legacy modal.
          const length = await controller.emote.getLength()
          if (length > 0) {
            await controller.emote.goTo(Math.floor(Math.random() * length))
          }
        }

        // Waiting out a Unity reload is not a failed attempt: the item is still on its way into the scene.
        const screenshot = () =>
          retryWhilePreviewReloads(() => controller.scene.getScreenshot(THUMBNAIL_SIZE, THUMBNAIL_SIZE))
        // The iframe also announces its initial, model-less render, and that LOAD can land after the UPDATE
        // was sent: metrics read against that empty scene have no geometry on Babylon, an error on Unity. Not a
        // failure, the model just isn't in yet; the real render's own LOAD starts the next read.
        const metrics: ItemMetrics | null =
          draft.type === ItemType.WEARABLE
            ? await retryWhilePreviewReloads(() => controller.scene.getMetrics()).catch((error: unknown) => {
                if (isPreviewEmpty(error)) return null
                throw error
              })
            : (draft.metrics ?? {})
        if (!metrics || (draft.type === ItemType.WEARABLE && hasEmptyModelMetrics(metrics))) return

        const result: PreviewResult = { metrics, thumbnail: null, thumbnailCategory: draft.category }
        if (!draft.thumbnail || isAutoThumbnailStale(draft)) {
          result.thumbnail = pose
            ? await renderPosedThumbnail(draft.contents, draft.model, pose).catch((error: unknown) => {
                console.warn('Posed thumbnail render failed, falling back to preview screenshot', error)
                return screenshot()
              })
            : await screenshot()
        }

        if (runRef.current !== run || run.done) return
        run.done = true
        report(run, 'ok')
        onResult(draft.id, result)
      } catch (error) {
        if (runRef.current !== run || run.done) return
        run.attempts++
        if (run.attempts >= MAX_ATTEMPTS) {
          console.error('Preview processing failed:', error)
          fail(run, 'error')
          return
        }
        run.retryTimer = window.setTimeout(attempt, RETRY_DELAY_MS)
      } finally {
        run.busy = false
        if (run.loadPending && runRef.current === run && !run.done) {
          run.loadPending = false
          attempt()
        }
      }
    })()
  }

  // The iframe's onLoad only fires once per draft: a later pose change has to kick off its run here.
  useEffect(() => {
    if (loadRef.current.phase === 'loaded') attempt()
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [runKey])

  useEffect(() => {
    if (renderer === undefined) return
    armWatchdog(bootedRef.current.booted ? RUN_BUDGET_MS : BOOT_BUDGET_MS)
    return () => window.clearTimeout(watchdogRef.current)
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [runKey, iframeKey, renderer])

  // Rewrapping the same contents each render would reload the iframe endlessly.
  const blob = useMemo(
    () => (isEmote ? toEmoteWithBlobs(draft.contents, draft.id) : toWearableWithBlobs(draft.contents, draft.id)),
    [isEmote, draft.contents, draft.id]
  )

  return (
    <div className={styles.hiddenPreview} aria-hidden data-testid="draft-processor">
      {renderer !== undefined && (
        <WearablePreview
          key={iframeKey}
          id={PREVIEW_ID}
          blob={blob}
          unity={renderer === PreviewRenderer.UNITY}
          disableBackground
          disableAutoRotate
          projection={PreviewProjection.ORTHOGRAPHIC}
          {...(isEmote ? { profile: 'default', disableFace: true, disableDefaultWearables: true, skin: '000000' } : {})}
          onUpdate={() => {
            if (loadRef.current.phase === 'idle') loadRef.current.phase = 'update-sent'
          }}
          onLoad={() => {
            // A LOAD before this iframe's UPDATE went out is its model-less boot scene: only a later one says the
            // model is on its way in.
            if (loadRef.current.phase === 'idle') return
            bootedRef.current.booted = true
            if (loadRef.current.phase === 'update-sent') armWatchdog(READ_BUDGET_MS)
            loadRef.current.phase = 'loaded'
            attempt()
          }}
          onError={() => {
            // Iframe-level errors also count as attempts so a broken preview can't hang forever.
            const run = runRef.current
            if (run.done || run.busy) return
            run.attempts++
            if (run.attempts >= MAX_ATTEMPTS) {
              fail(run, 'error')
            } else {
              window.clearTimeout(run.retryTimer)
              run.retryTimer = window.setTimeout(attempt, RETRY_DELAY_MS)
            }
          }}
        />
      )}
    </div>
  )
}
