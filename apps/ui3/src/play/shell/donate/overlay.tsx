// The Donate panel over the app, drawn by the web shell (its own document, above the app's iframe), at the HUD's
// scale: the player's amount is taken here, where the wallet is, not in the app. While it shows, the rest of the
// page takes no focus or input, keyboard focus stays in it (Tab cycles through its buttons and fields) and Escape
// closes it, as its CLOSE does (a tip already signed goes on and is watched); closing gives the focus back to the app.

import '@fontsource-variable/inter/index.css'
import '@hud/styles/tokens.css'
import { useEffect, useRef } from 'react'
import { createRoot, type Root } from 'react-dom/client'
import { installHudScale } from '@explorer-core/lib/hudScale'
import { DonatePanel } from '@hud/features/donate/DonatePanel'
import type { DonateHandlers, DonateView, ShellDonateUi } from './donation'
import styles from './overlay.module.css'

const FOCUSABLE = 'button:not([disabled]), input:not([disabled]), [href], [tabindex]:not([tabindex="-1"])'

function Overlay({ view, handlers, quiet }: { view: DonateView; handlers: DonateHandlers; quiet: boolean }): React.JSX.Element {
  const ref = useRef<HTMLDivElement>(null)
  const first = useRef(true)
  const shownStep = useRef<string | null>(null)
  // Each step's first focus goes to its first button: the one that sends nothing (Cancel, CLOSE).
  useEffect(() => {
    if (shownStep.current === view.step && ref.current?.contains(document.activeElement)) return
    shownStep.current = view.step
    const target = ref.current?.querySelector<HTMLElement>('button:not([disabled])') ?? ref.current
    if (first.current && quiet) target?.focus({ focusVisible: false } as FocusOptions)
    else target?.focus()
    first.current = false
  }, [view, quiet])
  const onKeyDown = (e: React.KeyboardEvent): void => {
    if (e.key === 'Escape') {
      e.preventDefault()
      handlers.onClose()
      return
    }
    if (e.key !== 'Tab') return
    const items = [...(ref.current?.querySelectorAll<HTMLElement>(FOCUSABLE) ?? [])]
    e.preventDefault()
    if (items.length === 0) return
    const at = items.indexOf(document.activeElement as HTMLElement)
    const next = e.shiftKey ? (at <= 0 ? items.length - 1 : at - 1) : at === items.length - 1 ? 0 : at + 1
    items[next].focus()
  }
  return (
    <div ref={ref} className={styles.overlay} tabIndex={-1} onKeyDown={onKeyDown}>
      <div className={styles.scrim} aria-hidden="true" />
      <div className={styles.stage}>
        <DonatePanel view={view} handlers={handlers} />
      </div>
    </div>
  )
}

/** The panel's host, mounted once in the shell page; `restoreFocus` gives the focus back to the app. */
export function createDonateOverlay(restoreFocus: () => void): ShellDonateUi {
  let host: HTMLDivElement | null = null
  let root: Root | null = null
  let opening = 0
  let quiet = false
  let inerted: HTMLElement[] = []
  const block = (): void => {
    for (const el of document.body.children) {
      if (el !== host && el instanceof HTMLElement && !el.inert) {
        el.inert = true
        inerted.push(el)
      }
    }
  }
  const unblock = (): void => {
    for (const el of inerted) el.inert = false
    inerted = []
  }
  return {
    show(view, handlers, how) {
      if (!host) {
        installHudScale()
        host = document.createElement('div')
        document.body.appendChild(host)
        root = createRoot(host)
      }
      if (host.hidden || opening === 0) {
        opening++
        quiet = how?.pointer === true
        block()
      }
      host.hidden = false
      root?.render(<Overlay key={opening} view={view} handlers={handlers} quiet={quiet} />)
    },
    hide() {
      if (!host || host.hidden) return
      root?.render(null)
      host.hidden = true
      unblock()
      restoreFocus()
    }
  }
}
