// The purchase dialog over the app, in the shell's dialog (../dialogOverlay): ShopPurchaseDialog, with the buyer's
// confirmation taken there, where the wallet is, not in the app. Escape cancels unless the purchase is processing.

import { MaskIcon } from '@hud/design'
import emotesIcon from '@hud/assets/backpack/icon-emotes.webp'
import { CategoryIcon } from '@hud/features/backpack/categoryIcons'
import { ShopPurchaseDialog, type ShopPurchaseItem, type ShopPurchaseStep } from '../../features/shop/ui'
import { createShellDialogHost, ShellDialog } from '../dialogOverlay'
import type { ShellPurchaseItem, ShellPurchaseView, ShellShopUi } from './purchase'

function withIcon(item: ShellPurchaseItem): ShopPurchaseItem {
  const { category, ...rest } = item
  const categoryIcon = category === 'emote' ? <MaskIcon src={emotesIcon} size={20} /> : category ? <CategoryIcon category={category} size={20} /> : undefined
  return { ...rest, categoryIcon }
}

function toStep(view: ShellPurchaseView): ShopPurchaseStep {
  return 'items' in view ? ({ ...view, items: view.items.map(withIcon) } as ShopPurchaseStep) : view
}

/** The dialog's host, mounted once in the shell page; `restoreFocus` gives the focus back to the app. */
export function createShopOverlay(restoreFocus: () => void): ShellShopUi {
  const host = createShellDialogHost(restoreFocus)
  return {
    show(view, handlers, how) {
      host.show(
        (quiet) => (
          <ShellDialog step={view.step} view={view} quiet={quiet} onEscape={view.step === 'processing' ? undefined : handlers.onClose}>
            <ShopPurchaseDialog view={toStep(view)} {...handlers} />
          </ShellDialog>
        ),
        how
      )
    },
    hide: () => host.hide()
  }
}
