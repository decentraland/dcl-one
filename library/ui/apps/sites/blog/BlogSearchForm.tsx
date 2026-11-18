import { useEffect, useId, useRef, useState, type KeyboardEvent, type MouseEvent } from 'react'
import { GeneralSearch } from '../../../foundations/icons/general'
import { BlogForm, BlogLink } from './BlogRouting'
import { BlogSearchResult } from './BlogSearchResult'
import {
  BLOG_QUERY_MAX,
  BLOG_QUERY_MIN,
  BLOG_SEARCH,
  blogSearchHref,
  type BlogSearchSuggestions
} from './blogModel'
import styles from './BlogSearchForm.module.css'

export type BlogSearchFormProps = {
  /** `bar` sits in the tabs menu; `outline` is the full-width mobile field. */
  variant: 'bar' | 'outline'
  query: string
  preview?: {
    onQueryChange: (query: string) => void
    suggestions: BlogSearchSuggestions | null
    loading: boolean
    failed: boolean
  }
}

function keepInputFocus(event: MouseEvent<HTMLAnchorElement>) {
  if (event.button === 0) event.preventDefault()
}

/** A GET form with an optional live preview supplied by the host. */
export function BlogSearchForm({ variant, query, preview }: BlogSearchFormProps) {
  const id = useId()
  const listId = `${id}-options`
  const form = useRef<HTMLFormElement>(null)
  const input = useRef<HTMLInputElement>(null)
  const [lastQuery, setLastQuery] = useState(query)
  const [draft, setDraft] = useState(query)
  const [showPreview, setShowPreview] = useState(false)
  const [active, setActive] = useState(-1)
  const updateQuery = preview?.onQueryChange

  if (query !== lastQuery) {
    setLastQuery(query)
    setDraft(query)
    setShowPreview(false)
    setActive(-1)
  }

  useEffect(() => {
    if (input.current && input.current.value !== query) input.current.value = query
  }, [query])

  useEffect(() => {
    if (!showPreview) return
    function onPointerDown(event: PointerEvent) {
      if (event.target instanceof Node && !form.current?.contains(event.target)) {
        setShowPreview(false)
        setActive(-1)
        updateQuery?.('')
      }
    }
    document.addEventListener('pointerdown', onPointerDown)
    return () => document.removeEventListener('pointerdown', onPointerDown)
  }, [showPreview, updateQuery])

  const current = draft.trim()
  const suggestions =
    preview?.suggestions?.query.toLowerCase() === current.toLowerCase() ? preview.suggestions : null
  const posts = suggestions?.posts ?? []
  const more = !!suggestions && suggestions.total > posts.length
  const count = posts.length + Number(more)
  const status = preview?.loading
    ? 'Searching…'
    : preview?.failed
      ? 'Search suggestions are unavailable.'
      : suggestions && !count
        ? 'No matching articles.'
        : ''
  const open = showPreview && !!(status || count)
  const expanded = open && count > 0
  const activeId = expanded && active >= 0 && active < count ? `${listId}-${active}` : undefined

  function close() {
    setShowPreview(false)
    setActive(-1)
    updateQuery?.('')
  }

  function onKeyDown(event: KeyboardEvent<HTMLInputElement>) {
    if (event.nativeEvent.isComposing) return
    if (event.key === 'Escape' && showPreview) {
      event.preventDefault()
      close()
    } else if (event.key === 'Enter' && activeId) {
      event.preventDefault()
      document.getElementById(activeId)?.click()
    } else if (
      (event.key === 'ArrowDown' || event.key === 'ArrowUp') &&
      current.length >= BLOG_QUERY_MIN
    ) {
      event.preventDefault()
      if (!showPreview) {
        setShowPreview(true)
        updateQuery?.(draft)
      }
      if (event.key === 'ArrowDown') setActive(count > 0 ? (active + 1) % count : 0)
      else if (count > 0) setActive(active <= 0 ? count - 1 : active - 1)
    }
  }

  return (
    <BlogForm
      ref={form}
      className={`${styles.form} ${styles[variant]}`}
      role="search"
      action={BLOG_SEARCH}
      method="get"
      onSubmit={preview ? close : undefined}
      onBlur={
        preview
          ? (event) => {
              if (!event.currentTarget.contains(event.relatedTarget)) close()
            }
          : undefined
      }
    >
      <label className={styles.srOnly} htmlFor={`${id}q`}>
        Search articles
      </label>
      <span className={styles.field}>
        <GeneralSearch className={styles.icon} size={20} aria-hidden="true" />
        <input
          className={`${styles.input}${preview ? ` ${styles.enhanced}` : ''}`}
          ref={input}
          id={`${id}q`}
          type="search"
          name="q"
          defaultValue={query}
          placeholder="Search articles..."
          maxLength={BLOG_QUERY_MAX}
          autoComplete="off"
          role={preview ? 'combobox' : undefined}
          aria-autocomplete={preview ? 'list' : undefined}
          aria-expanded={preview ? expanded : undefined}
          aria-controls={expanded ? listId : undefined}
          aria-activedescendant={activeId}
          onKeyDown={preview ? onKeyDown : undefined}
          onChange={
            preview
              ? (event) => {
                  const value = event.currentTarget.value
                  setDraft(value)
                  setActive(-1)
                  setShowPreview(true)
                  updateQuery?.(value)
                }
              : undefined
          }
        />
        {preview && draft && (
          <button
            type="button"
            className={styles.clear}
            aria-label="Clear search"
            onClick={() => {
              if (input.current) {
                input.current.value = ''
                input.current.focus()
              }
              setDraft('')
              close()
            }}
          >
            ×
          </button>
        )}
      </span>
      {preview && (
        <div role="status" className={styles.srOnly}>
          {open ? status : ''}
        </div>
      )}
      {preview && (
        <div className={styles.preview} hidden={!open}>
          {status && (
            <div aria-hidden="true" className={styles.status}>
              {status}
            </div>
          )}
          <div id={listId} role="listbox" aria-label="Article suggestions" hidden={!count}>
            {open &&
              posts.map((post, index) => (
                <BlogLink
                  className={styles.option}
                  href={post.href}
                  key={post.slug}
                  role="option"
                  id={`${listId}-${index}`}
                  aria-selected={active === index}
                  tabIndex={-1}
                  onMouseDown={keepInputFocus}
                  onClick={close}
                >
                  <BlogSearchResult post={post} query={current} />
                </BlogLink>
              ))}
            {open && more && (
              <BlogLink
                className={`${styles.option} ${styles.more}`}
                href={blogSearchHref(current)}
                role="option"
                id={`${listId}-${posts.length}`}
                aria-selected={active === posts.length}
                tabIndex={-1}
                onMouseDown={keepInputFocus}
                onClick={close}
              >
                See more results
              </BlogLink>
            )}
          </div>
        </div>
      )}
    </BlogForm>
  )
}
