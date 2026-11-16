// The ":smile" autocomplete list the chat input and the announcement composer share: the emojis
// whose shortcode starts with the word at the caret, one selected, picked by click or Enter.
import { searchByShortcode, type Emoji } from './emojiData'
import styles from './EmojiSuggestList.module.css'

export interface EmojiSuggestion {
  items: Emoji[]
  /** The ":word" being replaced, from its colon to the caret. */
  start: number
  end: number
  /** The highlighted row. */
  sel: number
}

/** The ":emoji" or "@name" word ending at the caret, if any. */
export function tokenAtCaret(value: string, caret: number): { kind: 'emoji' | 'mention'; query: string; start: number } | null {
  const before = value.slice(0, caret)
  const m = before.match(/(?:^|\s)([@:])(\S*)$/)
  if (!m) return null
  const query = m[2]
  const start = caret - query.length - 1
  if (m[1] === ':') return /^[\w+-]{2,}$/.test(query) && !/https?$/i.test(before.slice(0, start)) ? { kind: 'emoji', query, start } : null
  return /^[A-Za-z0-9]{1,15}$/.test(query) ? { kind: 'mention', query, start } : null
}

/** The emojis to suggest for the ":word" (2+ characters) at the caret, or null. */
export function emojiSuggestionAt(value: string, caret: number): EmojiSuggestion | null {
  const word = tokenAtCaret(value, caret)
  if (word?.kind !== 'emoji') return null
  const items = searchByShortcode(word.query, 50)
  return items.length > 0 ? { items, start: word.start, end: caret, sel: 0 } : null
}

/** The suggestion (emoji, or the chat's people) with its highlight moved one row down (+1) or up (-1), wrapping. */
export function moveSelection<S extends { items: readonly unknown[]; sel: number }>(s: S, by: 1 | -1): S {
  const n = s.items.length
  return { ...s, sel: (s.sel + by + n) % n }
}

/** The list; `className` places it over its input. Rows keep the input's focus when clicked. */
export function EmojiSuggestList({ suggestion, onAccept, className }: { suggestion: EmojiSuggestion; onAccept: (index: number) => void; className?: string }): React.JSX.Element {
  return (
    <div className={`${styles.panel} ${className ?? ''}`.trim()}>
      <ul className={styles.list} role="listbox">
        {suggestion.items.map((e, i) => (
          <li key={e.code} role="option" aria-selected={i === suggestion.sel}>
            <button
              type="button"
              className={`${styles.item} ${i === suggestion.sel ? styles.active : ''}`.trim()}
              onMouseDown={(ev) => ev.preventDefault()}
              onClick={() => onAccept(i)}
            >
              <span className={styles.glyph}>{e.emoji}</span>
              <span>{e.expression}</span>
            </button>
          </li>
        ))}
      </ul>
    </div>
  )
}
