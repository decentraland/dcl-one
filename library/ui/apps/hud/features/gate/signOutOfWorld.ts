import { clearStoredLogins } from '@explorer-core/features/auth/sso'
import { reloadPage } from '@explorer-core/lib/reloadPage'
import { inShell, postToShell } from '@explorer-core/lib/shell'
import { dropDestination } from '@explorer-core/lib/startScreenUrl'

// Signing out in world. The engine keeps its account while a realm is loaded, so the page loads
// again without the stored sign-in: the engine session, comms and any call end with the old page,
// and the new one opens on the start screen. Inside the shell, the shell does both: it holds the
// stored sign-in, and from here a reload would only load the app's iframe again. The place goes
// from the address bar first, so the new page is the start screen and not a link to that place.
export function signOutOfWorld(): void {
  if (inShell) return postToShell({ type: 'bevy-shell:sign-out' })
  clearStoredLogins()
  dropDestination()
  reloadPage()
}
