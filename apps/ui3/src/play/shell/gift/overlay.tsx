// The gift's transfer over the app, in the shell's dialog (../dialogOverlay): drawn where the wallet is, with the
// recipient and the item the shell resolved itself. Escape closes it, as its CLOSE does (a gift already signed goes on).

import { GiftTransferPanel } from '@hud/features/gifting/GiftTransferPanel'
import { createShellDialogHost, ShellDialog } from '../dialogOverlay'
import type { ShellGiftUi } from './gifting'

/** The panel's host, mounted once in the shell page; `restoreFocus` gives the focus back to the app. */
export function createGiftOverlay(restoreFocus: () => void): ShellGiftUi {
  const host = createShellDialogHost(restoreFocus)
  return {
    show(view, handlers) {
      host.show((quiet) => (
        <ShellDialog step={view.step} view={view} quiet={quiet} onEscape={handlers.onClose}>
          <GiftTransferPanel view={view} handlers={handlers} />
        </ShellDialog>
      ))
    },
    hide: () => host.hide()
  }
}
