import type { ReactNode } from 'react'
import { GeneralDecentraland } from '../../../foundations/icons'
import crew from '../assets/errors/crew.webp'
import locked from '../assets/errors/locked.webp'
import lost from '../assets/errors/lost.webp'
import wave from '../assets/errors/wave.webp'
import styles from './StatusPage.module.css'
import { ART_FILES, isRetryable, LOGOMARK_VIEW_BOX, statusCopy, type ErrorArt } from './statusCopy'

const ART_URLS: Record<ErrorArt, string> = { crew, locked, lost, wave }

export type StatusPageProps = {
  status: number
  /** Overrides the default copy for this status. */
  title?: string
  message?: string
  /** Technical detail (status text, stack, response body); shown only when isDev. */
  detail?: string
  isDev?: boolean
  homeHref?: string
  /** Replaces the default actions (home, plus retry or back). */
  actions?: ReactNode
}

/** The branded screen for HTTP errors (4xx and 5xx). */
export function StatusPage({
  status,
  title,
  message,
  detail,
  isDev = false,
  homeHref = '/',
  actions
}: StatusPageProps) {
  const copy = statusCopy(status)
  const art = ART_FILES[copy.art]
  return (
    <main className={styles.page} aria-labelledby="sp-title" data-status={status}>
      <div className={styles.card}>
        <a className={styles.logo} href={homeHref}>
          <GeneralDecentraland
            className={styles.logoMark}
            size={28}
            viewBox={LOGOMARK_VIEW_BOX}
            aria-hidden="true"
          />
          <span>Decentraland</span>
        </a>
        <img
          className={styles.art}
          src={ART_URLS[copy.art]}
          width={art.width}
          height={art.height}
          alt=""
        />
        <p className={styles.code}>
          {status} · {copy.eyebrow}
        </p>
        <h1 id="sp-title" className={styles.title}>
          {title ?? copy.title}
        </h1>
        <p className={styles.message}>{message ?? copy.message}</p>
        <nav className={styles.actions} aria-label="Error page navigation">
          {actions ?? (
            <>
              <a className={`${styles.button} ${styles.primary}`} href={homeHref}>
                Go to homepage
              </a>
              {isRetryable(status) ? (
                <button
                  type="button"
                  className={`${styles.button} ${styles.secondary}`}
                  onClick={() => window.location.reload()}
                >
                  Try again
                </button>
              ) : (
                <button
                  type="button"
                  className={`${styles.button} ${styles.secondary}`}
                  onClick={() => window.history.back()}
                >
                  Go back
                </button>
              )}
            </>
          )}
        </nav>
        {isDev && detail ? (
          <details className={styles.dev} open>
            <summary className={styles.devSummary}>Developer details (dev only)</summary>
            <pre className={styles.devDetail}>{detail}</pre>
          </details>
        ) : null}
      </div>
    </main>
  )
}
