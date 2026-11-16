import { clearStoredLogins } from '@explorer-core/features/auth/sso'
import { reloadPage } from '@explorer-core/lib/reloadPage'
import { inShell, onShellMessage, postToShell } from '@explorer-core/lib/shell'
import { dropDestination } from '@explorer-core/lib/startScreenUrl'
import { showToast } from '../../design'

let hearsShell = false

// Signing out in world. The engine keeps its account while a realm is loaded, so the page loads
// again without the stored sign-in: the engine session, comms and any call end with the old page,
// and the new one opens on the start screen. Inside the shell, the shell does both: it holds the
// stored sign-in, and from here a reload would only load the app's iframe again. The place goes
// from the address bar first, so the new page is the start screen and not a link to that place.
// The shell answers only when it could not remove the stored sign-in: the player stays in world.
export function signOutOfWorld(): void {
  if (inShell) {
    if (!hearsShell) {
      hearsShell = true
      onShellMessage('bevy-shell:sign-out-failed', () => showToast('Could not sign out. Try again in a moment.', { tone: 'error' }))
    }
    return postToShell({ type: 'bevy-shell:sign-out' })
  }
  clearStoredLogins()
  dropDestination()
  reloadPage()
}
