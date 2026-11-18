import { Component, type ErrorInfo, type ReactNode } from 'react'
import { useTranslation } from '~/intl'
import { track } from '~/lib/analytics'
import { captureError } from '~/lib/monitoring'
import styles from './ErrorBoundary.module.css'

type Props = { children: ReactNode }
type State = { hasError: boolean }

/** Catches render and chunk-load errors below it (e.g. a stale deploy) and offers a reload. */
export class ErrorBoundary extends Component<Props, State> {
  state: State = { hasError: false }

  static getDerivedStateFromError(): State {
    return { hasError: true }
  }

  componentDidCatch(error: unknown, info: ErrorInfo) {
    captureError(error, { flow: 'render', component_stack: info.componentStack })
    track('Error page')
  }

  render() {
    return this.state.hasError ? <ErrorFallback /> : this.props.children
  }
}

// React.lazy caches a failed chunk import, so recovery is a full reload rather than a re-render.
function ErrorFallback() {
  const { t } = useTranslation()
  return (
    <section className={styles.panel} data-testid="error-boundary">
      <h2 className={styles.title}>{t('error_boundary.title')}</h2>
      <p className={styles.text}>{t('error_boundary.description')}</p>
      <button
        className={styles.retryButton}
        type="button"
        data-testid="error-boundary-retry"
        onClick={() => window.location.reload()}
      >
        {t('error_boundary.retry')}
      </button>
    </section>
  )
}
