// The passport's avatar, drawn live by the engine in the passport's left column.
//
// The bridge scene draws it under the page (passportPreview), over its own copy of the panel's background,
// so every layer of the page over the column is cut open: the panel's background (ProfilePassport's
// `data-live-avatar`), the passport's scrim and whatever is under it (lib/engineWindow). A cover in the
// panel's own colours stays over the column until the scene says the avatar is drawn. When it cannot be
// drawn (no engine, a look that fails to load) the column shows the picture the passport has instead.
//
// One avatar is live at a time, the topmost passport's: a passport under another shows its picture.

import { useContext, useEffect, useLayoutEffect, useRef, useState, useSyncExternalStore, type ReactNode } from 'react'
import { Spinner } from '@dcl/one-ui'
import { subscribeHudScale } from '@explorer-core/lib/hudScale'
import type { AvatarLook } from '@explorer-core/engine/avatarEquip'
import type { PassportBackground, PassportPreviewMessage } from '@explorer-core/engine/protocol'
import { useSession } from '../session/SessionContext'
import { PopupIdContext } from '../../design/popups'
import { setEngineWindow } from '../../lib/engineWindow'
import cursorRotate from '../../assets/passport/cursor-rotate.png'
import styles from './PassportAvatar.module.css'

/** Longer than the scene takes to answer (passportPreview's LOAD_SECONDS, then DRAWN_FRAMES): a scene that
 *  never answers leaves the picture. */
const ANSWER_TIMEOUT_MS = 15000
/** Screen pixels the opening under the panel runs past the column's right edge. */
const WINDOW_BLEED = 8

// The avatars asking to be shown, oldest first: the last one is live.
let claims: number[] = []
let nextId = 1
const listeners = new Set<() => void>()
const subscribe = (listener: () => void): (() => void) => {
  listeners.add(listener)
  return () => listeners.delete(listener)
}
const changed = (): void => listeners.forEach((listener) => listener())
const top = (): number => claims[claims.length - 1] ?? 0

let live: { id: number; send: (msg: PassportPreviewMessage) => void } | null = null

/** Play an emote on the passport avatar on show, if one is. */
export function playPassportEmote(urn: string): void {
  live?.send({ kind: 'passportPreviewEmote', id: live.id, urn })
}

const nextFrame = (run: () => void): number =>
  typeof requestAnimationFrame === 'function' ? requestAnimationFrame(run) : (setTimeout(run, 16) as unknown as number)
const cancelFrame = (id: number): void => {
  if (typeof cancelAnimationFrame === 'function') cancelAnimationFrame(id)
  else clearTimeout(id)
}

/** Resolves once the entrance animations of the element's ancestors (the popup scaling in) have ended:
 *  its box on screen is not the final one before. */
async function settled(el: HTMLElement): Promise<void> {
  const running: Animation[] = []
  for (let node: HTMLElement | null = el; node != null; node = node.parentElement) {
    running.push(...(node.getAnimations?.() ?? []).filter((a) => a.playState === 'running'))
  }
  await Promise.all(running.map((a) => a.finished.catch(() => undefined)))
}

export function PassportAvatar({
  look,
  name,
  background,
  wave,
  picture,
  onLive,
  onReady
}: {
  look: AvatarLook
  name: string
  /** The panel's background on show, which the scene draws under the avatar. */
  background: PassportBackground
  wave: boolean
  /** Shown instead when the avatar cannot be drawn. */
  picture: ReactNode
  /** Whether the engine's avatar is in the column: the panel leaves its background open over it. */
  onLive: (live: boolean) => void
  /** Whether the avatar is drawn and on show: an emote can play on it. */
  onReady: (ready: boolean) => void
}): React.JSX.Element {
  const { passportPreview } = useSession()
  const popup = useContext(PopupIdContext)
  const [id] = useState(() => nextId++)
  const active = useSyncExternalStore(subscribe, () => top() === id, () => false)
  const [timedOut, setTimedOut] = useState(false)
  const state = passportPreview.state?.id === id ? passportPreview.state.state : null
  const failed = state === 'failed' || timedOut
  const shown = state === 'ready'
  const on = active && !failed
  const stage = useRef<HTMLDivElement>(null)
  const send = useRef(passportPreview.send)
  send.current = passportPreview.send
  const onLiveRef = useRef(onLive)
  onLiveRef.current = onLive
  const onReadyRef = useRef(onReady)
  onReadyRef.current = onReady
  const waveRef = useRef(wave)

  useLayoutEffect(() => {
    claims = [...claims, id]
    changed()
    return () => {
      claims = claims.filter((claim) => claim !== id)
      changed()
    }
  }, [id])

  useEffect(() => {
    if (!on || shown) return
    const timer = setTimeout(() => setTimedOut(true), ANSWER_TIMEOUT_MS)
    return () => clearTimeout(timer)
  }, [on, shown])

  useEffect(() => {
    if (!on || !shown) return
    onReadyRef.current(true)
    return () => onReadyRef.current(false)
  }, [on, shown])

  // The look and the background are read when the scene is told, not depended on: telling it again must
  // not take the avatar down first.
  const lookRef = useRef(look)
  lookRef.current = look
  const backgroundRef = useRef(background)
  backgroundRef.current = background
  const push = useRef<(() => void) | null>(null)
  useLayoutEffect(() => {
    const el = stage.current
    if (!on || el == null) return
    let reported = false
    const tell = (): void => {
      const box = el.getBoundingClientRect()
      if (!(box.width > 0 && box.height > 0)) return
      const rect = { x: box.left, y: box.top, width: box.width, height: box.height }
      const panel = el.closest<HTMLElement>('[data-passport-panel]')
      // the column's outer corners are the panel's own rounded ones, at the HUD's scale
      const radius = panel != null ? (parseFloat(getComputedStyle(panel).borderTopLeftRadius) || 0) * (box.width / (el.offsetWidth || box.width)) : 0
      reported = true
      send.current({ kind: 'passportPreview', open: { id, look: lookRef.current, rect, dpr: window.devicePixelRatio || 1, background: backgroundRef.current, wave: waveRef.current } })
      // The layers under the panel open a little further right than the column, under the panel's own
      // background: an opening that ended at the column's edge would shade that edge's pixels with their own.
      setEngineWindow({ popup, rect: { ...rect, width: rect.width + WINDOW_BLEED }, round: { topLeft: radius, bottomLeft: radius } })
      onLiveRef.current(true)
    }
    const again = (): void => {
      if (reported) tell()
    }
    push.current = again
    let gone = false
    void settled(el).then(() => {
      if (!gone) tell()
    })
    const observer = new ResizeObserver(again)
    observer.observe(el)
    const unsubscribe = subscribeHudScale(again)
    live = { id, send: (msg) => send.current(msg) }
    return () => {
      gone = true
      push.current = null
      observer.disconnect()
      unsubscribe()
      if (live?.id === id) live = null
      onLiveRef.current(false)
      // A newer passport's avatar has taken the stage already: closing here would close that one.
      if (claims.some((claim) => claim > id)) return
      if (reported) send.current({ kind: 'passportPreview', open: null })
      setEngineWindow(null)
    }
  }, [on, id, popup])
  const lookKey = JSON.stringify(look)
  useLayoutEffect(() => {
    push.current?.()
  }, [lookKey, background])

  if (!on) return <>{picture}</>
  return (
    <div ref={stage} className={styles.stage}>
      <div className={styles.cover} data-shown={shown ? '' : undefined} />
      {!shown && (
        <div className={styles.loading}>
          <Spinner variant="thin" size={70} />
        </div>
      )}
      <TurnSurface id={id} name={name} send={(msg) => send.current(msg)} />
    </div>
  )
}

/** The scene works out the turn and the coasting from the drag's speed, so this only reports the drag. */
function TurnSurface({ id, name, send }: { id: number; name: string; send: (msg: PassportPreviewMessage) => void }): React.JSX.Element {
  const self = useRef<HTMLDivElement>(null)
  const cursor = useRef<HTMLImageElement>(null)
  const drag = useRef<{ pointer: number; x: number } | null>(null)
  const pending = useRef({ dx: 0, since: 0, frame: 0 })
  const [dragging, setDragging] = useState(false)

  const flush = (): void => {
    const p = pending.current
    p.frame = 0
    const now = performance.now()
    // screen pixels, not CSS ones: the scene turns the avatar a degree a pixel
    if (p.dx !== 0) send({ kind: 'passportPreviewDrag', id, dx: p.dx * (window.devicePixelRatio || 1), seconds: Math.max(1 / 240, (now - p.since) / 1000) })
    p.dx = 0
    p.since = now
  }
  useEffect(
    () => () => {
      if (pending.current.frame !== 0) cancelFrame(pending.current.frame)
    },
    []
  )

  const placeCursor = (e: React.PointerEvent): void => {
    const el = self.current
    const img = cursor.current
    if (el == null || img == null) return
    const box = el.getBoundingClientRect()
    const scale = el.offsetWidth > 0 ? box.width / el.offsetWidth : 1
    img.style.left = `${(e.clientX - box.left) / scale}px`
    img.style.top = `${(e.clientY - box.top) / scale}px`
  }
  const release = (pointer: number): void => {
    if (drag.current?.pointer !== pointer) return
    drag.current = null
    setDragging(false)
  }

  return (
    <div
      ref={self}
      className={styles.surface}
      data-dragging={dragging ? '' : undefined}
      role="img"
      aria-label={`${name}'s avatar. Drag it to turn it.`}
      onPointerDown={(e) => {
        if (e.button !== 0) return
        drag.current = { pointer: e.pointerId, x: e.clientX }
        pending.current.since = performance.now()
        e.currentTarget.setPointerCapture?.(e.pointerId)
        setDragging(true)
        placeCursor(e)
      }}
      onPointerMove={(e) => {
        const d = drag.current
        if (d == null || d.pointer !== e.pointerId) return
        placeCursor(e)
        pending.current.dx += e.clientX - d.x
        d.x = e.clientX
        if (pending.current.frame === 0) pending.current.frame = nextFrame(flush)
      }}
      onPointerUp={(e) => release(e.pointerId)}
      onPointerCancel={(e) => release(e.pointerId)}
      onLostPointerCapture={(e) => release(e.pointerId)}
    >
      <img ref={cursor} className={styles.cursor} src={cursorRotate} alt="" aria-hidden="true" draggable={false} />
    </div>
  )
}
