// The purchase dialog over the app, drawn by the web shell (its own document, above the app's iframe):
// the design's 90% black scrim and ShopPurchaseDialog, at the HUD's scale. The buyer's confirmation is
// taken here, where the wallet is, not in the app. While it shows, keyboard focus stays in it (Tab cycles,
// Escape cancels unless the purchase is processing); closing gives the focus back to the app.

import '@fontsource-variable/inter/index.css'
import '../../styles/tokens.css'
import { useEffect, useRef } from 'react'
import { createRoot, type Root } from 'react-dom/client'
import { MaskIcon } from '../../design'
import emotesIcon from '../../assets/backpack/icon-emotes.webp'
import { CategoryIcon } from '../../features/backpack/categoryIcons'
import { ShopPurchaseDialog, type ShopPurchaseItem, type ShopPurchaseStep } from '../../features/shop/ui'
import { installHudScale } from '../../lib/hudScale'
import type { ShellPurchaseHandlers, ShellPurchaseItem, ShellPurchaseView, ShellShopUi } from './purchase'
import styles from './overlay.module.css'

function withIcon(item: ShellPurchaseItem): ShopPurchaseItem {
  const { category, ...rest } = item
  const categoryIcon = category === 'emote' ? <MaskIcon src={emotesIcon} size={20} /> : category ? <CategoryIcon category={category} size={20} /> : undefined
  return { ...rest, categoryIcon }
}

function toStep(view: ShellPurchaseView): ShopPurchaseStep {
  return 'item' in view ? ({ ...view, item: withIcon(view.item) } as ShopPurchaseStep) : view
}

const FOCUSABLE = 'button:not([disabled]), [href], [tabindex]:not([tabindex="-1"])'

function Overlay({ view, handlers, quiet }: { view: ShellPurchaseView; handlers: ShellPurchaseHandlers; quiet: boolean }): React.JSX.Element {
  const ref = useRef<HTMLDivElement>(null)
  const busy = view.step === 'processing'
  const first = useRef(true)
  // Focus the dialog's main action as each step shows (the last enabled button: BUY ITEM, CONTINUE…).
  // Opened by a pointer press in the app's frame, the browser would take that first focus for the
  // keyboard's (this page saw no pointer) and ring the button: `quiet` keeps that one unrung. Opened from
  // the keyboard, and on every later step (this page now knows how it is used), focus rings as usual.
  useEffect(() => {
    const buttons = ref.current?.querySelectorAll<HTMLElement>(FOCUSABLE)
    const target = buttons && buttons.length > 0 ? buttons[buttons.length - 1] : ref.current
    if (first.current && quiet) target?.focus({ focusVisible: false } as FocusOptions)
    else target?.focus()
    first.current = false
  }, [view.step, quiet])
  const onKeyDown = (e: React.KeyboardEvent): void => {
    if (e.key === 'Escape') {
      e.preventDefault()
      if (!busy) handlers.onClose()
      return
    }
    if (e.key !== 'Tab') return
    const items = [...(ref.current?.querySelectorAll<HTMLElement>(FOCUSABLE) ?? [])]
    if (items.length === 0) {
      e.preventDefault()
      return
    }
    const at = items.indexOf(document.activeElement as HTMLElement)
    const next = e.shiftKey ? (at <= 0 ? items.length - 1 : at - 1) : at === items.length - 1 ? 0 : at + 1
    e.preventDefault()
    items[next].focus()
  }
  return (
    <div ref={ref} className={styles.overlay} tabIndex={-1} onKeyDown={onKeyDown}>
      <div className={styles.scrim} aria-hidden="true" />
      <div className={styles.stage}>
        <ShopPurchaseDialog view={toStep(view)} {...handlers} />
      </div>
    </div>
  )
}

/** The dialog's host, mounted once in the shell page; `restoreFocus` gives the focus back to the app. */
export function createShopOverlay(restoreFocus: () => void): ShellShopUi {
  let host: HTMLDivElement | null = null
  let root: Root | null = null
  // Each opening is its own dialog (a fresh first focus); its steps re-render it.
  let opening = 0
  let quiet = false
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
      }
      host.hidden = false
      root?.render(<Overlay key={opening} view={view} handlers={handlers} quiet={quiet} />)
    },
    hide() {
      if (!host || host.hidden) return
      root?.render(null)
      host.hidden = true
      restoreFocus()
    }
  }
}
