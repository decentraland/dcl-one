import { useEffect, useId, useRef, useState } from 'react'
import { GeneralCheck, GeneralWarningError } from '../../../foundations/icons'
import styles from './HelpStatus.module.css'

type HelpServiceState = 'ok' | 'down'
type HelpOverallState = 'ok' | 'slow' | 'down'

type HelpStatusReport = {
  overall: HelpOverallState
  /** The overall state in words, shown as the list's header and announced when the checks finish. */
  overallText: string
  services: readonly { name: string; state: HelpServiceState }[]
}

type HelpStatusProps = {
  /** The toggle's label. */
  label: string
  /** Null while the services are being checked. */
  report: HelpStatusReport | null
  detailsLabel: string
  detailsHref: string
}

const STATE_CLASS = { ok: styles.ok, slow: styles.slow, down: styles.down } as const

function StatusMark({ state }: { state: HelpOverallState }) {
  return (
    <span className={[styles.mark, STATE_CLASS[state]].join(' ')} aria-hidden="true">
      {state === 'ok' ? (
        <GeneralCheck size={12} focusable="false" />
      ) : (
        <GeneralWarningError size={11} focusable="false" />
      )}
    </span>
  )
}

/**
 * The system status disclosure: a spinner while the services are checked, then the overall mark and a toggle that
 * opens the per-service list. Escape or a click outside closes it. TODO(figma-ds): replace the disclosure with the
 * design system's Dropdown once it exists.
 */
function HelpStatus({ label, report, detailsLabel, detailsHref }: HelpStatusProps) {
  const listId = useId()
  const [open, setOpen] = useState(false)
  const rootRef = useRef<HTMLDivElement>(null)
  const toggleRef = useRef<HTMLButtonElement>(null)

  useEffect(() => {
    if (!open) return
    const onPointerDown = (event: PointerEvent) => {
      if (!rootRef.current?.contains(event.target as Node)) setOpen(false)
    }
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key !== 'Escape') return
      setOpen(false)
      toggleRef.current?.focus()
    }
    document.addEventListener('pointerdown', onPointerDown)
    document.addEventListener('keydown', onKeyDown)
    return () => {
      document.removeEventListener('pointerdown', onPointerDown)
      document.removeEventListener('keydown', onKeyDown)
    }
  }, [open])

  return (
    <div className={styles.root} ref={rootRef}>
      <span className={styles.visuallyHidden} role="status">
        {report ? `${label}: ${report.overallText}` : ''}
      </span>
      {report ? (
        <>
          <StatusMark state={report.overall} />
          <button
            ref={toggleRef}
            type="button"
            className={styles.toggle}
            aria-expanded={open}
            aria-controls={listId}
            onClick={() => setOpen((value) => !value)}
          >
            {label}
          </button>
          <div id={listId} className={styles.popover} hidden={!open}>
            <p className={[styles.header, STATE_CLASS[report.overall]].join(' ')}>
              {report.overallText}
            </p>
            <ul className={styles.services}>
              {report.services.map((service) => (
                <li key={service.name} className={styles.service}>
                  <StatusMark state={service.state} />
                  <span>{service.name}</span>
                  <span className={styles.visuallyHidden}>
                    {service.state === 'ok' ? ' (up)' : ' (down)'}
                  </span>
                </li>
              ))}
            </ul>
            <div className={styles.details}>
              <a
                className={styles.detailsLink}
                href={detailsHref}
                target="_blank"
                rel="noopener noreferrer"
              >
                {detailsLabel}
              </a>
            </div>
          </div>
        </>
      ) : (
        <span className={styles.spinner} aria-hidden="true" />
      )}
    </div>
  )
}

export { HelpStatus }
export type { HelpOverallState, HelpServiceState, HelpStatusProps, HelpStatusReport }
