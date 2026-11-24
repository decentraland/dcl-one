// The notice the HideUi key (U) shows as it cycles the UI: what its next press does, with the key
// drawn as a key cap.

import { dismissToast, KeyCap, showToast } from '../../design'

/** What the HideUi key has hidden: nothing, the Explorer HUD, or the HUD and the scene UI. */
export type UiHidden = 'none' | 'explorer' | 'all'

const TOAST_KEY = 'hide-ui'

/** Show the notice for `hidden` (`key` labels the HideUi binding), or take it down at 'none'. */
export function showHideUiToast(hidden: UiHidden, key: string): void {
  if (hidden === 'none') {
    dismissToast(TOAST_KEY)
    return
  }
  showToast(
    <span>
      Press <KeyCap label={key} /> {hidden === 'explorer' ? 'to hide the scene UI.' : 'to show the UI again.'}
    </span>,
    { key: TOAST_KEY }
  )
}
