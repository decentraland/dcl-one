import type { CSSProperties, MouseEvent, ReactNode } from 'react'
import { GeneralDecentralandColor } from '../../../foundations/icons'
import dclLogo from '../assets/dcl-logo.svg'
import externalIcon from '../assets/download-steps/external-link-icon.svg'
import highlightIcon from '../assets/download-steps/highlight-icon.webp'
import styles from './DownloadSuccess.module.css'

type DownloadSuccessStep = {
  title: ReactNode
  text: ReactNode
  image?: string
  /** What the screenshot shows; empty when it only repeats the step text. */
  imageAlt?: string
  /** `contain` for screenshots that must show whole (the macOS install window). */
  imageFit?: 'cover' | 'contain'
  /** Center of the browser's download button in the screenshot, in percent of its size. */
  highlight?: { x: number; y: number }
}

type DownloadSuccessFooterLine = {
  /** The line's text, placing the rendered link (for "Need help? Go to {link}" style copy). */
  text: (link: ReactNode) => ReactNode
  link: {
    label: ReactNode
    href: string
    external?: boolean
    onClick?: (event: MouseEvent<HTMLAnchorElement>) => void
  }
}

type DownloadSuccessProps = {
  title: ReactNode
  subtitle: ReactNode
  /** Accessible label of the step list. */
  stepsLabel: string
  /** The "Step {n}" label shown above each step title. */
  stepLabel: (step: number) => ReactNode
  steps: readonly DownloadSuccessStep[]
  /** Lines under the steps, such as "download again" and "need help", each around one link. */
  footerLines?: readonly DownloadSuccessFooterLine[]
  /** Shown over the page while the installer transfers; `progress` null is indeterminate. */
  downloading?: { label: string; progress: number | null } | null
  /** Rendered after the page content, for the app footer. */
  afterContent?: ReactNode
}

/** The /download_success page body: header, install steps and the transfer overlay. */
function DownloadSuccess({
  title,
  subtitle,
  stepsLabel,
  stepLabel,
  steps,
  footerLines = [],
  downloading,
  afterContent
}: DownloadSuccessProps) {
  return (
    <>
      <main className={styles.root}>
        <header className={styles.header}>
          <img src={dclLogo} alt="" width={48} height={48} />
          <h1 className={styles.title}>{title}</h1>
          <p className={styles.subtitle}>{subtitle}</p>
        </header>
        <ol className={styles.steps} aria-label={stepsLabel}>
          {steps.map((step, index) => (
            <li key={index} className={styles.step}>
              <div
                className={
                  index % 3 === 2
                    ? `${styles.stepHeader} ${styles.stepHeaderDeep}`
                    : styles.stepHeader
                }
              >
                <p className={styles.stepLabel}>{stepLabel(index + 1)}</p>
                <h2 className={styles.stepTitle}>{step.title}</h2>
                <p className={styles.stepText}>{step.text}</p>
              </div>
              {step.image ? (
                <div className={styles.media}>
                  <img
                    className={step.imageFit === 'contain' ? styles.imageContain : styles.image}
                    src={step.image}
                    alt={step.imageAlt ?? ''}
                  />
                  {step.highlight ? (
                    <span
                      className={styles.highlight}
                      data-testid="download-steps-highlight"
                      style={
                        {
                          '--highlight-x': `${step.highlight.x}%`,
                          '--highlight-y': `${step.highlight.y}%`
                        } as CSSProperties
                      }
                    >
                      <span className={styles.glow} />
                      <img className={styles.highlightIcon} src={highlightIcon} alt="" />
                    </span>
                  ) : null}
                </div>
              ) : null}
            </li>
          ))}
        </ol>
        {footerLines.length ? (
          <div className={styles.footer}>
            {footerLines.map((line, index) => (
              <p key={index} className={styles.footerLine}>
                {line.text(
                  <a
                    className={styles.footerLink}
                    href={line.link.href}
                    target={line.link.external ? '_blank' : undefined}
                    rel={line.link.external ? 'noopener noreferrer' : undefined}
                    onClick={line.link.onClick}
                  >
                    {line.link.label}
                    {line.link.external ? (
                      <img className={styles.externalIcon} src={externalIcon} alt="" />
                    ) : null}
                  </a>
                )}
              </p>
            ))}
          </div>
        ) : null}
      </main>
      {downloading ? (
        <div className={styles.overlay} role="status">
          <GeneralDecentralandColor size={96} aria-hidden="true" />
          <p className={styles.overlayLabel}>{downloading.label}</p>
          <progress
            className={styles.progress}
            aria-label={downloading.label}
            max={100}
            {...(downloading.progress === null ? {} : { value: downloading.progress })}
          />
        </div>
      ) : null}
      {afterContent}
    </>
  )
}

export { DownloadSuccess }
export type { DownloadSuccessFooterLine, DownloadSuccessProps, DownloadSuccessStep }
