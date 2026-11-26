// The on-screen lane of situational reactions: each one the engine plays, the local player's or a
// player's nearby, rises from beside the reactions button and is gone. The engine passes them only
// while the "Chat Reactions" setting is on, so the lane has no switch of its own. It is part of the
// chat: when the chat gives way to a panel or the HUD is hidden it goes too, with what is in flight,
// so a screen with no HUD has no emoji over it. Reactions played meanwhile are not shown later.

import { memo, useEffect, useRef, useState } from 'react'
import { getHudScale } from '@explorer-core/lib/hudScale'
import type { ChatState } from '../session/useEngineSession'
import { MAX_FLYING, SWAY, SWAY_PERIOD, reactionFlight, type ReactionFlight } from './reactionMotion'
import styles from './ReactionLane.module.css'

interface Flying {
  id: number
  emoji: string
  /** Where it starts, in the lane's px. */
  left: number
  top: number
  flight: ReactionFlight
}

export const ReactionLane = memo(function ReactionLane({
  subscribe,
  anchor
}: {
  subscribe: ChatState['onReaction']
  /** What the reactions rise from beside: the reactions button's place, which keeps its box while
   *  the button is not drawn (the chat collapsed, the nearby list open). */
  anchor: React.RefObject<HTMLElement | null>
}): React.JSX.Element {
  const [flying, setFlying] = useState<Flying[]>([])
  // how many are in flight, known before the next render
  const count = useRef(0)
  const nextId = useRef(0)
  const timers = useRef(new Set<ReturnType<typeof setTimeout>>())

  useEffect(
    () =>
      subscribe((reaction) => {
        const from = anchor.current
        // one too many, or nobody watching: not shown, and not queued for later
        if (from == null || count.current >= MAX_FLYING || document.hidden) return
        const box = from.getBoundingClientRect()
        // the lane is scaled as the HUD is: client px back into its own
        const scale = getHudScale() || 1
        const flight = reactionFlight(Math.random)
        const id = nextId.current++
        count.current++
        setFlying((now) => [
          ...now,
          {
            id,
            emoji: reaction.emoji,
            left: (box.left + box.width / 2) / scale + flight.x,
            top: (box.top + box.height / 2) / scale - flight.y,
            flight
          }
        ])
        const timer = setTimeout(() => {
          timers.current.delete(timer)
          count.current--
          setFlying((now) => now.filter((one) => one.id !== id))
        }, flight.life * 1000)
        timers.current.add(timer)
      }),
    [subscribe, anchor]
  )
  useEffect(() => {
    const pending = timers.current
    return () => {
      for (const timer of pending) clearTimeout(timer)
      pending.clear()
    }
  }, [])

  return (
    <div className={styles.lane} aria-hidden="true">
      {flying.map(({ id, emoji, left, top, flight }) => (
        <span
          key={id}
          className={styles.particle}
          style={
            {
              left,
              top,
              '--life': `${flight.life}s`,
              '--rise': `${-flight.rise}px`,
              '--drift': `${flight.drift}px`,
              '--size': `${flight.size}px`,
              '--sway': `${SWAY}px`,
              '--sway-period': `${SWAY_PERIOD / 2}s`,
              '--sway-at': `${-flight.swayAt}s`
            } as React.CSSProperties
          }
        >
          <span className={styles.drift}>
            <span className={styles.sway}>
              <span className={styles.glyph}>{emoji}</span>
            </span>
          </span>
        </span>
      ))}
    </div>
  )
})
