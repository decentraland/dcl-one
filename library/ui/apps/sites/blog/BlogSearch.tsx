import { useId, useState, type KeyboardEvent } from 'react'
import { inAppLink, type BlogNavigate } from './blogLinks'
import styles from './BlogSearch.module.css'
import { SearchHighlight } from './SearchHighlight'

// The blog's search box with its suggestions, as source's Search (components/blog/Search/Search.tsx):
// suggestions show once the value is over 2 characters; up to 4 hits, then "See more results" when
// there are more; arrows move the selection, Enter opens it (or the search page), Escape clears.
// Ours adds the combobox semantics source lacks (role, expanded state, active option).

export type BlogSearchHit = {
  id: string
  url: string
  image: string
  /** Highlight markup (em around matches), rendered by SearchHighlight. */
  highlightedTitle: string
  highlightedDescription: string
}

export type BlogSearchStatus = 'loading' | 'error' | 'ready'

export type BlogSearchProps = {
  value: string
  onValueChange: (value: string) => void
  status: BlogSearchStatus
  /** The hits for the current value (source asks for 5, shows 4). */
  hits: readonly BlogSearchHit[]
  /** Shown under "Search error". */
  errorMessage?: string
  onNavigate?: BlogNavigate
}

const SHOWN = 4

/** As source (utils/safeCssUrl.ts): https only, normalised by the URL parser, double quotes encoded. */
export function safeCssUrl(url: string): string {
  try {
    const parsed = new URL(url)
    return parsed.protocol === 'https:' ? parsed.toString().replace(/"/g, '%22') : ''
  } catch {
    return ''
  }
}
export const searchPageHref = (q: string) => `/blog/search?q=${encodeURIComponent(q)}`

export function BlogSearch({
  value,
  onValueChange,
  status,
  hits,
  errorMessage,
  onNavigate
}: BlogSearchProps) {
  const listId = useId()
  const [selected, setSelected] = useState(-1)
  const open = value.trim().length > 2
  const shown = status === 'ready' ? hits.slice(0, SHOWN) : []
  const more = status === 'ready' && hits.length > SHOWN

  const close = () => {
    onValueChange('')
    setSelected(-1)
  }
  const go = (href: string) => {
    if (onNavigate) onNavigate(href)
    else window.location.assign(href)
  }

  const onKeyDown = (event: KeyboardEvent<HTMLInputElement>) => {
    // As source: the selection cycles over the hits and the "more" entry (index 4).
    const last = hits.length > 3 ? SHOWN : hits.length
    if (event.key === 'ArrowUp') {
      event.preventDefault()
      setSelected((index) => (index <= 0 ? last : index - 1))
    } else if (event.key === 'ArrowDown') {
      event.preventDefault()
      setSelected((index) => (index >= last ? 0 : index + 1))
    } else if (event.key === 'Enter') {
      event.preventDefault()
      const hit = hits[selected]
      if (selected === SHOWN || selected === -1) go(searchPageHref(value))
      else if (selected >= 0 && hit) go(hit.url)
      close()
    } else if (event.key === 'Escape') {
      close()
    }
  }

  const optionId = (index: number) => `${listId}-${index}`
  const active =
    open && ((selected >= 0 && selected < shown.length) || (selected === SHOWN && more))
      ? optionId(selected)
      : undefined

  return (
    <div className={`${styles.container} ${open ? styles.open : ''}`}>
      {open ? <div className={styles.overlay} onClick={close} aria-hidden="true" /> : null}
      <div className={styles.inputBox}>
        <input
          className={styles.input}
          type="text"
          role="combobox"
          aria-label="Search the blog"
          aria-expanded={open}
          aria-controls={open ? listId : undefined}
          aria-activedescendant={active}
          aria-autocomplete="list"
          value={value}
          onChange={(event) => onValueChange(event.target.value)}
          onKeyDown={onKeyDown}
          placeholder="Search..."
        />
        {value ? (
          <button type="button" className={styles.clear} onClick={close} aria-label="Clear search">
            ×
          </button>
        ) : null}
      </div>
      {open ? (
        <ul className={styles.results} id={listId} role="listbox" aria-label="Search suggestions">
          {status === 'loading' ? (
            <li className={styles.notice} role="presentation">
              <span className={styles.noticeText}>Searching...</span>
            </li>
          ) : null}
          {status === 'error' ? (
            <li className={styles.notice} role="presentation">
              <div className={styles.noticeIcon} aria-hidden="true">
                ⚠️
              </div>
              <strong className={styles.noticeTitle}>Search error</strong>
              <span className={styles.noticeText}>
                {errorMessage ?? 'Something went wrong. Please try again.'}
              </span>
            </li>
          ) : null}
          {status === 'ready' && hits.length === 0 ? (
            <li className={styles.notice} role="presentation">
              <div className={styles.noticeIcon} aria-hidden="true">
                🔍
              </div>
              <strong className={styles.noticeTitle}>No matches</strong>
              <span className={styles.noticeText}>Try searching for something else</span>
            </li>
          ) : null}
          {shown.map((hit, index) => (
            <li
              key={hit.id}
              id={optionId(index)}
              role="option"
              aria-selected={index === selected}
              className={`${styles.hit} ${index === selected ? styles.selected : ''}`}
              onMouseEnter={() => setSelected(index)}
            >
              <a
                className={styles.hitLink}
                href={hit.url}
                {...inAppLink(hit.url, onNavigate)}
                onClickCapture={() => close()}
              >
                <div
                  className={styles.hitImage}
                  style={
                    safeCssUrl(hit.image)
                      ? { backgroundImage: `url("${safeCssUrl(hit.image)}")` }
                      : undefined
                  }
                />
                <div className={styles.hitText}>
                  <SearchHighlight className={styles.hitTitle} value={hit.highlightedTitle} />
                  <SearchHighlight
                    className={styles.hitDescription}
                    value={hit.highlightedDescription}
                  />
                </div>
              </a>
            </li>
          ))}
          {more ? (
            <li
              id={optionId(SHOWN)}
              role="option"
              aria-selected={selected === SHOWN}
              className={`${styles.more} ${selected === SHOWN ? styles.selected : ''}`}
              onMouseEnter={() => setSelected(SHOWN)}
            >
              <a
                className={styles.moreLink}
                href={searchPageHref(value)}
                {...inAppLink(searchPageHref(value), onNavigate)}
                onClickCapture={() => close()}
              >
                See more results
              </a>
            </li>
          ) : null}
        </ul>
      ) : null}
    </div>
  )
}
