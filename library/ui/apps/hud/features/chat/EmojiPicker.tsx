// Emoji picker — eight section toggles, a search field, and every
// section in one scroll. Clicking an emoji inserts its glyph and keeps the panel open.

import { useEffect, useMemo, useRef, useState } from 'react'
import { MaskIcon, SearchField } from '../../design'
import { EMOJI_BY_CODE, EMOJI_GROUPS, loadRecents, pushRecent, searchByShortcode, type Emoji } from './emojiData'
import smileysIcon from '../../assets/chat/emoji-categories/smileyspeople.png'
import animalsIcon from '../../assets/chat/emoji-categories/animals.png'
import foodIcon from '../../assets/chat/emoji-categories/food.png'
import activitiesIcon from '../../assets/chat/emoji-categories/activities.png'
import placesIcon from '../../assets/chat/emoji-categories/places.png'
import objectsIcon from '../../assets/chat/emoji-categories/objects.png'
import symbolsIcon from '../../assets/chat/emoji-categories/symbols.png'
import flagIcon from '../../assets/chat/emoji-categories/flag.png'
import { isCancelKey } from '../../lib/bindingLabels'
import styles from './EmojiPicker.module.css'

// Toggle order differs from the scroll order: smileys + people share one, activities comes before places.
const TOGGLES: { icon: string; label: string; section: string }[] = [
  { icon: smileysIcon, label: 'Smileys and people', section: 'Smileys & Emotion' },
  { icon: animalsIcon, label: 'Animals and nature', section: 'Animals & Nature' },
  { icon: foodIcon, label: 'Food and drink', section: 'Food & Drink' },
  { icon: activitiesIcon, label: 'Activities', section: 'Activities' },
  { icon: placesIcon, label: 'Travel and places', section: 'Travel & Places' },
  { icon: objectsIcon, label: 'Objects', section: 'Objects' },
  { icon: symbolsIcon, label: 'Symbols', section: 'Symbols' },
  { icon: flagIcon, label: 'Flags', section: 'Flags' }
]

function Grid({ emojis, onPick }: { emojis: Emoji[]; onPick: (e: Emoji) => void }): React.JSX.Element {
  return (
    <div className={styles.grid}>
      {emojis.map((e) => (
        <button key={e.code} type="button" className={styles.emoji} title={e.expression} data-ui-sound="press" onClick={() => onPick(e)}>
          {e.emoji}
        </button>
      ))}
    </div>
  )
}

const COLS = 8
const CELL = 36
const GAP = 2
// The scrollbar's thumb: it starts 10px down the scroll area (the CSS's top), stops 3px short of
// its end and is never shorter than 33px.
const THUMB_PAD = 13
const THUMB_MIN = 33
const CHUNK_ROWS = 10

/** A block of rows that mounts its buttons only once it nears the visible area; until then it
 *  holds its exact height, so section offsets (and jumping to a section) stay correct. */
function LazyRows({ emojis, onPick, root, eager }: { emojis: Emoji[]; onPick: (e: Emoji) => void; root: React.RefObject<HTMLDivElement | null>; eager: boolean }): React.JSX.Element {
  const ref = useRef<HTMLDivElement>(null)
  const [shown, setShown] = useState(eager || typeof IntersectionObserver === 'undefined')
  useEffect(() => {
    if (shown || ref.current == null) return
    const io = new IntersectionObserver((entries) => entries.some((e) => e.isIntersecting) && setShown(true), { root: root.current, rootMargin: '300px 0px' })
    io.observe(ref.current)
    return () => io.disconnect()
  }, [shown, root])
  const rows = Math.ceil(emojis.length / COLS)
  if (shown) return <Grid emojis={emojis} onPick={onPick} />
  return <div ref={ref} style={{ height: rows * CELL + (rows - 1) * GAP }} />
}

function Section({ name, emojis, onPick, root, eager }: { name: string; emojis: Emoji[]; onPick: (e: Emoji) => void; root: React.RefObject<HTMLDivElement | null>; eager: boolean }): React.JSX.Element {
  const per = COLS * CHUNK_ROWS
  const chunks = useMemo(() => Array.from({ length: Math.ceil(emojis.length / per) }, (_, i) => emojis.slice(i * per, (i + 1) * per)), [emojis, per])
  return (
    <section className={styles.section} data-section={name}>
      <div className={styles.sectionHeader}>{name === 'Smileys & Emotion' ? 'Smileys and emotions' : name.replace(' & ', ' and ')}</div>
      <div className={styles.rows}>
        {chunks.map((c, i) => (
          <LazyRows key={i} emojis={c} onPick={onPick} root={root} eager={eager && i === 0} />
        ))}
      </div>
    </section>
  )
}

export function EmojiPicker({ onPick, onClose }: { onPick: (glyph: string) => void; onClose?: () => void }): React.JSX.Element {
  const [query, setQuery] = useState('')
  const [section, setSection] = useState<string | null>(null)
  const bodyRef = useRef<HTMLDivElement>(null)
  const [recents, setRecents] = useState<string[]>(() => loadRecents())
  const q = query.trim()
  const results = useMemo(() => (q ? searchByShortcode(q, 200) : []), [q])
  const recentEmojis = useMemo(() => recents.map((c) => EMOJI_BY_CODE.get(c)).filter((e): e is Emoji => e != null), [recents])
  const pick = (e: Emoji): void => {
    onPick(e.emoji)
    setRecents(pushRecent(e.code))
  }

  const jump = (name: string): void => {
    setQuery('')
    setSection(name)
    requestAnimationFrame(() => {
      const el = bodyRef.current?.querySelector<HTMLElement>(`[data-section="${name}"]`)
      if (el && bodyRef.current) bodyRef.current.scrollTop = el.offsetTop
    })
  }

  // The thumb's height and place follow the scroll area; `travel` is how far it can move.
  const thumbRef = useRef<HTMLDivElement>(null)
  const travel = useRef(0)
  const placeThumb = (): void => {
    const body = bodyRef.current
    const thumb = thumbRef.current
    if (!body || !thumb) return
    const range = body.scrollHeight - body.clientHeight
    thumb.style.display = range > 0 ? '' : 'none'
    if (range <= 0) return
    const track = body.clientHeight - THUMB_PAD
    const height = Math.max(THUMB_MIN, (track * body.clientHeight) / body.scrollHeight)
    travel.current = track - height
    thumb.style.height = `${height}px`
    thumb.style.transform = `translateY(${(travel.current * body.scrollTop) / range}px)`
  }
  useEffect(placeThumb)
  const drag = useRef<{ y: number; top: number } | null>(null)
  const onThumbDown = (e: React.PointerEvent<HTMLDivElement>): void => {
    if (!bodyRef.current) return
    e.currentTarget.setPointerCapture(e.pointerId)
    drag.current = { y: e.clientY, top: bodyRef.current.scrollTop }
  }
  const onThumbMove = (e: React.PointerEvent<HTMLDivElement>): void => {
    const body = bodyRef.current
    if (!drag.current || !body || travel.current <= 0) return
    // the panel is scaled with the HUD: the thumb's drawn width over its 8px gives the scale
    const scale = e.currentTarget.getBoundingClientRect().width / 8 || 1
    body.scrollTop = drag.current.top + ((e.clientY - drag.current.y) / scale) * ((body.scrollHeight - body.clientHeight) / travel.current)
  }

  // The toggle follows the section at the top of the scroll (read once per frame).
  const scrollFrame = useRef(0)
  const onScroll = (): void => {
    if (scrollFrame.current !== 0) return
    scrollFrame.current = requestAnimationFrame(() => {
      scrollFrame.current = 0
      placeThumb()
      trackSection()
    })
  }
  const trackSection = (): void => {
    const body = bodyRef.current
    if (!body || q) return
    let current = section ?? TOGGLES[0].section
    for (const el of body.querySelectorAll<HTMLElement>('[data-section]')) {
      if (el.offsetTop <= body.scrollTop + 1) current = el.dataset.section ?? current
    }
    if (section == null && body.scrollTop === 0) return
    const toggle = current === 'People & Body' || current === 'Frequently used' ? 'Smileys & Emotion' : current
    if (toggle !== section) setSection(toggle)
  }

  return (
    <div className={styles.root} role="dialog" aria-label="Emoji picker" onKeyDown={(e) => {
        e.stopPropagation()
        if (isCancelKey(e)) onClose?.()
      }}
    >
      <div className={styles.tabs}>
        {TOGGLES.map((t) => (
          <button
            key={t.section}
            type="button"
            className={`${styles.tab} ${t.section === section && !q ? styles.tabActive : ''}`.trim()}
            aria-label={t.label}
            aria-pressed={t.section === section && !q}
            data-ui-sound="press"
            onClick={() => jump(t.section)}
          >
            <MaskIcon src={t.icon} size={25} />
          </button>
        ))}
      </div>

      <div className={styles.searchRow}>
        <SearchField variant="light" size="sm" value={query} onChange={setQuery} placeholder="Search emoji" />
      </div>

      <div className={styles.scroll}>
      <div ref={bodyRef} className={styles.body} onScroll={onScroll}>
        {q ? (
          results.length > 0 ? (
            <Grid emojis={results} onPick={pick} />
          ) : (
            <div className={styles.noResults}>No results</div>
          )
        ) : (
          <>
            {recentEmojis.length > 0 && <Section name="Frequently used" emojis={recentEmojis} onPick={pick} root={bodyRef} eager />}
            {EMOJI_GROUPS.map((g, i) => (
              <Section key={g.name} name={g.name} emojis={g.emojis} onPick={pick} root={bodyRef} eager={i === 0} />
            ))}
          </>
        )}
      </div>
      <div
        ref={thumbRef}
        className={styles.thumb}
        aria-hidden="true"
        onMouseDown={(e) => e.preventDefault()}
        onPointerDown={onThumbDown}
        onPointerMove={onThumbMove}
        onPointerUp={() => (drag.current = null)}
        onPointerCancel={() => (drag.current = null)}
      />
      </div>
    </div>
  )
}
