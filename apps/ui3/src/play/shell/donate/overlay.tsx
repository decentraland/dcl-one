// The Donate panel over the app, in the shell's dialog (../dialogOverlay): the player's amount is taken there, where
// the wallet is, not in the app. Escape closes it, as its CLOSE does (a tip already signed goes on and is watched).

import { DonatePanel } from '@hud/features/donate/DonatePanel'
import { createShellDialogHost, ShellDialog } from '../dialogOverlay'
import type { ShellDonateUi } from './donation'

/** The panel's host, mounted once in the shell page; `restoreFocus` gives the focus back to the app. */
export function createDonateOverlay(restoreFocus: () => void): ShellDonateUi {
  const host = createShellDialogHost(restoreFocus)
  return {
    show(view, handlers, how) {
      host.show(
        (quiet) => (
          <ShellDialog step={view.step} view={view} quiet={quiet} onEscape={handlers.onClose}>
            <DonatePanel view={view} handlers={handlers} />
          </ShellDialog>
        ),
        how
      )
    },
    hide: () => host.hide()
  }
}
