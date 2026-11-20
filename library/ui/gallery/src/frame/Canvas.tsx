import { Component, useEffect, type ErrorInfo, type ReactNode } from 'react'
import { entries } from '../examples'
import { foundationPages } from '../foundations/FoundationPages'
import type { Example } from '../registry'
import { HEIGHT_MESSAGE } from '../messages'
import styles from './Canvas.module.css'

class ExampleBoundary extends Component<{ children: ReactNode }, { error: Error | null }> {
  state = { error: null as Error | null }
  static getDerivedStateFromError(error: Error) {
    return { error }
  }
  componentDidCatch(error: Error, info: ErrorInfo) {
    // Keep the failure visible in the console as well, where the build check reads it.
    console.error(error, info.componentStack)
  }
  render() {
    return this.state.error ? (
      <p role="alert" data-example-error className={styles.error}>
        {this.state.error.message}
      </p>
    ) : (
      this.props.children
    )
  }
}

function ExampleView({ example }: { example: Example }) {
  return (
    <section
      className={styles.example}
      data-example={example.id}
      data-surface={example.surface ?? 'dark'}
    >
      <h2 className={styles.title}>{example.title}</h2>
      {example.note ? <p className={styles.note}>{example.note}</p> : null}
      <div className={styles.stage} data-stage>
        <ExampleBoundary>{example.render()}</ExampleBoundary>
      </div>
    </section>
  )
}

// The canvas runs inside the gallery's iframe; the iframe's width is the viewport media queries read.
function useReportHeight() {
  useEffect(() => {
    if (window.parent === window) return
    const report = () =>
      window.parent.postMessage(
        { type: HEIGHT_MESSAGE, height: document.documentElement.scrollHeight },
        window.location.origin
      )
    const observer = new ResizeObserver(report)
    observer.observe(document.body)
    report()
    return () => observer.disconnect()
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
    <main className={styles.canvas} data-component={page.name}>
      {page.examples.map((example) => (
        <ExampleView key={example.id} example={example} />
      ))}
    </main>
  )
}
