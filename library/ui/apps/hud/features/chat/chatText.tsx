// Rich chat text — ports the SDK7 chat's `decorateMessageWithLinks`: turn a raw
// message into clickable URLs, location coords (teleport), and @username mentions
// (clickable → profile viewer; highlighted when they mention you). Parsing is a pure
// function so it's unit-testable; <MessageText> renders the tokens with handlers.

import type { NearbyMember } from '@explorer-core/engine/protocol'
import { buildNameIndex, parseMessage, resolveMention } from '@explorer-core/features/chat/chatParse'

export function MessageText({
  text,
  members,
  styles,
  onMention,
  onLocation,
  onWorld
}: {
  text: string
  members: NearbyMember[]
  styles: { url: string; mention: string; location: string; world: string }
  /** A resolved @mention was clicked (address known). */
  onMention: (address: string, name: string, e: React.MouseEvent) => void
  /** A location link (x,y) was clicked. */
  onLocation: (x: number, y: number) => void
  /** A world name (e.g. boedo.dcl.eth) was clicked → prompt to jump to that realm. */
  onWorld?: (name: string) => void
}): React.JSX.Element {
  const index = buildNameIndex(members)
  return (
    <>
      {parseMessage(text).map((t, i) => {
        if (t.type === 'url') {
          return (
            <a key={i} className={styles.url} href={t.value} target="_blank" rel="noreferrer noopener">
              {t.value}
            </a>
          )
        }
        if (t.type === 'world') {
          if (!onWorld) return <span key={i}>{t.value}</span>
          return (
            <button key={i} type="button" className={styles.world} onClick={() => onWorld(t.value)}>
              {t.value}
            </button>
          )
        }
        if (t.type === 'location') {
          return (
            <button key={i} type="button" className={styles.location} onClick={() => onLocation(t.x, t.y)}>
              {t.value}
            </button>
          )
        }
        if (t.type === 'mention') {
          const addr = resolveMention(t, index)
          if (!addr) return <span key={i}>{t.value}</span>
          return (
            <button
              key={i}
              type="button"
              className={styles.mention}
              onClick={(e) => onMention(addr, t.name, e)}
              onContextMenu={(e) => onMention(addr, t.name, e)}
            >
              @{t.name}
            </button>
          )
        }
        return <span key={i}>{t.value}</span>
      })}
    </>
  )
}
