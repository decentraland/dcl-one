import { useCallback, useEffect, useRef, useState } from 'react'
import { breakpoints } from '@dcl/one-ui'
import { entries } from '../examples'
import { foundationPages } from '../foundations/FoundationPages'
import { HEIGHT_MESSAGE } from '../messages'
import {
  MAX_WIDTH,
  MIN_WIDTH,
  canvasHref,
  galleryHref,
  generatedHref,
  parseRoute,
  widthPresets,
  type Route,
  type Width
} from '../url'
import styles from './Shell.module.css'

const componentNames = entries.map((entry) => entry.name)
// Exports shown on a page named after another export open by their own name too.
const aliases = Object.fromEntries(
  entries.flatMap((entry) =>
    entry.components.filter((name) => name !== entry.name).map((name) => [name, entry.name])
  )
)
const foundationNames = foundationPages.map((page) => page.name)
const presets = widthPresets(breakpoints)
const groups = [...new Set(entries.map((entry) => entry.group))]

const figmaNames = new Set(__GENERATED_COMPONENTS__.map((component) => component.name))
const runtimeNames = new Set(entries.flatMap((entry) => [entry.name, ...entry.components]))

/** In-app navigation: the sidebar stays mounted, so its scroll and filter survive a selection. */
type Navigate = (href: string) => void

function SourceBadge({ label }: { label: string }) {
  return <span className={styles.badge}>{label}</span>
}

function NavLink({
  href,
  current,
  label,
  source,
  badge,
  navigate
}: {
  href: string
  current: boolean
  label: string
  source: 'runtime' | 'generated'
  /** Shown only for a name both sources have. */
  badge?: string
  navigate: Navigate
}) {
  return (
    <li>
      <a
        className={styles.navLink}
        href={href}
        data-source={source}
        aria-current={current ? 'page' : undefined}
        onClick={(event) => {
          // A modified or non-primary click keeps the browser's own behaviour (new tab, new window).
          if (
            event.defaultPrevented ||
            event.button !== 0 ||
            event.metaKey ||
            event.ctrlKey ||
            event.shiftKey ||
            event.altKey
          )
            return
          event.preventDefault()
          navigate(href)
        }}
      >
        <span>{label}</span>
        {badge ? <SourceBadge label={badge} /> : null}
      </a>
    </li>
  )
}

function Navigation({ route, navigate }: { route: Route; navigate: Navigate }) {
  const [filter, setFilter] = useState('')
  const matches = (...names: readonly string[]) =>
    names.some((name) => name.toLowerCase().includes(filter.trim().toLowerCase()))
  const width = route.source === 'runtime' ? route.width : undefined
  const isCurrent = (view: string, name: string) =>
    route.source === 'runtime' && route.view === view && route.name === name
  const generatedName =
    route.source === 'generated' ? new URLSearchParams(route.query).get('component') : null
  const figma = __GENERATED_COMPONENTS__.filter((component) => matches(component.name))
  return (
    <nav className={styles.nav} aria-label="Gallery">
      <input
        className={styles.filter}
        type="search"
        aria-label="Filter"
        placeholder="Filter"
        value={filter}
        onChange={(event) => setFilter(event.target.value)}
      />
      <h2 className={styles.navHeading}>Foundations</h2>
      <ul className={styles.navList}>
        {foundationPages
          .filter((page) => matches(page.title))
          .map((page) => (
            <NavLink
              key={page.name}
              href={galleryHref({ foundations: page.name, width })}
              current={isCurrent('foundations', page.name)}
              label={page.title}
              source="runtime"
              navigate={navigate}
            />
          ))}
      </ul>
      <h2 className={styles.navHeading}>Components</h2>
      <p className={styles.navNote}>What apps import from @dcl/one-ui.</p>
      {groups.map((group) => {
        const items = entries.filter(
          (entry) => entry.group === group && matches(entry.name, ...entry.components)
        )
        return items.length ? (
          <section key={group}>
            <h3 className={styles.groupHeading}>{group}</h3>
            <ul className={styles.navList}>
              {items.map((entry) => (
                <NavLink
                  key={entry.name}
                  href={galleryHref({ component: entry.name, width })}
                  current={isCurrent('component', entry.name)}
                  label={entry.name}
                  source="runtime"
                  badge={figmaNames.has(entry.name) ? 'runtime' : undefined}
                  navigate={navigate}
                />
              ))}
            </ul>
          </section>
        ) : null
      })}
      <details className={styles.reference} open={route.source === 'generated' || undefined}>
        <summary className={styles.navHeading}>Figma reference</summary>
        <p className={styles.navNote}>
          Auto-generated previews of the Figma design, for comparison. Apps never use them.
        </p>
        <ul className={styles.navList}>
          {figma.map((component) => (
            <NavLink
              key={component.name}
              href={galleryHref({ source: 'generated', component: component.name })}
              current={generatedName === component.name}
              label={component.name}
              source="generated"
              badge={runtimeNames.has(component.name) ? 'Figma' : undefined}
              navigate={navigate}
            />
          ))}
          <NavLink
            href={galleryHref({ source: 'generated', foundations: 'tokens' })}
            current={route.source === 'generated' && generatedName === null}
            label="Foundations"
            source="generated"
            badge="Figma"
            navigate={navigate}
          />
        </ul>
      </details>
    </nav>
  )
}

function WidthControl({
  route,
  navigate
}: {
  route: Extract<Route, { source: 'runtime' }>
  navigate: Navigate
}) {
  const go = (width: Width) => navigate(galleryHref({ [route.view]: route.name, width }))
  const preset = presets.find((item) => item.width === route.width)
  return (
    <div className={styles.toolbar}>
      <label className={styles.control}>
        <span>Width</span>
        <select
          value={preset?.id ?? 'custom'}
          onChange={(event) => {
            const next = presets.find((item) => item.id === event.target.value)
            if (next) go(next.width)
          }}
        >
          {presets.map((item) => (
            <option key={item.id} value={item.id}>
              {item.label}
            </option>
          ))}
          {preset ? null : <option value="custom">{`custom ${route.width}px`}</option>}
        </select>
      </label>
      <form
        className={styles.control}
        onSubmit={(event) => {
          event.preventDefault()
          const value = new FormData(event.currentTarget).get('width')
          go(Number(value))
        }}
      >
        <label className={styles.control}>
          <span>Custom px</span>
          <input
            name="width"
            type="number"
            min={MIN_WIDTH}
            max={MAX_WIDTH}
            required
            defaultValue={typeof route.width === 'number' ? route.width : ''}
          />
        </label>
        <button type="submit">Apply</button>
      </form>
      <a href={canvasHref(route.view, route.name)} target="_blank" rel="noreferrer">
        Open canvas
      </a>
    </div>
  )
}

function RuntimeView({
  route,
  navigate
}: {
  route: Extract<Route, { source: 'runtime' }>
  navigate: Navigate
}) {
  const frame = useRef<HTMLIFrameElement>(null)
  const [height, setHeight] = useState(480)
  useEffect(() => {
    const receive = (event: MessageEvent) => {
      if (event.source !== frame.current?.contentWindow) return
      const data = event.data as { type?: unknown; height?: unknown }
      if (data?.type === HEIGHT_MESSAGE && typeof data.height === 'number')
        setHeight(Math.max(240, Math.ceil(data.height)))
    }
    window.addEventListener('message', receive)
    return () => window.removeEventListener('message', receive)
  }, [])
  const entry = entries.find((item) => item.name === route.name)
  const page = foundationPages.find((item) => item.name === route.name)
  const title = route.view === 'component' ? route.name : (page?.title ?? route.name)
  return (
    <>
      <header className={styles.header}>
        {route.alias ? (
          <p className={styles.summary}>
            {route.alias} is shown on the {route.name} page.
          </p>
        ) : null}
        {route.missing ? (
          <p role="alert" className={styles.missing}>
            No runtime page named “{route.missing}”; showing {title}.
          </p>
        ) : null}
        <h1 className={styles.title}>
          {title}
          {figmaNames.has(title) ? <SourceBadge label="runtime" /> : null}
        </h1>
        <p className={styles.summary}>{entry?.summary ?? page?.summary}</p>
        {entry ? (
          <p className={styles.exports}>
            Exports shown:{' '}
            {entry.components.map((name) => (
              <code key={name}>{name}</code>
            ))}
          </p>
        ) : null}
        <WidthControl route={route} navigate={navigate} />
      </header>
      <div className={styles.viewport} data-width={route.width}>
        <iframe
          ref={frame}
          className={styles.canvas}
          title={`${title} canvas`}
          src={canvasHref(route.view, route.name)}
          style={{ width: route.width === 'fill' ? '100%' : `${route.width}px`, height }}
        />
      </div>
    </>
  )
}

function GeneratedView({ query }: { query: string }) {
  const href = generatedHref(__GENERATED_REVIEW_URL__, query)
  const name = new URLSearchParams(query).get('component')
  return (
    <>
      <header className={styles.header}>
        <h1 className={styles.title}>
          {name ?? 'Foundations'} <SourceBadge label="Figma reference" />
        </h1>
        <p className={styles.summary}>
          An auto-generated preview of the Figma design, for comparison only: apps never use it.
          {name && runtimeNames.has(name) ? ` Apps use the ${name} component instead.` : ''}{' '}
          <a href={href}>Open it on its own</a>.
        </p>
      </header>
      <iframe className={styles.generated} title="Figma reference" src={href} />
    </>
  )
}

export function Shell({ search: initial }: { search: string }) {
  const [search, setSearch] = useState(initial)
  useEffect(() => {
    const restore = () => setSearch(window.location.search)
    window.addEventListener('popstate', restore)
    return () => window.removeEventListener('popstate', restore)
  }, [])
  const navigate = useCallback<Navigate>((href) => {
    const url = new URL(href, window.location.href)
    window.history.pushState(null, '', url)
    setSearch(url.search)
  }, [])
  const route = parseRoute(search, componentNames, foundationNames, aliases)
  return (
    <div className={styles.layout}>
      <Navigation route={route} navigate={navigate} />
      <main className={styles.main}>
        {route.source === 'generated' ? (
          <GeneratedView key={route.query} query={route.query} />
        ) : (
          <RuntimeView key={`${route.view}:${route.name}`} route={route} navigate={navigate} />
        )}
      </main>
    </div>
  )
}
