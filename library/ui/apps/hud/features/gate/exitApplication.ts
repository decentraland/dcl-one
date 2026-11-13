// Leave the Explorer. A tab the user opened themselves can't be closed by script, so send them
// somewhere safe instead: the host's fixed exit, never an address a link chose. Inside the shell,
// the shell does it: from here it would only navigate the app's iframe.
import { inShell, postToShell } from '@explorer-core/lib/shell'
import { exitUrl, isNativeHud } from '@hud-host'

/** The desktop client can't be quit from the page, so it offers no Exit. */
export function canExit(): boolean {
  return !isNativeHud()
}

export function exitApplication(): void {
  if (inShell) return postToShell({ type: 'bevy-shell:exit' })
  window.close()
  window.location.replace(exitUrl())
}
