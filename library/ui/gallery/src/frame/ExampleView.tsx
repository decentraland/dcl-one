import { Component, type ErrorInfo, type ReactNode } from 'react'
import type { Example } from '../registry'
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

/** One example on a canvas: its title and note over a stage that keeps a failure to itself. */
export function ExampleView({ example }: { example: Example }) {
  // Rendered as an element inside the boundary, so a throw in the render function lands there too.
  const Render = example.render
  return (
    <section
      className={styles.example}
      data-example={example.id}
      data-surface={example.surface ?? 'dark'}
    >
      <h2 className={styles.title}>{example.title}</h2>
      {example.note ? <p className={styles.note}>{example.note}</p> : null}
      <div className={styles.stage} data-stage>
        <ExampleBoundary>
          <Render />
        </ExampleBoundary>
      </div>
    </section>
  )
}
