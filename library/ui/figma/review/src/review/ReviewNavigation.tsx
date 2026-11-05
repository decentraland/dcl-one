import { useState } from 'react'
import { useSelector } from '@tanstack/react-store'
import data from './catalog.json'
import { reviewStore } from './review.store'
import { sections } from './Foundations'
import { matchesPlatform, platforms, type Platform } from './navigation'
import styles from './ReviewGallery.module.css'

function ReviewNavigation({
  selected,
  foundation
}: {
  selected: string
  foundation: string | null
}) {
  const state = useSelector(reviewStore, (value) => value)
  const [expandedPages, setExpandedPages] = useState<string[]>([])
  const search = state.navigationSearch.trim().toLowerCase()
  const components = data.components.filter(
    (component) =>
      component.variants.some((variant) => matchesPlatform(variant, state.platform)) &&
      `${component.page} ${component.name}`.toLowerCase().includes(search)
  )
  const link = (params: Record<string, string>) =>
    '/dcl-ui-review/?' +
    new URLSearchParams({
      platform: state.platform,
      component: selected,
      variant: new URLSearchParams(location.search).get('variant') ?? '0',
      ...params
    })
  return (
    <aside className={styles.sidebar}>
      <h1>DCL One UI</h1>
      <label className={styles.navigationControl}>
        Browse
        <select
          name="browse"
          value={foundation ? 'foundations' : 'components'}
          onChange={(event) => {
            location.href =
              event.target.value === 'foundations'
                ? link({ foundations: 'tokens' })
                : link({ component: selected })
          }}
        >
          <option value="components">Components</option>
          <option value="foundations">Foundations</option>
        </select>
      </label>
      {!foundation && (
        <label className={styles.navigationControl}>
          Platform
          <select
            name="platform"
            value={state.platform}
            onChange={(event) => {
              const platform = event.target.value as Platform
              const params = new URLSearchParams(location.search)
              params.set('platform', platform)
              const first = data.components.find((component) =>
                component.variants.some((variant) => matchesPlatform(variant, platform))
              )
              if (first) {
                params.set('component', first.name)
                params.set(
                  'variant',
                  String(first.variants.findIndex((variant) => matchesPlatform(variant, platform)))
                )
              }
              location.href = '/dcl-ui-review/?' + params
            }}
          >
            {platforms.map((platform) => (
              <option key={platform} value={platform}>
                {platform === 'all'
                  ? 'All platforms'
                  : platform === 'web'
                    ? 'Web'
                    : platform === 'explorer'
                      ? 'Explorer (engine unconfirmed)'
                      : platform === 'unclassified'
                        ? 'Not classified'
                        : platform === 'bevy'
                          ? 'Bevy'
                          : 'Unity'}
              </option>
            ))}
          </select>
        </label>
      )}
      <label className={styles.navigationControl}>
        Search
        <input
          name="navigation-search"
          type="search"
          placeholder={foundation ? 'Search foundations' : 'Search components'}
          value={state.navigationSearch}
          onChange={(event) =>
            reviewStore.setState((value) => ({ ...value, navigationSearch: event.target.value }))
          }
        />
      </label>
      {!foundation && (
        <button
          className={styles.controlButton}
          type="button"
          onClick={() =>
            setExpandedPages(
              components.every((component) => expandedPages.includes(component.page))
                ? []
                : [...new Set(components.map((component) => component.page))]
            )
          }
        >
          {components.length > 0 &&
          components.every((component) => expandedPages.includes(component.page))
            ? 'Collapse all'
            : 'Expand all'}
        </button>
      )}
      <nav
        className={styles.navigation}
        aria-label={foundation ? 'Foundations navigation' : 'Component navigation'}
      >
        {foundation
          ? sections
              .filter((name) => name.includes(search))
              .map((name) => (
                <a
                  key={name}
                  className={foundation === name ? styles.selected : undefined}
                  href={link({ foundations: name })}
                >
                  {name}
                </a>
              ))
          : [...new Set(components.map((component) => component.page))].map((page) => (
              <details
                key={page}
                open={expandedPages.includes(page)}
                onToggle={(event) => {
                  const open = event.currentTarget.open
                  setExpandedPages((pages) =>
                    open
                      ? pages.includes(page)
                        ? pages
                        : [...pages, page]
                      : pages.filter((item) => item !== page)
                  )
                }}
              >
                <summary>{page}</summary>
                {components
                  .filter((component) => component.page === page)
                  .map((component) => (
                    <a
                      key={component.name}
                      className={selected === component.name ? styles.selected : undefined}
                      href={link({
                        component: component.name,
                        variant: String(
                          component.variants.findIndex((variant) =>
                            matchesPlatform(variant, state.platform)
                          )
                        )
                      })}
                    >
                      {component.name}
                      <small>
                        {
                          component.variants.filter((variant) =>
                            matchesPlatform(variant, state.platform)
                          ).length
                        }{' '}
                        variants
                      </small>
                    </a>
                  ))}
              </details>
            ))}
      </nav>
      {!foundation && components.length === 0 && <p>No matching captured components.</p>}
      {foundation && <p>Shared captured foundations; no engine-specific split is verified.</p>}
    </aside>
  )
}
export { ReviewNavigation }
