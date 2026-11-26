import { useEffect } from 'react'
import { COMPONENT_OVERVIEW } from '../componentPages'
import { EmbeddedCanvas } from '../embedded'
import { entries } from '../examples'
import { foundationPages } from '../foundations/FoundationPages'
import { HEIGHT_MESSAGE } from '../messages'
import styles from './Canvas.module.css'
import { ComponentShowcase } from './ComponentShowcase'
import { ExampleView } from './ExampleView'

// The canvas runs inside the gallery's iframe; the iframe's width is the viewport media queries read.
function useReportHeight() {
  useEffect(() => {
    if (window.parent === window) return
    const report = () =>
      window.parent.postMessage(
        { type: HEIGHT_MESSAGE, height: document.documentElement.scrollHeight },
        window.location.origin
      )
    // The gallery sizes the frame to this document, so the canvas never scrolls itself. A scrollbar of its own would
    // narrow the viewport the examples are drawn in, and then keep itself on a page whose pictures grow with the
    // width: the frame takes the narrower page's height, which the full width no longer fits.
    document.documentElement.style.overflowY = 'hidden'
    const observer = new ResizeObserver(report)
    observer.observe(document.body)
    report()
    return () => {
      observer.disconnect()
      document.documentElement.style.overflowY = ''
    }
  }, [])
}

export function Canvas({ search }: { search: string }) {
  useReportHeight()
  const params = new URLSearchParams(search)
  const foundation = params.get('foundations')
  const page =
    foundation !== null
      ? foundationPages.find((item) => item.name === foundation)
      : entries.find((item) => item.name === params.get('component'))
  useEffect(() => {
    document.body.style.minHeight = page && 'minHeight' in page ? `${page.minHeight}px` : ''
  }, [page])
  if (foundation === null && params.get('component') === COMPONENT_OVERVIEW)
    return <ComponentShowcase />
  if (!page)
    return (
      <p role="alert" className={styles.error}>
        No gallery page matches {search || 'an empty query'}.
      </p>
    )
  if ('render' in page)
    return (
      <main className={styles.canvas} data-foundations={page.name}>
        {page.render()}
      </main>
    )
  return (
    <EmbeddedCanvas value={params.get('embedded') === '1'}>
      <main className={styles.canvas} data-component={page.name}>
        {page.examples.map((example) => (
          <ExampleView key={example.id} example={example} />
        ))}
      </main>
    </EmbeddedCanvas>
  )
}
