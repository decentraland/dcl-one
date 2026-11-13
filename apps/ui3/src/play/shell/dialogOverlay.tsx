// A dialog over the app, drawn by the web shell (its own document, above the app's iframe): the design's 90% black
// scrim and the dialog, at the HUD's scale. The Shop's purchase dialog and the Donate panel both use it, so a
// confirmation is taken here, where the wallet is, not in the app. While it shows, the rest of the page (the app's
// frame included) takes no focus or input, keyboard focus stays in it (Tab cycles) and Escape closes it when the
// dialog allows; closing gives the focus back to the app.

import '@fontsource-variable/inter/index.css'
import '@hud/styles/tokens.css'
import { useEffect, useRef, type ReactNode } from 'react'
import { createRoot, type Root } from 'react-dom/client'
import { installHudScale } from '@explorer-core/lib/hudScale'
import styles from './dialogOverlay.module.css'

const FOCUSABLE = 'button:not([disabled]), input:not([disabled]), [href], [tabindex]:not([tabindex="-1"])'
/** The buttons a step's first focus goes to (a scrolling row or a field may come before them, and is reached by Tab). */
const BUTTONS = 'button:not([disabled])'

export interface ShellDialogProps {
  /** The step shown: a new step takes the first focus again. */
  step: string
  /** Changes with every view, so a view that replaced the focused button takes the focus in again. */
  view: unknown
  /** The first focus of a dialog opened by a pointer press in the app's frame stays unringed. */
  quiet: boolean
  /** Escape's action; none while the dialog may not be closed. */
  onEscape?: () => void
  children: ReactNode
}

export function ShellDialog({ step, view, quiet, onEscape, children }: ShellDialogProps): React.JSX.Element {
  const ref = useRef<HTMLDivElement>(null)
  const first = useRef(true)
  const shownStep = useRef<string | null>(null)
  // Focus the dialog's first enabled button as each step shows: the one that spends nothing (CANCEL, CLOSE), so an
  // Enter pressed in passing never confirms. Opened by a pointer press in the app's frame, the browser would take
  // that first focus for the keyboard's (this page saw no pointer) and ring the button: `quiet` keeps that one
  // unrung. Opened from the keyboard, and on every later step, focus rings as usual. A view that replaces the
  // focused button within the same step takes the focus in again too: it never falls to the page under the scrim.
  useEffect(() => {
    if (shownStep.current === step && ref.current?.contains(document.activeElement)) return
    shownStep.current = step
    const buttons = ref.current?.querySelectorAll<HTMLElement>(BUTTONS)
    const target = buttons && buttons.length > 0 ? buttons[0] : (ref.current?.querySelector<HTMLElement>(FOCUSABLE) ?? ref.current)
    if (first.current && quiet) target?.focus({ focusVisible: false } as FocusOptions)
    else target?.focus()
    first.current = false
  }, [step, view, quiet])
  const onKeyDown = (e: React.KeyboardEvent): void => {
    if (e.key === 'Escape') {
      e.preventDefault()
      onEscape?.()
      return
    }
    if (e.key !== 'Tab') return
    e.preventDefault()
    const items = [...(ref.current?.querySelectorAll<HTMLElement>(FOCUSABLE) ?? [])]
    if (items.length === 0) return
    const at = items.indexOf(document.activeElement as HTMLElement)
    const next = e.shiftKey ? (at <= 0 ? items.length - 1 : at - 1) : at === items.length - 1 ? 0 : at + 1
    items[next].focus()
  }
  return (
    <div ref={ref} className={styles.overlay} tabIndex={-1} onKeyDown={onKeyDown}>
      <div className={styles.scrim} aria-hidden="true" />
      <div className={styles.stage}>{children}</div>
    </div>
  )
}

export interface ShellDialogHost {
  /** Shows `render(quiet)`; the first show after a hide is a new opening (a fresh first focus). */
  show(render: (quiet: boolean) => ReactNode, opening?: { pointer: boolean }): void
  hide(): void
}

/** The dialog's host, mounted once in the shell page; `restoreFocus` gives the focus back to the app. */
export function createShellDialogHost(restoreFocus: () => void): ShellDialogHost {
  let host: HTMLDivElement | null = null
  let root: Root | null = null
  // Each opening is its own dialog (a fresh first focus); its steps re-render it.
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
    show(render, how) {
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
      root?.render(<div key={opening} style={{ display: 'contents' }}>{render(quiet)}</div>)
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
