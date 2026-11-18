// Leave the Explorer. On the desktop the engine quits. A tab the user opened themselves can't be
// closed by script, so send them somewhere safe instead: the host's fixed exit, never an address a
// link chose. Inside the shell, the shell does it: from here it would only navigate the app's
// iframe.
import { inShell, postToShell } from '@explorer-core/lib/shell'
import { exitUrl, isNativeHud } from '@hud-host'

let nativeQuit: (() => void) | null = null

export function setNativeQuit(quit: (() => void) | null): void {
  nativeQuit = quit
}

export function exitApplication(): void {
  if (isNativeHud()) {
    if (nativeQuit != null) nativeQuit()
    else console.warn('[exit] no engine to quit yet')
    return
  }
  if (inShell) return postToShell({ type: 'bevy-shell:exit' })
  window.close()
  window.location.replace(exitUrl())
}
