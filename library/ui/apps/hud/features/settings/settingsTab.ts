// The Settings tab the page opens on: the one last picked, shared by the full page and the lobby's
// compact Settings, or the one a caller asks for (the Auto Graphics message opens Performance).

let lastTab: string | null = null

export function getSettingsTab(): string | null {
  return lastTab
}

export function setSettingsTab(tab: string | null): void {
  lastTab = tab
}
