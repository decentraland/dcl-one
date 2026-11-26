import { useRef, useSyncExternalStore, type MouseEvent } from 'react'
import { useCanvasHeight } from '../canvasHeight'
import { COMPONENT_OVERVIEW, componentGroups, type ComponentGroup } from '../componentPages'
import { OPEN_MESSAGE } from '../messages'
import type { GalleryEntry } from '../registry'
import { canvasHref, galleryHref, parseWidth, type Width } from '../url'
import styles from './ComponentShowcase.module.css'
import { ExampleView } from './ExampleView'

// The gallery resizes the canvas when the reviewer picks another width, so a resize is when the chosen width changes.
function onResize(notify: () => void) {
  window.addEventListener('resize', notify)
  return () => window.removeEventListener('resize', notify)
}

// The width chosen in the gallery around this canvas, which its URL carries. A canvas open on its own has none: its
// width is the window's.
function chosenWidth(): Width {
  if (window.parent === window) return 'fill'
  try {
    return parseWidth(new URLSearchParams(window.parent.location.search).get('width'))
  } catch {
    // A page of another origin embeds the canvas: its URL is out of reach.
    return 'fill'
  }
}

// Inside the gallery the page opens in place, keeping the sidebar and the chosen width. A modified click, or a
// canvas opened on its own, follows the link to the gallery page.
function openInGallery(event: MouseEvent<HTMLAnchorElement>, name: string) {
  if (
    window.parent === window ||
    event.button !== 0 ||
    event.metaKey ||
    event.ctrlKey ||
    event.shiftKey ||
    event.altKey
  )
    return
  event.preventDefault()
  window.parent.postMessage({ type: OPEN_MESSAGE, component: name }, window.location.origin)
}

/**
 * A page whose content is positioned against the viewport (a dialog, a drawer): its own canvas, so it keeps a
 * viewport to itself instead of covering the pages around it. It is an embedded canvas (`embedded=1`), where an
 * example that opens on load starts closed.
 */
function PageCanvas({ entry }: { entry: GalleryEntry }) {
  const frame = useRef<HTMLIFrameElement>(null)
  const height = useCanvasHeight(frame, entry.minHeight ?? 240)
  return (
    <iframe
      ref={frame}
      className={styles.frame}
      title={`${entry.name} canvas`}
      src={`${canvasHref('component', entry.name)}&embedded=1`}
      loading="lazy"
      style={{ height }}
    />
  )
}

function ShowcasePage({ entry, width }: { entry: GalleryEntry; width: Width }) {
  return (
    <section className={styles.page} data-showcase-page={entry.name}>
      <header className={styles.pageHeader}>
        <h3 className={styles.pageTitle}>
          <a
            href={`./${galleryHref({ component: entry.name, width })}`}
            target="_top"
            data-gallery-link
            onClick={(event) => openInGallery(event, entry.name)}
          >
            {entry.name}
          </a>
        </h3>
        <p className={styles.pageSummary}>{entry.summary}</p>
      </header>
      {!entry.examples.length ? (
        <p className={styles.empty} data-showcase-empty>
          This page has no examples yet.
        </p>
      ) : entry.minHeight ? (
        <PageCanvas entry={entry} />
      ) : (
        entry.examples.map((example) => (
          <ExampleView key={example.id} example={example} heading="h4" />
        ))
      )}
    </section>
  )
}

/** Every component page in sequence, grouped as the sidebar groups them, each under a link to its own page. */
export function ComponentShowcase({
  groups = componentGroups
}: {
  groups?: readonly ComponentGroup[]
}) {
  // A link opened in a new tab keeps the chosen width, as the page opened in place does.
  const width = useSyncExternalStore(onResize, chosenWidth)
  return (
    <main className={styles.showcase} data-component={COMPONENT_OVERVIEW}>
      {groups.map((group) => (
        <section key={group.name} className={styles.group} data-showcase-group={group.name}>
          <h2 className={styles.groupTitle}>{group.name}</h2>
          {group.entries.map((entry) => (
            <ShowcasePage key={entry.name} entry={entry} width={width} />
          ))}
        </section>
      ))}
    </main>
  )
}
