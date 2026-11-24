import { useId, type KeyboardEventHandler, type ReactNode, type Ref } from 'react'
import { t } from '../../../shop/discovery/intl'
import { GeneralClose, GeneralSearch } from '../../foundations/icons/general'
import styles from './ShopSearch.module.css'

export type ShopSearchRow = {
  id: string
  kind: 'facet' | 'item' | 'collection' | 'creator' | 'see-all' | 'recent' | 'popular'
  section: string
  label: ReactNode
  title?: string
  sub?: string
  thumb?: ReactNode
  activate: (via: 'click' | 'keyboard') => void
}

type ShopSearchProps = {
  inputRef: Ref<HTMLInputElement>
  value: string
  open: boolean
  active: number
  recent: string[]
  rows: ShopSearchRow[]
  loading: boolean
  failed: boolean
  empty: boolean
  onOpenChange: (open: boolean) => void
  onChange: (value: string) => void
  onSubmit: (value: string) => void
  onKeyDown: KeyboardEventHandler<HTMLInputElement>
  onClear: () => void
  onClearRecent: () => void
  onRemoveRecent: (term: string) => void
  onRetry: () => void
}

export function ShopSearch({
  inputRef,
  value,
  open,
  active,
  recent,
  rows,
  loading,
  failed,
  empty,
  onOpenChange,
  onChange,
  onSubmit,
  onKeyDown,
  onClear,
  onClearRecent,
  onRemoveRecent,
  onRetry
}: ShopSearchProps) {
  const listboxId = useId()
  const query = value.trim()
  const sections: [string, string][] = query
    ? [
        ['facets', t('search.explore')],
        ['items', t('search.items')],
        ['collections', t('search.collections')],
        ['creators', t('search.creators')],
        ['see-all', '']
      ]
    : [
        ['recent', t('search.recent')],
        ['popular', t('search.popular')]
      ]
  return (
    <div
      className={styles.root + ' shop-search'}
      data-testid="search"
      onBlur={(e) => !e.currentTarget.contains(e.relatedTarget as Node) && onOpenChange(false)}
    >
      <form
        role="search"
        onSubmit={(e) => {
          e.preventDefault()
          onSubmit(value)
        }}
      >
        <GeneralSearch
          aria-hidden="true"
          size={20}
          className={styles.icon}
        />
        <input
          ref={inputRef}
          type="search"
          role="combobox"
          aria-label={t('search.label')}
          aria-expanded={open}
          aria-controls={listboxId}
          aria-autocomplete="list"
          aria-activedescendant={active >= 0 ? rows[active]?.id : undefined}
          placeholder={t('search.placeholder')}
          autoComplete="off"
          spellCheck={false}
          value={value}
          onChange={(e) => {
            onChange(e.target.value)
            onOpenChange(true)
          }}
          onFocus={() => onOpenChange(true)}
          onKeyDown={onKeyDown}
          data-testid="search-input"
        />
        {value ? (
          <button
            type="button"
            className={styles.clear}
            onClick={onClear}
            aria-label={t('search.clear')}
          >
            <GeneralClose aria-hidden="true" size={16} />
          </button>
        ) : null}
      </form>
      {open ? (
        <div
          className={styles.panel}
          data-testid="search-panel"
          onMouseDown={(e) => (e.target as HTMLElement).closest('button') || e.preventDefault()}
        >
          {loading ? (
            <p className={styles.empty} role="status">
              {t('search.searching')}
            </p>
          ) : null}
          {failed ? (
            <p className={styles.empty} role="alert">
              {t('search.error')}{' '}
              <button type="button" className={styles.link} onClick={onRetry}>
                {t('search.retry')}
              </button>
            </p>
          ) : null}
          {empty ? (
            <p className={styles.empty} role="status">
              {t('search.noEntities', { query })}
            </p>
          ) : null}
          {!query && recent.length > 0 ? (
            <div className={styles.head}>
              <span>{t('search.recent')}</span>
              <button type="button" className={styles.link} onClick={onClearRecent}>
                {t('search.clearRecent')}
              </button>
            </div>
          ) : null}
          <div className={query ? undefined : styles.recentarea}>
            <div id={listboxId} role="listbox" aria-label={t('search.suggestions')}>
              {sections.map(([section, heading]) => {
                const inSection = rows
                  .map((r, i) => [r, i] as const)
                  .filter(([r]) => r.section === section)
                if (inSection.length === 0) return null
                const option = (row: ShopSearchRow, i: number) => ({
                  id: row.id,
                  role: 'option' as const,
                  'aria-selected': i === active,
                  'data-active': i === active || undefined,
                  'data-kind': row.kind,
                  onClick: () => row.activate('click')
                })
                if (section === 'recent') {
                  return (
                    <div key={section} role="group" aria-label={heading}>
                      {inSection.map(([row, i]) => (
                        <div key={row.id} className={styles.recent} {...option(row, i)}>
                          <GeneralSearch
                            aria-hidden="true"
                            size={16}
                            className={styles.recenticon}
                          />
                          <span className={styles.recenttext}>{row.label}</span>
                        </div>
                      ))}
                    </div>
                  )
                }
                if (section === 'popular') {
                  return (
                    <div key={section} role="group" aria-label={heading}>
                      <div className={styles.head} role="presentation">
                        <span>{heading}</span>
                      </div>
                      <div className={styles.chips} role="none">
                        {inSection.map(([row, i]) => (
                          <span key={row.id} className={styles.chip} {...option(row, i)}>
                            {row.label}
                          </span>
                        ))}
                      </div>
                    </div>
                  )
                }
                if (section === 'see-all') {
                  return inSection.map(([row, i]) => (
                    <div key={row.id} className={styles.seeall} {...option(row, i)}>
                      {row.label}
                    </div>
                  ))
                }
                return (
                  <div key={section} role="group" aria-label={heading}>
                    <div className={styles.head} role="presentation">
                      <span>{heading}</span>
                    </div>
                    {inSection.map(([row, i]) => (
                      <div key={row.id} className={styles.row} {...option(row, i)}>
                        {row.thumb}
                        <span className={styles.text}>
                          <span className={styles.name} title={row.title}>
                            {row.label}
                          </span>
                          {row.sub ? <span className={styles.sub}>{row.sub}</span> : null}
                        </span>
                      </div>
                    ))}
                  </div>
                )
              })}
            </div>
            {/* The removals are controls, not options: beside the listbox, one per recent row, in the tab order. */}
            {!query && recent.length > 0 ? (
              <ul className={styles.removes} aria-label={t('search.recent')}>
                {recent.map((term) => (
                  <li key={term}>
                    <button
                      type="button"
                      className={styles.remove}
                      aria-label={t('search.removeRecent', { query: term })}
                      onClick={() => {
                        onRemoveRecent(term)
                      }}
                    >
                      <GeneralClose aria-hidden="true" size={14} />
                    </button>
                  </li>
                ))}
              </ul>
            ) : null}
          </div>
        </div>
      ) : null}
    </div>
  )
}
