/** HUD-only opt-in, also set by the native client's --debug-panel flag. */
export const DEBUG_PANEL_PARAM = 'debugPanel'

export function debugPanelLauncherEnabled(): boolean {
  return new URLSearchParams(location.search).get(DEBUG_PANEL_PARAM) === '1'
}
